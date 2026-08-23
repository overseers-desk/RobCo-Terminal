//! `xtask verify`: drive a binary through the CLI/window contract and say
//! which items it honors.
//!
//! [`crate::snap::CONTRACT`] states what a binary must do for the eval
//! harness to be able to screenshot it at all. Until now that statement
//! was only enforced implicitly, by `snap` failing in some way when a
//! binary broke it. This subcommand checks it directly, item by item, on
//! the same private Xvfb display and the same isolated scratch
//! environment `snap` uses -- so a rebuild binary can be held to the
//! contract before anyone tries to take a picture with it.
//!
//! It also checks the one behavior that is in the contract's spirit but
//! not in its numbered list, because it is about processes rather than
//! windows: **a second invocation opens a window in the first instance**
//! and exits 0, rather than starting a second application. That is the
//! single-instance contract this binary must honor: only one process per
//! identity ever owns the display's windows and the instance lock.
//!
//! What it deliberately does not check: anything requiring a terminal
//! session to be running inside the window (`-e`, `--workdir`), which is
//! the renderer's and the session's business rather than the shell's. Those
//! are checked by the terminal session's own tests.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::proc::{capture, reexec_under_xvfb, run_ignore_status};
use crate::x11;

const INNER_ENV: &str = "XTASK_VERIFY_INNER";

/// How many direct children a process has, or `None` where the kernel will not
/// say (a non-Linux host, or a process that has already gone).
///
/// `/proc/<pid>/task/<tid>/children` is the kernel's own answer and needs no
/// `ps`, no `pgrep` and no parsing of a process table. It is a snapshot: the
/// file is documented as unreliable while children are being reaped, which is
/// exactly why the caller waits for a *rise* rather than trusting one read.
fn children_of(pid: u32) -> Option<usize> {
    let tasks = std::fs::read_dir(format!("/proc/{pid}/task")).ok()?;
    let mut total = 0;
    for task in tasks.flatten() {
        let path = task.path().join("children");
        if let Ok(list) = std::fs::read_to_string(&path) {
            total += list.split_ascii_whitespace().count();
        }
    }
    Some(total)
}

/// Poll until the process has at least `wanted` children, and answer what it
/// ended with either way.
fn wait_for_children(pid: u32, wanted: usize, timeout: Duration) -> usize {
    let deadline = Instant::now() + timeout;
    loop {
        let count = children_of(pid).unwrap_or(0);
        if count >= wanted || Instant::now() >= deadline {
            return count;
        }
        sleep(Duration::from_millis(100));
    }
}

pub struct VerifyArgs {
    pub binary: PathBuf,
    pub profile: String,
    pub width: u32,
    pub height: u32,
}

pub fn run(args: VerifyArgs) -> Result<()> {
    let binary = std::fs::canonicalize(&args.binary)
        .with_context(|| format!("binary not found: {}", args.binary.display()))?;

    if std::env::var_os(INNER_ENV).is_none() {
        let screen = format!("-screen 0 {}x{}x24", args.width, args.height);
        let argv = vec![
            "verify".to_string(),
            binary.display().to_string(),
            "--profile".to_string(),
            args.profile.clone(),
            "--width".to_string(),
            args.width.to_string(),
            "--height".to_string(),
            args.height.to_string(),
        ];
        return reexec_under_xvfb(INNER_ENV, &screen, &argv);
    }

    run_inner(&binary, &args.profile, args.width)
}

/// The isolated environment contract item 5 requires a binary to honor.
/// Every path is inside one scratch directory, so a run can neither see
/// nor be seen by a real user session or another concurrent run --
/// including, critically here, its instance lock and its IPC socket.
struct Scratch {
    dir: tempfile::TempDir,
}

impl Scratch {
    fn new() -> Result<Self> {
        let dir = tempfile::Builder::new()
            .prefix("robco-verify.")
            .tempdir_in(std::env::temp_dir())
            .context("creating scratch dir")?;
        let run_dir = dir.path().join("run");
        std::fs::create_dir_all(&run_dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&run_dir, std::fs::Permissions::from_mode(0o700))?;
        }
        std::fs::create_dir_all(dir.path().join("tmp"))?;
        Ok(Scratch { dir })
    }

    fn apply(&self, command: &mut Command) {
        let path = self.dir.path();
        command
            .env("HOME", path)
            .env("XDG_DATA_HOME", path.join(".local/share"))
            .env("XDG_CONFIG_HOME", path.join(".config"))
            .env("XDG_CACHE_HOME", path.join(".cache"))
            .env("XDG_RUNTIME_DIR", path.join("run"))
            .env("TMPDIR", path.join("tmp"))
            .env("WINIT_UNIX_BACKEND", "x11")
            .env("LIBGL_ALWAYS_SOFTWARE", "1")
            // The harness drives windows through xdotool and xprop, so
            // the app has to be on the Xvfb display and not on whatever
            // Wayland compositor the developer's own session runs. A
            // toolkit that sees WAYLAND_DISPLAY inherited from that
            // session will prefer it and land somewhere the harness
            // cannot see; unsetting it is the only instruction every
            // toolkit obeys.
            .env_remove("WAYLAND_DISPLAY");
    }
}

/// Kills the app on the way out however we leave.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Starts a window manager on the current display if there is none, and
/// waits for it to claim the root window. The returned handle kills it on
/// the way out.
fn start_window_manager() -> Option<Running> {
    if x11::window_manager_present() {
        return None;
    }
    let child = Command::new("openbox")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let wm = Running(child);
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if x11::window_manager_present() {
            return Some(wm);
        }
        sleep(Duration::from_millis(100));
    }
    // It never claimed the root window; let the item skip rather than
    // leave a half-started process behind.
    None
}

fn run_inner(binary: &Path, profile: &str, screen_width: u32) -> Result<()> {
    let identity = binary
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let scratch = Scratch::new()?;
    let mut checks = Checks::default();

    // Fullscreen is a request a client makes of the window manager, so
    // item 8 needs one on the display. The re-exec above gives us a bare
    // Xvfb with nothing running on it, so start a WM ourselves rather than
    // skipping the item forever. Any WM would do; openbox is the smallest
    // one that speaks EWMH. If it is not installed the item still skips,
    // which is the honest answer on a machine that cannot check it.
    let _wm = start_window_manager();

    // Item 1, first half: the help the contract's option list comes from.
    let help = run_capturing(binary, &["--help"], &scratch)?;
    let missing: Vec<&str> = [
        "--default-settings",
        "--profile",
        "--workdir",
        "-e",
        "--fullscreen",
    ]
    .into_iter()
    .filter(|flag| !help.1.contains(flag))
    .collect();
    checks.item(
        "--help exits 0 and documents the contract's options",
        help.0 == Some(0) && missing.is_empty(),
        if missing.is_empty() {
            format!("exit {:?}", help.0)
        } else {
            format!("exit {:?}, undocumented: {missing:?}", help.0)
        },
    );

    // Item 1: `--default-settings --profile <name>` starts.
    let mut command = Command::new(binary);
    command
        .arg("--default-settings")
        .arg("--profile")
        .arg(profile)
        .stdin(Stdio::null());
    scratch.apply(&mut command);
    let child = command
        .spawn()
        .with_context(|| format!("spawning {}", binary.display()))?;
    let app_pid = child.id();
    let app = Running(child);

    // Item 2: a window of class = the binary's basename, owned by the
    // process we launched, big enough not to be a helper.
    let windows = x11::wait_for_windows(&identity, app_pid, 1, Duration::from_secs(30));
    checks.item(
        "a window of WM_CLASS = the binary's basename belongs to the launched pid",
        !windows.is_empty(),
        format!(
            "class {identity:?}, pid {app_pid}, {} window(s)",
            windows.len()
        ),
    );
    let Some(wid) = windows.first().cloned() else {
        checks.report();
        bail!("no window: the rest of the contract cannot be checked");
    };

    // Item 4: the minimum-size hint exists, and the window stands at or above
    // it, since a window under its own minimum is a window showing less than
    // the terminal grid the minimum exists to hold. The hint reserves the
    // bank at its narrowest, not its live width, so it is not the instrument
    // `--units` reads the seam drag through; `compare --region bank` is.
    match x11::min_size_hint(&wid) {
        Ok((min_width, min_height)) => {
            let geometry = capture("xdotool", &["getwindowgeometry", "--shell", wid.as_str()])
                .ok()
                .and_then(|out| x11::shell_wh(&out));
            checks.item(
                "WM_NORMAL_HINTS carries a program-specified minimum size the window stands at",
                min_width > 0
                    && min_height > 0
                    && geometry.is_some_and(|(w, h)| w >= min_width && h >= min_height),
                match geometry {
                    Some((w, h)) => format!("{w} by {h}, minimum {min_width} by {min_height}"),
                    None => format!("minimum {min_width} by {min_height}, geometry unreadable"),
                },
            )
        }
        Err(e) => checks.item(
            "WM_NORMAL_HINTS carries a program-specified minimum size the window stands at",
            false,
            e.to_string(),
        ),
    }

    // Item 3: `Ctrl+Shift+T` opens one more channel/tab.
    //
    // The channel this opens is a shell, and a shell is a direct child of the
    // application process (the pty is forked there), so the count of the launched pid's
    // children is the count of its channels, readable from outside without
    // asking the binary anything. That is the only externally visible
    // difference a second channel makes until the bank's furniture is drawn:
    // the window does not change size, and its title is whatever the new
    // shell says, which may be nothing.
    //
    // It runs here, before the second and third invocations, because those
    // open further windows with channels of their own and the count would then
    // be a sum rather than an observation.
    match children_of(app_pid) {
        // The baseline is a *settled* count, not the first read: a window is
        // mapped by X before the session behind it has finished forking, and
        // item 2 returns the moment the window appears, so an immediate read
        // can catch the application with no shell yet and make any later
        // number look like a rise.
        Some(_) => {
            let before = wait_for_children(app_pid, 1, Duration::from_secs(10));
            run_ignore_status("xdotool", &["windowactivate", "--sync", &wid]);
            run_ignore_status("xdotool", &["windowfocus", "--sync", &wid]);
            run_ignore_status("xdotool", &["key", "--clearmodifiers", "ctrl+shift+t"]);
            let after = wait_for_children(app_pid, before + 1, Duration::from_secs(10));
            checks.item(
                "Ctrl+Shift+T opens one more channel",
                after > before,
                format!("{before} shell(s) before, {after} after"),
            );
        }
        None => checks.item(
            "Ctrl+Shift+T opens one more channel",
            false,
            format!("cannot read /proc/{app_pid}/task/*/children"),
        ),
    }

    // The window title: a window with no title is one a user cannot tell
    // from another in a task switcher.
    let name = x11::window_name(&wid);
    checks.item(
        "the window has a title",
        !name.is_empty(),
        format!("{name:?}"),
    );

    // The single-instance rule: a second invocation must hand its request
    // over and exit 0 promptly, without becoming an application of its own.
    let mut second = Command::new(binary);
    second
        .arg("--default-settings")
        .arg("--profile")
        .arg(profile)
        .stdin(Stdio::null());
    scratch.apply(&mut second);
    let started = Instant::now();
    let status = second
        .status()
        .with_context(|| format!("spawning a second {}", binary.display()))?;
    let elapsed = started.elapsed();
    checks.item(
        "a second invocation exits 0 instead of starting a second application",
        status.success(),
        format!(
            "exit {:?} after {:.2}s",
            status.code(),
            elapsed.as_secs_f32()
        ),
    );

    // ...and the window it asked for opens *in the first instance*, which
    // is the part that distinguishes a handoff from a silent no-op.
    let windows = x11::wait_for_windows(&identity, app_pid, 2, Duration::from_secs(15));
    checks.item(
        "the second invocation's window opens in the first instance",
        windows.len() >= 2,
        format!("{} window(s) now belong to pid {app_pid}", windows.len()),
    );

    // The IPC carries a payload, not just a knock: `--fullscreen` on the
    // handing-over invocation must reach the window the first instance
    // opens.
    let before: std::collections::HashSet<String> = windows.iter().cloned().collect();
    let mut third = Command::new(binary);
    third
        .arg("--default-settings")
        .arg("--profile")
        .arg(profile)
        .arg("--fullscreen")
        .stdin(Stdio::null());
    scratch.apply(&mut third);
    let status = third.status().context("spawning a third invocation")?;
    let windows = x11::wait_for_windows(&identity, app_pid, 3, Duration::from_secs(15));
    let fresh: Vec<String> = windows
        .iter()
        .filter(|w| !before.contains(*w))
        .cloned()
        .collect();
    checks.item(
        "a handed-over --fullscreen invocation opens a further window",
        status.success() && !fresh.is_empty(),
        format!("exit {:?}, new window {fresh:?}", status.code()),
    );

    // Whether that window is *fullscreen* is a question only a window
    // manager can answer: fullscreen is a request a client makes of the
    // WM (`_NET_WM_STATE_FULLSCREEN`), so under a bare Xvfb the call is a
    // no-op in every toolkit and the window keeps its size. Asserting
    // geometry here would be testing the absence of a window manager
    // rather than the binary, so the check states what it could not
    // check instead of passing or failing on it.
    if !x11::window_manager_present() {
        checks.skip(
            "the handed-over window comes up fullscreen",
            "no window manager on this display; fullscreen is the WM's to grant",
        );
    } else {
        let width = fresh
            .first()
            .map(|wid| {
                for _ in 0..20 {
                    let width = x11::geometry_width(&crate::proc::capture_lenient(
                        "xdotool",
                        &["getwindowgeometry", wid],
                    ))
                    .unwrap_or(0);
                    if width >= screen_width {
                        return width;
                    }
                    sleep(Duration::from_millis(250));
                }
                0
            })
            .unwrap_or(0);
        checks.item(
            "the handed-over window comes up fullscreen",
            width >= screen_width,
            format!("width {width} on a {screen_width}px screen"),
        );
    }

    // Item 1: the profile is *named*, so a name that is not a built-in
    // profile has to be refused. A silent fallback to the default would
    // have the harness screenshot the wrong picture under the right file
    // name, which is the one failure mode an eval harness cannot have.
    let bogus = run_capturing(
        binary,
        &["--default-settings", "--profile", "No Such Profile"],
        &scratch,
    )?;
    checks.item(
        "an unknown --profile is refused rather than silently defaulted",
        bogus.0.is_some_and(|code| code != 0),
        format!("exit {:?}", bogus.0),
    );

    // Multi-window: the windows are distinct X windows, not one
    // window reported twice.
    let mut unique = windows.clone();
    unique.sort();
    unique.dedup();
    checks.item(
        "the windows are distinct",
        unique.len() == windows.len(),
        format!("{unique:?}"),
    );

    // Item 2 says WM_CLASS is the binary's *own* basename, which is a
    // claim about identity, not just a string: a copy under another name
    // has to be a separate application, with its own class and its own
    // instance lock, so it can run beside the stock binary rather than
    // handing its window to it. Checked by running a renamed copy while
    // the first instance is up and seeing it become an instance of its
    // own.
    let alias_name = format!("{identity}-alias");
    let alias = scratch.dir.path().join(&alias_name);
    std::fs::copy(binary, &alias).context("copying the binary under another name")?;
    let mut alias_command = Command::new(&alias);
    alias_command.arg("--default-settings").stdin(Stdio::null());
    scratch.apply(&mut alias_command);
    let alias_child = alias_command
        .spawn()
        .with_context(|| format!("spawning {}", alias.display()))?;
    let alias_pid = alias_child.id();
    let alias_running = Running(alias_child);
    let alias_windows = x11::wait_for_windows(&alias_name, alias_pid, 1, Duration::from_secs(30));
    checks.item(
        "a renamed copy is its own application, not a handoff to the first",
        !alias_windows.is_empty(),
        format!(
            "class {alias_name:?}, pid {alias_pid}, {} window(s)",
            alias_windows.len()
        ),
    );
    drop(alias_running);

    drop(app);
    // Give the app a moment to unlink its socket before the scratch
    // directory goes, so a leak here shows up as a failure of the next
    // run rather than a mystery.
    sleep(Duration::from_millis(200));

    checks.report();
    if checks.failed() {
        bail!("contract not honored");
    }
    println!("CONTRACT OK");
    Ok(())
}

/// Runs the binary to completion with the scratch environment, returning
/// its exit code and combined output.
fn run_capturing(binary: &Path, args: &[&str], scratch: &Scratch) -> Result<(Option<i32>, String)> {
    let mut command = Command::new(binary);
    command.args(args).stdin(Stdio::null());
    scratch.apply(&mut command);
    let output = command
        .output()
        .with_context(|| format!("spawning {} {args:?}", binary.display()))?;
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    Ok((output.status.code(), text))
}

/// A checked item's verdict. `Skip` exists so the report can say what the
/// environment prevented it from checking: a harness that quietly drops
/// an item it could not run reads, later, as an item that passed.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Ok,
    Fail,
    Skip,
}

#[derive(Default)]
struct Checks {
    lines: Vec<(Verdict, String, String)>,
}

impl Checks {
    fn item(&mut self, what: &str, ok: bool, detail: String) {
        let verdict = if ok { Verdict::Ok } else { Verdict::Fail };
        self.lines.push((verdict, what.to_string(), detail));
    }

    fn skip(&mut self, what: &str, why: &str) {
        self.lines
            .push((Verdict::Skip, what.to_string(), why.to_string()));
    }

    fn failed(&self) -> bool {
        self.lines
            .iter()
            .any(|(verdict, _, _)| *verdict == Verdict::Fail)
    }

    fn report(&self) {
        for (index, (verdict, what, detail)) in self.lines.iter().enumerate() {
            let tag = match verdict {
                Verdict::Ok => "ok",
                Verdict::Fail => "FAIL",
                Verdict::Skip => "skip",
            };
            println!("{:>2}. [{tag}] {what}\n      {detail}", index + 1);
        }
    }
}
