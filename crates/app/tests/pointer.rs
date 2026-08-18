//! The pointer path end to end, through the surface the binary runs.
//!
//! Every piece under here is already tested one crate down: the
//! distortion transform against its own closed-form math, the selection
//! arithmetic against Konsole's, the scroll policy against rio-vt's
//! display offset. What is not tested down there is that they are wired
//! to each other and to a real session, which is what these drive: a
//! scripted child writes a line, and then it is only presses, moves and
//! wheel notches in window pixels.
//!
//! No window and no display. `TerminalSurface::headless` is the same
//! surface minus the two things the pointer path never asks (how big the
//! window is, and what to draw), so this is an ordinary `cargo test`.

use std::time::{Duration, Instant};

use app::shell::Surface;
use app::window::TerminalSurface;
use term::{CellSize, SessionConfig, Viewport};
use winit::dpi::PhysicalPosition;
use winit::event::{MouseButton, MouseScrollDelta};
use winit::keyboard::ModifiersState;

const CELL_W: f64 = 9.0;
const CELL_H: f64 = 18.0;
const COLS: u32 = 80;

/// A child that writes a fixed screen and then holds the pty open, so
/// the grid is a function of the script and nothing else.
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

fn surface(script: &str, rows: u32) -> TerminalSurface {
    let viewport = Viewport::new(
        COLS * CELL_W as u32,
        rows * CELL_H as u32,
        1.0,
        CellSize::new(CELL_W as f32, CELL_H as f32),
    );
    TerminalSurface::headless(&scripted(script), viewport)
}

/// The middle of a cell, in window pixels: what the shell would hand the
/// surface if the pointer were there.
fn at(column: u32, row: u32) -> PhysicalPosition<f64> {
    PhysicalPosition::new(
        f64::from(column) * CELL_W + CELL_W / 2.0,
        f64::from(row) * CELL_H + CELL_H / 2.0,
    )
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

fn none() -> ModifiersState {
    ModifiersState::empty()
}

/// The whole gesture: press on the first cell of a line, drag across it,
/// let go. What comes back is what Konsole would have copied.
#[test]
fn press_drag_release_selects_the_dragged_run() {
    let mut surface = surface("printf 'hello world\\n'", 24);
    wait_for_screen(&mut surface, "hello world");

    surface.mouse_pressed(MouseButton::Left, at(0, 0), none());
    surface.cursor_moved(at(11, 0), none());
    surface.mouse_released(MouseButton::Left, at(11, 0), none());

    assert_eq!(surface.last_selection(), Some("hello world"));
}

/// A drag that stops short of the end of the word stops short in the
/// text too: the selection is the cells the pointer crossed, not the
/// word it started in.
#[test]
fn a_short_drag_selects_only_what_it_crossed() {
    let mut surface = surface("printf 'hello world\\n'", 24);
    wait_for_screen(&mut surface, "hello world");

    surface.mouse_pressed(MouseButton::Left, at(0, 0), none());
    surface.cursor_moved(at(5, 0), none());
    surface.mouse_released(MouseButton::Left, at(5, 0), none());

    assert_eq!(surface.last_selection(), Some("hello"));
}

/// Two presses on the same cell inside the double-click window take the
/// whole word, without a drag.
#[test]
fn a_double_click_takes_the_word_under_the_pointer() {
    let mut surface = surface("printf 'hello world\\n'", 24);
    wait_for_screen(&mut surface, "hello world");

    surface.mouse_pressed(MouseButton::Left, at(7, 0), none());
    surface.mouse_released(MouseButton::Left, at(7, 0), none());
    surface.mouse_pressed(MouseButton::Left, at(7, 0), none());

    assert_eq!(surface.last_selection(), Some("world"));
}

/// One wheel notch moves the view three lines back into history, and the
/// rows on screen move down by exactly that much.
#[test]
fn a_wheel_notch_scrolls_the_view_three_lines_back() {
    let mut surface = surface(
        "i=1; while [ $i -le 60 ]; do echo line$i; i=$((i+1)); done",
        10,
    );
    wait_for_screen(&mut surface, "line60");

    let before = surface.viewport_text();
    assert_eq!(surface.scroll_offset(), 0, "the view starts at the bottom");

    surface.mouse_wheel(MouseScrollDelta::LineDelta(0.0, 1.0), none());

    let after = surface.viewport_text();
    assert_eq!(surface.scroll_offset(), 3, "one notch is three lines");
    assert_eq!(
        after[3], before[0],
        "the row that was at the top is three rows lower"
    );
    assert_ne!(after[0], before[0], "and something older is above it");
}

/// Scrolling back down again returns to following the live output.
#[test]
fn scrolling_back_down_re_follows_the_output() {
    let mut surface = surface(
        "i=1; while [ $i -le 60 ]; do echo line$i; i=$((i+1)); done",
        10,
    );
    wait_for_screen(&mut surface, "line60");
    let bottom = surface.viewport_text();

    surface.mouse_wheel(MouseScrollDelta::LineDelta(0.0, 2.0), none());
    assert_eq!(surface.scroll_offset(), 6);

    surface.mouse_wheel(MouseScrollDelta::LineDelta(0.0, -2.0), none());
    assert_eq!(surface.scroll_offset(), 0);
    assert_eq!(surface.viewport_text(), bottom);
}

/// A program that turned mouse reporting on owns the pointer: the click
/// goes down the pty as an SGR report instead of marking the screen.
///
/// The pty's own line discipline echoes what we write back to us, with
/// ESC rendered `^[`, so the report is visible on the screen it was
/// never meant to reach, which is exactly what makes it readable here
/// without a cooperating child.
#[test]
fn a_click_reaches_the_program_once_it_asks_for_the_mouse() {
    // DECSET 1000 (report clicks) and 1006 (SGR encoding).
    let mut surface = surface("printf '\\033[?1000h\\033[?1006h'; sleep 10", 24);
    // The modes arrive with the first bytes the child writes.
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline && !surface.terminal_uses_mouse() {
        surface.pump();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        surface.terminal_uses_mouse(),
        "the child never turned mouse reporting on"
    );

    surface.mouse_pressed(MouseButton::Left, at(4, 2), none());
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        surface.pump();
        let screen = surface.viewport_text().join("");
        if screen.contains("[<0;5;3M") {
            assert!(
                surface.last_selection().is_none(),
                "the click reported to the program must not also mark the screen"
            );
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!(
        "the SGR report never reached the pty\n--- screen ---\n{}",
        surface.viewport_text().join("\n")
    );
}
