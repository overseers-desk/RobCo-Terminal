//! Deterministic screenshots of the running appliance.
//!
//! Runs the target binary on a private Xvfb display with a scratch HOME,
//! drives it with xdotool, and captures a screenshot with ImageMagick's
//! `import`. See [`CONTRACT`] for what a binary must do to be drivable by
//! this harness at all.
//!
//! Two mechanics worth knowing:
//!
//! - The command re-execs itself under `xvfb-run`, guarded by an
//!   `XTASK_SNAP_INNER` env var so the second invocation skips the Xvfb
//!   wrapping: it spawns `current_exe()` (Rust has no direct
//!   process-image-replace on all targets) and exits with the child's exit
//!   code, which is externally indistinguishable.
//! - The output path is made absolute against the current directory without
//!   requiring it to exist. Unlike a full `realpath -m` it does not
//!   collapse `..` in a nonexistent tail; out paths in practice are simple,
//!   non-relative-dotted paths.
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

use crate::proc::{capture, reexec_under_xvfb, run_ignore_status, run_ok};
use crate::x11;

/// The CLI/window contract a binary must honor for this harness to drive
/// it at all, independent of which rendering stack sits behind it.
pub const CONTRACT: &str = r#"
1. `<binary> --default-settings --profile <name>` starts fresh, ignoring
   any real user config, seeded from the named built-in profile.
2. The main window's WM_CLASS matches the binary's own basename, so
   `xdotool search --class $(basename BINARY)` finds it. A stray helper
   window is tolerated as long as at least one candidate window belongs
   to the launched PID and has geometry width > 100.
3. `Ctrl+Shift+T` opens one more channel/tab.
4. The window's program-specified minimum width (as read by
   `xprop -id <wid> WM_NORMAL_HINTS`) is the channel bank's live pixel width
   plus the least screen well the binary will work in, and the window never
   stands under it. The well's share is fixed for as long as the font is, so
   a change in that minimum is the bank's own movement -- which is what
   --units reads the seam drag through, and all it needs from this item.
5. HOME, XDG_DATA_HOME, XDG_CONFIG_HOME, XDG_CACHE_HOME, XDG_RUNTIME_DIR
   and TMPDIR are honored for settings/socket isolation, so concurrent
   scratch runs never collide with a real user session or each other.
"#;

pub struct SnapArgs {
    pub binary: PathBuf,
    pub profile: String,
    pub out: PathBuf,
    pub width: u32,
    pub height: u32,
    pub channels: u32,
    pub units: Option<i64>,
    pub deterministic: bool,
}

/// The `-e` command this tool hands the first channel instead of the user's
/// shell: `yes` reprints this one short line forever.
///
/// Two designs were tried and measurably rejected before this one (by
/// snapping twice and comparing pixels, not by reasoning alone; all
/// figures below are glass-crop RMSE from that snapping, one host, one
/// session -- guides to the shape of the problem, not portable constants):
///
/// - A line as wide as the widest plausible terminal (so it always wraps)
///   still gets a `\n` from `yes` after each copy, so it wraps into some
///   number of terminal rows and the newline then forces a fresh line at
///   column 0 -- a phase that depends on `len(line) mod columns`, which
///   this harness does not control and is not guaranteed to divide evenly.
///   Two independent runs visibly disagreed on which row started at which
///   digit; the same digit landed in different screen columns run to run.
/// - Dropping every newline (one character, `yes CHAR | tr -d '\n'`, the
///   terminal's own auto-wrap doing all the line breaks) removes that
///   phase but trades it for a worse problem: a fully-lit, high-frequency
///   fill is exquisitely sensitive to the one- or two-pixel rendering
///   jitter every CRT-glass comparison already carries (curvature
///   resampling, antialiasing, the bloom pass), because *every* pixel
///   sits on a glyph edge. Measured same-binary self-floor 0.027,
///   cross-run 0.215 -- five times looser than the content-*mismatched*
///   banner figure the deterministic mode exists to beat.
///
/// This line is short enough (`DETERMINISTIC_LINE.len() == 10`) to fit
/// inside one row on every bundled profile at any window size this harness
/// snaps at (the narrowest realistic screen well is still >40 columns), so
/// `yes` never reaches its own wrap boundary: every row is this line,
/// left-anchored at column 0, followed by whatever blank the row already
/// carried -- the density plain shell content already has, not an
/// adversarial worst case for RMSE. Position is then anchored by the
/// terminal's own left margin, not by a phase this harness has to get
/// right. Measured: self-floor 0.014, close to the *non*-deterministic
/// (real shell prompt) self-floor of 0.052 measured on the same host in
/// the same session, confirming the remaining noise is this host's
/// rendering jitter, not a phase this line reintroduced.
const DETERMINISTIC_LINE: &str = "0123456789";

const INNER_ENV: &str = "XTASK_SNAP_INNER";

pub fn run(args: SnapArgs) -> Result<()> {
    let binary = std::fs::canonicalize(&args.binary)
        .with_context(|| format!("binary not found: {}", args.binary.display()))?;
    let out = make_absolute(&args.out);

    if std::env::var_os(INNER_ENV).is_none() {
        return reexec_under_xvfb_for_snap(&binary, &args.profile, &out, &args);
    }

    run_inner(&binary, &out, &args)
}

/// `realpath -m`-ish: absolute, but doesn't require the path to exist.
fn make_absolute(p: &Path) -> PathBuf {
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(p)
    }
}

fn reexec_under_xvfb_for_snap(
    binary: &Path,
    profile: &str,
    out: &Path,
    args: &SnapArgs,
) -> Result<()> {
    let screen = format!("-screen 0 {}x{}x24", args.width, args.height);

    let mut argv: Vec<String> = vec![
        "snap".into(),
        binary.display().to_string(),
        profile.to_string(),
        out.display().to_string(),
        "--width".into(),
        args.width.to_string(),
        "--height".into(),
        args.height.to_string(),
        "--channels".into(),
        args.channels.to_string(),
    ];
    if let Some(u) = args.units {
        argv.push("--units".into());
        argv.push(u.to_string());
    }
    if args.deterministic {
        argv.push("--deterministic".into());
    }

    reexec_under_xvfb(INNER_ENV, &screen, &argv)
}

/// Kills the app child and removes the scratch dir on drop, regardless of
/// which return path got us there, so a scratch dir never outlives its run.
struct Cleanup {
    child: Child,
    _scratch: tempfile::TempDir,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        // _scratch's own Drop removes the directory tree.
    }
}

/// `binary` and `out` are the resolved absolute paths [`run`] worked out;
/// everything else this needs it reads off the parsed arguments directly,
/// rather than being handed the same eight fields one at a time.
fn run_inner(binary: &Path, out: &Path, args: &SnapArgs) -> Result<()> {
    let SnapArgs {
        profile,
        width,
        height,
        channels,
        units,
        deterministic,
        ..
    } = args;
    let (width, height, channels) = (*width, *height, *channels);
    let (units, deterministic) = (*units, *deterministic);
    let scratch = tempfile::Builder::new()
        .prefix("robco-snap.")
        .tempdir_in(std::env::temp_dir())
        .context("creating scratch dir")?;

    let run_dir = scratch.path().join("run");
    std::fs::create_dir_all(&run_dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&run_dir, std::fs::Permissions::from_mode(0o700))?;
    }
    let tmp_dir = scratch.path().join("tmp");
    std::fs::create_dir_all(&tmp_dir)?;

    let mut cmd = Command::new(binary);
    cmd.arg("--default-settings").arg("--profile").arg(profile);
    if deterministic {
        // Catches every trailing argument (contract item: "-e ... use it as
        // the last option"), so this must be the last thing added to argv.
        cmd.arg("-e").arg("yes").arg(DETERMINISTIC_LINE);
    }
    let child = cmd
        .env("HOME", scratch.path())
        .env("XDG_DATA_HOME", scratch.path().join(".local/share"))
        .env("XDG_CONFIG_HOME", scratch.path().join(".config"))
        .env("XDG_CACHE_HOME", scratch.path().join(".cache"))
        .env("XDG_RUNTIME_DIR", &run_dir)
        .env("TMPDIR", &tmp_dir)
        .env("WINIT_UNIX_BACKEND", "x11")
        .env("LIBGL_ALWAYS_SOFTWARE", "1")
        // Same reason `verify` does it: the screenshot is taken off the X
        // display this command was pointed at, so the app must not prefer a
        // Wayland compositor inherited from the developer's own session and
        // draw somewhere the screenshot cannot see it.
        .env_remove("WAYLAND_DISPLAY")
        .stdin(Stdio::null())
        .spawn()
        .with_context(|| format!("spawning {}", binary.display()))?;

    let app_pid = child.id();
    let guard = Cleanup {
        child,
        _scratch: scratch,
    };

    let class = binary
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    let wid = find_window(&class, app_pid)?;
    let Some(wid) = wid else {
        bail!("no sizable window of pid {app_pid} found");
    };

    run_ignore_status(
        "xdotool",
        &[
            "windowsize",
            "--sync",
            &wid,
            &width.to_string(),
            &height.to_string(),
        ],
    );
    run_ok("xdotool", &["windowfocus", "--sync", &wid])?;
    sleep(Duration::from_secs(1));

    if channels >= 2 {
        for _ in 2..=channels {
            run_ok("xdotool", &["key", "--clearmodifiers", "ctrl+shift+t"])?;
            sleep(Duration::from_millis(250));
        }
    }

    if let Some(units) = units {
        fit_units(&wid, height, units)?;
    }

    // Let shells print their prompts and the tube settle.
    sleep(Duration::from_secs(3));

    run_ok("import", &["-window", &wid, &out.display().to_string()])?;
    println!("{}", out.display());

    // `guard` drops here: kills the app child and removes the scratch dir.
    drop(guard);
    Ok(())
}

/// Poll (30s) for a window of `class` belonging to
/// `app_pid`. The predicate itself -- pid match plus geometry width > 100,
/// contract item 2 -- lives in [`crate::x11`], shared with `verify`.
fn find_window(class: &str, app_pid: u32) -> Result<Option<String>> {
    Ok(
        x11::wait_for_windows(class, app_pid, 1, Duration::from_secs(30))
            .into_iter()
            .next(),
    )
}

/// The shipped default profile's channel bank, in logical pixels: where the
/// seam stands before anything is dragged, which is where the grab has to
/// start. The number is the annunciator's own measures -- 3 (bank padding) +
/// 46 (numeral lane) + 16 (column gap) + 168 (twelve characters of the
/// shipped LED font's measured cell) + 14 (right padding) -- pinned by
/// `chassis::a_cabinet_built_from_the_shipped_profile_alone_stands_247_px_wide`,
/// the same figure `compare`'s `--region bank` crops at. A binary snapped at
/// a profile with a different bank needs its own number here.
const SHIPPED_BANK_WIDTH: i64 = 247;

/// The window's program-specified minimum width. It moves exactly as the bank
/// does, the screen well's share of it being fixed for as long as the font
/// is, so the difference between two readings is what the drag moved the bank
/// by (contract item 4).
fn hint_width(wid: &str) -> Result<i64> {
    let (min_width, _min_height) = x11::min_size_hint(wid)?;
    Ok(min_width)
}

fn fit_units(wid: &str, height: u32, units: i64) -> Result<()> {
    let start_hint = hint_width(wid)?;

    let start = SHIPPED_BANK_WIDTH + 2;
    let target = start + 12 * (units - 12);

    let geom = capture("xdotool", &["getwindowgeometry", "--shell", wid])?;
    let (x, y) =
        x11::shell_xy(&geom).context("parsing X/Y from xdotool getwindowgeometry --shell")?;
    let mid_y = y + height as i64 / 2;

    run_ok(
        "xdotool",
        &[
            "mousemove",
            "--sync",
            &(x + start).to_string(),
            &mid_y.to_string(),
        ],
    )?;
    run_ok("xdotool", &["mousedown", "1"])?;

    let step: i64 = if target > start { 24 } else { -24 };
    let mut pos = start;
    while (target - pos) * step > 0 {
        pos += step;
        if (target - pos) * step < 0 {
            pos = target;
        }
        run_ok(
            "xdotool",
            &[
                "mousemove",
                "--sync",
                &(x + pos).to_string(),
                &mid_y.to_string(),
            ],
        )?;
        sleep(Duration::from_millis(50));
    }
    run_ok(
        "xdotool",
        &[
            "mousemove",
            "--sync",
            &(x + target).to_string(),
            &mid_y.to_string(),
        ],
    )?;
    sleep(Duration::from_millis(200));
    run_ok("xdotool", &["mouseup", "1"])?;

    // Verify the fit against the same instrument: a drag that missed by a
    // character or more is a loud warning, never a quietly wrong picture. A
    // drag that moved the hint by nothing at all is the profile answering
    // that it has no bank to fit, and reads as the same warning.
    sleep(Duration::from_millis(500));
    let moved = hint_width(wid)? - start_hint;
    let want = 12 * (units - 12);
    if (moved - want).abs() >= 12 {
        eprintln!("warning: seam drag moved the bank by {moved} px, wanted {want}");
    }
    Ok(())
}
