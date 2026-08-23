//! RobCo Terminal application.
//!
//! This crate is the process: the command line, the window shell, the
//! single-instance arbitration, the crash logger, and everything hanging
//! inside a window: the wgpu surface, the rio-vt session behind it, the
//! input path in front of it, and the live settings handle beside it.
//! The terminal itself (`crates/term`) hangs inside it, and so does the
//! CRT pass graph (`crates/crt-render`): the grid is drawn into an
//! offscreen target and the chain takes it from there to the swapchain,
//! all inside [`window::TerminalSurface`]. The cabinet around the glass is
//! `crates/chassis`: it measures the window's division into a bank column
//! and a screen well and owns the seam between them, and [`column`] is the
//! host mount that puts its casting on the frame after the chain has run,
//! composited outside the chain rather than bending with it.
//!
//! The behavior here is not a matter of taste: `cargo run -p xtask --
//! contract` prints the CLI/window contract the eval harness drives a
//! binary through, and that contract is normative.
//!
//! Module map:
//!
//! - [`cli`] -- argument parsing, including `-e`'s catch-everything rule.
//! - [`instance`] -- single-instance lock and the new-window IPC.
//! - [`shell`] -- winit event loop, multi-window, geometry hints, title.
//! - [`channels`] / [`bank`] / [`chord`] -- what a window's sessions are
//!   numbered, which page of them the bank is showing, and the digit chord
//!   that names one. The three state machines of the channel bank.
//! - [`geometry`] -- the four numbers the contract measures.
//! - [`overlay`] / [`badge`] -- the transient size badge: when it shows, and
//!   the mount that draws it over the finished frame.
//! - [`crashlog`] -- the fatal-signal backtrace logger.
//! - [`paths`] -- where per-user files live.
//! - [`window`] / [`gpu`] -- the wgpu surface and the session it drives,
//!   which is what fills a shell window ([`window::TerminalSurface`]).
//! - [`column`] -- the bank column's metal, composited over the frame the
//!   chain drew.
//! - [`input`], [`mouse`], [`clipboard`] -- keyboard encoding,
//!   mouse reporting, and copy/paste. Composed input
//!   (an IME's committed text) comes in beside the keyboard rather than
//!   through it, at [`window::TerminalSurface::ime_input`], which is also
//!   where the record of what is *not* drawn yet lives.
//! - [`distortion`] -- the inverse screen-curvature transform that turns a
//!   window pixel back into a flat-screen one before it becomes a cell.
//! - [`settings`] -- `robco-config` behind a typed, live-reloading handle.

pub mod badge;
pub mod bank;
pub mod channels;
pub mod chord;
pub mod cli;
pub mod clipboard;
pub mod column;
pub mod crashlog;
pub mod distortion;
pub mod frame_stats;
pub mod geometry;
pub mod gpu;
pub mod input;
pub mod instance;
pub mod mouse;
pub mod overlay;
pub mod paths;
pub mod settings;
pub mod shell;
pub mod tmux;
pub mod window;
pub mod workarea;

/// The application's version, as `--version` prints it.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The binary's own basename, which is the application's identity: the
/// WM_CLASS it presents, the instance lock it takes, and the directory its
/// per-user files live in. A renamed copy therefore runs alongside the
/// stock binary instead of fighting it for one lock.
///
/// The basename is taken whole, extension included: the contract's own
/// wording is `xdotool search --class $(basename BINARY)`, and shell
/// `basename` strips nothing. On Unix, where binaries have no extension,
/// stripping one would make no difference; on Windows it would, and the
/// contract's reading is the one the harness can actually check, so it
/// wins. The `.exe` suffix is dropped there, since a WM_CLASS-equivalent of
/// `robco-term.exe` would be odd.
pub fn identity() -> String {
    let exe = std::env::current_exe().ok();
    let name = exe
        .as_deref()
        .and_then(std::path::Path::file_name)
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "robco-term".to_string());
    name.strip_suffix(".exe")
        .map(str::to_string)
        .unwrap_or(name)
}
