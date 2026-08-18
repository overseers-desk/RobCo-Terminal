//! The `--profile` flag, judged at the glass rather than at the log line.
//!
//! `profile_cli.rs` proves the flag is parsed and resolved; it reads the
//! `look:` line the binary prints, which is printed from the resolved config
//! before any window exists. That is exactly the half that was already
//! working when the flag reached nothing else: under `--default-settings`
//! there is no settings handle to attach, and the surface fell back to
//! `Config::default()` on every frame, so `--default-settings --profile
//! <name>` -- the pair `xtask snap` screenshots by -- filed the shipped
//! default under whatever name was asked for.
//!
//! So this test looks at pixels. Two profiles whose backgrounds differ must
//! not paint the same glass.
//!
//! Each launch gets its **own** Xvfb, one window on it and nothing else. That
//! is not fastidiousness: with two windows on one display and no window
//! manager, X keeps no backing store for an occluded window, so `import
//! -window` on the covered one returns whatever is in front of it -- a
//! comparison that "passes" while measuring the wrong window. One window per
//! display is what makes the reading mean anything.
//!
//! Needs Xvfb, xdotool and ImageMagick, and a GPU the binary can reach;
//! skips without them, the way the tmux tests skip without tmux.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const BINARY: &str = env!("CARGO_BIN_EXE_robco-term");

/// Backgrounds that cannot be confused: `Default Amber` is `#000000`,
/// `Commodore 64` is `#3b3b8f`. Both are shipped screen presets, so neither
/// needs a config file to exist -- which matters, because the flag pair under
/// test is the one that reads no user config at all.
const DARK: &str = "Default Amber";
const BLUE: &str = "Commodore 64";

fn have(tool: &str) -> bool {
    Command::new("which")
        .arg(tool)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn tools_present() -> bool {
    for tool in ["Xvfb", "xdotool", "import", "magick"] {
        if !have(tool) {
            eprintln!("skipping: no {tool}");
            return false;
        }
    }
    true
}

/// An Xvfb on a display number nobody else in this test binary is using,
/// killed when it goes out of scope.
struct Display {
    number: u32,
    child: std::process::Child,
}

impl Display {
    fn start(number: u32) -> Option<Display> {
        let child = Command::new("Xvfb")
            .arg(format!(":{number}"))
            .args(["-screen", "0", "1200x800x24", "-nolisten", "tcp"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let display = Display { number, child };
        // Wait for the server to answer rather than sleeping a guess at it.
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if Command::new("xdotool")
                .arg("getdisplaygeometry")
                .env("DISPLAY", display.name())
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                return Some(display);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        None
    }

    fn name(&self) -> String {
        format!(":{}", self.number)
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Launch the binary under `profile` on its own display and answer the mean
/// RGB of a patch in the middle of its window, which on this appliance is
/// glass and nothing else.
fn glass_tint(display_number: u32, scratch: &Path, profile: &str) -> Option<[f64; 3]> {
    let display = Display::start(display_number)?;
    let name = display.name();

    let run = scratch.join(format!("run{display_number}"));
    std::fs::create_dir_all(&run).ok()?;
    let home = scratch.join(format!("home{display_number}"));
    std::fs::create_dir_all(&home).ok()?;

    let mut child = Command::new(BINARY)
        .args(["--default-settings", "--profile", profile])
        .env("DISPLAY", &name)
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_DATA_HOME", home.join("data"))
        .env("XDG_CACHE_HOME", home.join("cache"))
        .env("XDG_RUNTIME_DIR", &run)
        .env_remove("WAYLAND_DISPLAY")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let tint = read_window(&name, scratch, profile);
    let _ = child.kill();
    let _ = child.wait();
    tint
}

/// Wait for the one window on this display, screenshot it, and average a
/// central patch.
fn read_window(display: &str, scratch: &Path, profile: &str) -> Option<[f64; 3]> {
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut wid = None;
    while Instant::now() < deadline && wid.is_none() {
        let out = Command::new("xdotool")
            .args(["search", "--class", "robco-term"])
            .env("DISPLAY", display)
            .output()
            .ok()?;
        wid = String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::to_string)
            .next_back();
        if wid.is_none() {
            std::thread::sleep(Duration::from_millis(250));
        }
    }
    let wid = wid?;
    // The window exists before the first frame reaches it; give the chain a
    // moment to paint rather than photographing an unpainted surface.
    std::thread::sleep(Duration::from_millis(2500));

    let shot = scratch.join(format!("{}.png", profile.replace(' ', "-")));
    let path = shot.display().to_string();
    Command::new("import")
        .args(["-window", &wid, &path])
        .env("DISPLAY", display)
        .status()
        .ok()
        .filter(|s| s.success())?;

    let out = Command::new("magick")
        .args([
            &path,
            "-gravity",
            "center",
            "-crop",
            "200x120+0+0",
            "+repage",
            "-format",
            "%[fx:mean.r*255] %[fx:mean.g*255] %[fx:mean.b*255]",
            "info:",
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let text = String::from_utf8_lossy(&out.stdout);
    let values: Vec<f64> = text
        .split_whitespace()
        .filter_map(|v| v.parse().ok())
        .collect();
    (values.len() == 3).then(|| [values[0], values[1], values[2]])
}

#[test]
fn a_named_profile_reaches_the_glass_under_default_settings() {
    if !tools_present() {
        return;
    }
    let scratch = tempfile::tempdir().expect("scratch dir");
    let dir: PathBuf = scratch.path().to_path_buf();

    let Some(dark) = glass_tint(71, &dir, DARK) else {
        eprintln!("skipping: could not stand a window up for {DARK:?}");
        return;
    };
    let Some(blue) = glass_tint(72, &dir, BLUE) else {
        eprintln!("skipping: could not stand a window up for {BLUE:?}");
        return;
    };

    // `Commodore 64`'s `#3b3b8f` is blue: more blue than red, and by a
    // margin no amount of glow accounts for. `Default Amber`'s `#000000`
    // under an amber phosphor is the opposite. Asserting the *direction*
    // rather than only a distance is what makes this a statement about the
    // profile reaching the glass rather than about two pictures differing.
    assert!(
        blue[2] > blue[0] + 20.0,
        "{BLUE:?} did not paint a blue screen: {blue:?}"
    );
    assert!(
        dark[2] <= dark[0] + 20.0,
        "{DARK:?} painted a blue screen: {dark:?}"
    );

    let apart: f64 = (0..3).map(|i| (blue[i] - dark[i]).abs()).sum();
    assert!(
        apart > 40.0,
        "the two profiles painted the same glass, so --profile reached the \
         log line and nothing else: {DARK:?} {dark:?} vs {BLUE:?} {blue:?}"
    );
}
