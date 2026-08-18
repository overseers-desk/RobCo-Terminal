//! The keytab's scroll actions, end to end through the surface the binary runs.
//!
//! `app::input` already pins which key produces which `KeyAction`, and
//! `term::viewport` already pins what a `Scroll` does to rio-vt's display
//! offset. What neither can say is that the two are wired to each other: for
//! a stretch of this rebuild the surface decoded `scrollPageUp` and then
//! dropped it, because the viewport it belonged to had not landed. These
//! drive the whole path -- a scripted child fills the scrollback, and then
//! it is only key presses.
//!
//! Same harness as `pointer.rs`, minus the window: `TerminalSurface::headless`
//! is the surface with no swapchain, and `key_input` is `key_pressed` with
//! winit's own `KeyEvent` peeled off (that type cannot be built outside winit).

use std::time::{Duration, Instant};

use app::window::TerminalSurface;
use term::{CellSize, SessionConfig, Viewport};
use winit::keyboard::ModifiersState;
use winit::keyboard::{Key, NamedKey};

const CELL_W: f64 = 9.0;
const CELL_H: f64 = 18.0;
const COLS: u32 = 80;
const ROWS: u32 = 10;

/// Sixty numbered lines into a ten-row window, so there is a scrollback to
/// move through and every row says which one it is.
const SIXTY_LINES: &str = "i=1; while [ $i -le 60 ]; do echo line$i; i=$((i+1)); done";

fn scripted(script: &str) -> SessionConfig {
    SessionConfig {
        program: Some("/bin/sh".to_string()),
        args: vec!["-c".to_string(), format!("{script}; sleep 10")],
        working_directory: None,
        env: vec![
            ("TERM".to_string(), "xterm-256color".to_string()),
            ("ENV".to_string(), String::new()),
        ],
        scrollback: 1000,
    }
}

fn surface(script: &str) -> TerminalSurface {
    let viewport = Viewport::new(
        COLS * CELL_W as u32,
        ROWS * CELL_H as u32,
        1.0,
        CellSize::new(CELL_W as f32, CELL_H as f32),
    );
    TerminalSurface::headless(&scripted(script), viewport)
}

/// Pump until the screen says `text`, or fail with what it did say.
fn wait_for_screen(surface: &mut TerminalSurface, text: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        surface.pump();
        if surface.viewport_text().iter().any(|l| l.contains(text)) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!(
        "timed out waiting for {text:?}\n--- screen ---\n{}",
        surface.viewport_text().join("\n")
    );
}

fn press(surface: &mut TerminalSurface, key: NamedKey, modifiers: ModifiersState) {
    surface.key_input(&Key::Named(key), None, modifiers);
}

/// `default.keytab`: `key Prior +Shift-AppScreen : scrollPageUp`. The view
/// moves back by a screenful and the rows on it change.
#[test]
fn shift_page_up_moves_the_viewport_back_through_the_scrollback() {
    let mut surface = surface(SIXTY_LINES);
    wait_for_screen(&mut surface, "line60");

    let before = surface.viewport_text();
    assert_eq!(surface.scroll_offset(), 0, "the view starts at the bottom");

    press(&mut surface, NamedKey::PageUp, ModifiersState::SHIFT);

    let offset = surface.scroll_offset();
    assert!(offset > 0, "Shift+PageUp left the view at the live bottom");
    let after = surface.viewport_text();
    assert_ne!(after, before, "the view moved but shows the same rows");
    assert!(
        after.iter().any(|l| l.contains("line5")),
        "a page back from line60 in a ten-row window should show the fifties\n{}",
        after.join("\n")
    );

    // ...and Shift+PageDown brings it back to following the live output,
    // exactly as scrolling the wheel back down does.
    press(&mut surface, NamedKey::PageDown, ModifiersState::SHIFT);
    assert_eq!(surface.scroll_offset(), 0);
    assert_eq!(surface.viewport_text(), before);
}

/// `key Up +Shift-AppScreen : scrollLineUp`, and its partner going down. One
/// line each, the sign the wheel uses.
#[test]
fn shift_up_and_down_move_the_viewport_one_line() {
    let mut surface = surface(SIXTY_LINES);
    wait_for_screen(&mut surface, "line60");

    press(&mut surface, NamedKey::ArrowUp, ModifiersState::SHIFT);
    assert_eq!(surface.scroll_offset(), 1);
    press(&mut surface, NamedKey::ArrowUp, ModifiersState::SHIFT);
    assert_eq!(surface.scroll_offset(), 2);
    press(&mut surface, NamedKey::ArrowDown, ModifiersState::SHIFT);
    assert_eq!(surface.scroll_offset(), 1);
}

/// The other half of the same wiring: a key the keytab binds to *bytes* leaves
/// the view exactly where it was. Shift+PageUp moves the viewport because the
/// keytab says so, not because the surface swallows shifted keys.
#[test]
fn a_key_the_keytab_binds_to_bytes_moves_nothing() {
    let mut surface = surface(SIXTY_LINES);
    wait_for_screen(&mut surface, "line60");

    press(&mut surface, NamedKey::PageUp, ModifiersState::empty());
    assert_eq!(
        surface.scroll_offset(),
        0,
        "unshifted PageUp is the program's key, not the viewport's"
    );
}
