//! The X11 questions both `snap` and `verify` ask about a running
//! binary's windows, asked once.
//!
//! Everything here goes through `xdotool` and `xprop`: driving those tools
//! beats reimplementing X11 protocol handling in Rust.

use anyhow::{bail, Context, Result};
use std::thread::sleep;
use std::time::Duration;

use crate::proc::{capture, capture_lenient};

/// Every window of `class` that belongs to `pid` and is bigger than a
/// stray helper (geometry width > 100).
///
/// Contract item 2: the class is the binary's own basename, and a helper
/// window is tolerated as long as a real one is there, which is what the
/// width test screens out.
pub fn sizable_windows(class: &str, pid: u32) -> Vec<String> {
    let mut found = Vec::new();
    for cand in capture_lenient("xdotool", &["search", "--class", class]).lines() {
        let window_pid: u32 = capture_lenient("xdotool", &["getwindowpid", cand])
            .trim()
            .parse()
            .unwrap_or(0);
        if window_pid != pid {
            continue;
        }
        let geometry = capture_lenient("xdotool", &["getwindowgeometry", cand]);
        if geometry_width(&geometry).unwrap_or(0) > 100 {
            found.push(cand.to_string());
        }
    }
    found
}

/// Polls [`sizable_windows`] until at least `wanted` of them exist, or
/// the timeout lapses. Returns whatever the last poll saw, so a caller
/// can report the count it actually got.
pub fn wait_for_windows(class: &str, pid: u32, wanted: usize, timeout: Duration) -> Vec<String> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let windows = sizable_windows(class, pid);
        if windows.len() >= wanted || std::time::Instant::now() >= deadline {
            return windows;
        }
        sleep(Duration::from_millis(500));
    }
}

/// Extracts the width from an `xdotool getwindowgeometry` line like
/// `  Geometry: 1448x1086`.
pub fn geometry_width(out: &str) -> Option<u32> {
    for line in out.lines() {
        if let Some(rest) = line.trim().strip_prefix("Geometry: ") {
            return rest.split('x').next()?.parse().ok();
        }
    }
    None
}

/// Parses `xdotool getwindowgeometry --shell` output (`KEY=VALUE` lines)
/// for X and Y.
pub fn shell_xy(out: &str) -> Option<(i64, i64)> {
    let mut x = None;
    let mut y = None;
    for line in out.lines() {
        if let Some(v) = line.strip_prefix("X=") {
            x = v.trim().parse().ok();
        } else if let Some(v) = line.strip_prefix("Y=") {
            y = v.trim().parse().ok();
        }
    }
    Some((x?, y?))
}

/// The window's program-specified minimum size, as `xprop -id <wid>
/// WM_NORMAL_HINTS` reports it. This is the instrument contract item 4
/// names, so the parsing lives here rather than at either call site.
pub fn min_size_hint(wid: &str) -> Result<(i64, i64)> {
    let xprop_out =
        capture("xprop", &["-id", wid, "WM_NORMAL_HINTS"]).context("reading WM_NORMAL_HINTS")?;
    for line in xprop_out.lines() {
        if let Some(idx) = line.find("program specified minimum size: ") {
            let rest = &line[idx + "program specified minimum size: ".len()..];
            let mut numbers = rest.split(" by ").map(|part| {
                part.trim()
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse::<i64>()
            });
            let width = numbers.next().and_then(Result::ok);
            let height = numbers.next().and_then(Result::ok);
            if let (Some(width), Some(height)) = (width, height) {
                return Ok((width, height));
            }
        }
    }
    bail!("no program specified minimum size in WM_NORMAL_HINTS of {wid}");
}

/// The title the window advertises (`WM_NAME`).
pub fn window_name(wid: &str) -> String {
    capture_lenient("xdotool", &["getwindowname", wid])
        .trim()
        .to_string()
}

/// Whether a window manager is running on the current display.
///
/// It matters because the properties a WM owns cannot be checked without
/// one: fullscreen, in particular, is a request a client makes of the WM
/// (`_NET_WM_STATE_FULLSCREEN`), so under a bare Xvfb with no WM every
/// toolkit's fullscreen call is a no-op and the window keeps its size.
/// A harness that asserted fullscreen geometry there would be testing the
/// absence of a window manager, not the binary.
pub fn window_manager_present() -> bool {
    let root = capture_lenient("xprop", &["-root", "_NET_SUPPORTING_WM_CHECK"]);
    root.contains("window id #") && !root.contains("not found")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_width_reads_xdotools_wording() {
        let out = "Window 12345\n  Position: 0,0 (screen: 0)\n  Geometry: 1448x1086\n";
        assert_eq!(geometry_width(out), Some(1448));
        assert_eq!(geometry_width("nothing here"), None);
    }

    #[test]
    fn shell_xy_reads_the_key_value_form() {
        let out = "WINDOW=1234\nX=17\nY=42\nWIDTH=800\nHEIGHT=600\n";
        assert_eq!(shell_xy(out), Some((17, 42)));
    }
}
