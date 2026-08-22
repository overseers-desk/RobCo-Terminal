//! `Ctrl+Shift+C` and `Ctrl+Shift+V` end to end through the surface the binary
//! runs.
//!
//! The clipboard itself needs a display, and CI has none: `TerminalSurface`
//! skips the platform call when it holds no window, so what these can pin is
//! the half that has nothing to do with the platform. The shortcut layer
//! claims both chords, so neither reaches the child as text. Before the layer
//! bound them, `Ctrl+Shift+V` fell through to winit's decoded text and typed
//! a stray byte into the shell, which is the failure worth a test: a paste
//! that does nothing is a nuisance, and a paste that types `\x16` at a prompt
//! is a wrong command.
//!
//! Same harness as `channel_bank.rs`, minus the cabinet: these keys are the
//! window's own and stand whether or not the bank is on show.

use std::time::{Duration, Instant};

use app::window::TerminalSurface;
use term::{CellSize, SessionConfig, Viewport};
use winit::keyboard::{Key, ModifiersState, NamedKey};

const CELL_W: f32 = 9.0;
const CELL_H: f32 = 18.0;
const WINDOW_W: u32 = 720;
const WINDOW_H: u32 = 400;

const CTRL_SHIFT: ModifiersState = ModifiersState::CONTROL.union(ModifiersState::SHIFT);

/// A shell that reads a line and prints it back inside brackets, so the screen
/// says exactly what reached the pty and where the line ended.
fn scripted() -> SessionConfig {
    SessionConfig {
        program: Some("/bin/sh".to_string()),
        args: vec![
            "-c".to_string(),
            "echo ready; while IFS= read -r l; do echo \"[$l]\"; done".to_string(),
            String::new(),
        ],
        working_directory: None,
        env: vec![
            ("TERM".to_string(), "xterm-256color".to_string()),
            ("ENV".to_string(), String::new()),
        ],
        scrollback: 200,
    }
}

fn surface() -> TerminalSurface {
    let viewport = Viewport::new(WINDOW_W, WINDOW_H, 1.0, CellSize::new(CELL_W, CELL_H));
    TerminalSurface::headless(&scripted(), viewport)
}

fn wait_for(surface: &mut TerminalSurface, text: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        surface.pump();
        if let Some(line) = surface.viewport_text().iter().find(|l| l.contains(text)) {
            return line.trim().to_string();
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!(
        "timed out waiting for {text:?}\n--- screen ---\n{}",
        surface.viewport_text().join("\n")
    );
}

fn character(surface: &mut TerminalSurface, c: &str, modifiers: ModifiersState) {
    surface.key_input(&Key::Character(c.into()), Some(c), modifiers);
}

#[test]
fn the_clipboard_chords_reach_the_child_as_nothing() {
    let mut surface = surface();
    wait_for(&mut surface, "ready");

    character(&mut surface, "a", ModifiersState::empty());
    // Both chords, in the middle of a half-typed line. X11 hands the shifted
    // letter up in upper case, which is what a real keyboard sends here.
    character(&mut surface, "C", CTRL_SHIFT);
    character(&mut surface, "V", CTRL_SHIFT);
    character(&mut surface, "b", ModifiersState::empty());
    surface.key_input(&Key::Named(NamedKey::Enter), None, ModifiersState::empty());

    assert_eq!(
        wait_for(&mut surface, "["),
        "[ab]",
        "the two chords put something on the wire"
    );
}
