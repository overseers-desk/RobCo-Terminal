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

use app::clipboard::Target;
use app::shell::Surface;
use app::window::TerminalSurface;
use term::{CellSize, SessionConfig, Viewport};
use winit::dpi::PhysicalPosition;
use winit::event::{MouseButton, MouseScrollDelta};
use winit::keyboard::{Key, ModifiersState, NamedKey};

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
        grapheme_clustering: false,
    }
    rate: None,
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

/// Let a wheel glide arrive: tick the surface, as the shell's loop would,
/// until the view stops moving. A notch sets the view gliding over
/// `term::viewport::WHEEL_GLIDE`; the offset it settles on is the one the
/// notch asked for.
fn settle(surface: &mut TerminalSurface) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        surface.tick();
        if !surface.is_gliding() {
            return;
        }
        assert!(Instant::now() < deadline, "the wheel glide never arrived");
        std::thread::sleep(Duration::from_millis(4));
    }
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

/// Control with the left button is macOS's secondary click, so there the
/// same gesture never marks anything: the press goes where a right press
/// goes, which is the settings application, and the drag that follows has no
/// selection to extend. Everywhere else Control is just a modifier held over
/// an ordinary drag, and the run comes back joined.
///
/// Driven through the real `mouse_pressed`/`mouse_released` rather than the
/// routing table one crate down, because the substitution that makes the two
/// platforms differ lives here.
#[test]
fn a_control_drag_marks_nothing_on_macos_and_copies_a_run_elsewhere() {
    let mut surface = surface("printf 'hello world\\n'", 24);
    wait_for_screen(&mut surface, "hello world");

    let control = ModifiersState::CONTROL;
    surface.mouse_pressed(MouseButton::Left, at(0, 0), control);
    surface.cursor_moved(at(11, 0), control);
    surface.mouse_released(MouseButton::Left, at(11, 0), control);

    let expected = if cfg!(target_os = "macos") {
        None
    } else {
        Some("hello world")
    };
    assert_eq!(surface.last_selection(), expected);
}

/// A window that loses focus never sees the button come up, so the press
/// that was in the air is over: the ordinary press after it is ordinary at
/// both ends. Without the reset, the next release would go out as a right
/// one, on macOS where the substitution happens and nowhere else.
#[test]
fn a_press_interrupted_by_losing_focus_does_not_colour_the_next_one() {
    let mut surface = surface("printf 'hello world\\n'", 24);
    wait_for_screen(&mut surface, "hello world");

    // Away from the cell the drag below starts on: two presses on one cell
    // inside the double-click window are a double click, which is a different
    // gesture and not what this is about.
    surface.mouse_pressed(MouseButton::Left, at(20, 0), ModifiersState::CONTROL);
    surface.focus_changed(false);
    surface.focus_changed(true);

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

/// One wheel notch sends the view three lines back into history, gliding
/// there over a few frames; once it arrives the rows on screen have moved
/// down by exactly that much.
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
    assert!(surface.is_gliding(), "a notch sets the view gliding");
    settle(&mut surface);

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
    settle(&mut surface);
    assert_eq!(surface.scroll_offset(), 6);

    surface.mouse_wheel(MouseScrollDelta::LineDelta(0.0, -2.0), none());
    settle(&mut surface);
    assert_eq!(surface.scroll_offset(), 0);
    assert_eq!(surface.viewport_text(), bottom);
}

/// A trackpad's pixels move the view as they come, by fractions of a row:
/// half a cell up is half a row back, held as one line with the picture
/// shifted half a row; a point on the glass maps to the cell drawn under it.
#[test]
fn trackpad_pixels_scroll_the_view_by_fractions_of_a_row() {
    let mut surface = surface(
        "i=1; while [ $i -le 60 ]; do echo line$i; i=$((i+1)); done",
        10,
    );
    wait_for_screen(&mut surface, "line60");
    let bottom = surface.viewport_text();

    surface.mouse_wheel(
        MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, CELL_H / 2.0)),
        none(),
    );
    assert!(
        !surface.is_gliding(),
        "pixels move the view, they do not glide"
    );
    assert_eq!(surface.scroll_offset(), 1, "half a row back holds one line");
    let half = surface.viewport_text();
    assert_eq!(half[1], bottom[0], "one line held: the rows moved down one");

    // With the picture shifted up half a row, a press in the top half of a
    // cell lands on the line drawn there, which is the row below the
    // unshifted one: the selection says which text the pointer was over.
    surface.mouse_pressed(MouseButton::Left, at(0, 0), none());
    surface.cursor_moved(at(5, 0), none());
    surface.mouse_released(MouseButton::Left, at(5, 0), none());
    let picked = surface.last_selection().unwrap_or_default().to_string();
    assert_eq!(
        picked,
        half[1][..picked.len()],
        "selected on the row under the pointer"
    );

    surface.mouse_wheel(
        MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, -CELL_H / 2.0)),
        none(),
    );
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

/// A shell that says `ready` and then reads a line at a time, echoing it in
/// brackets: what reached the pty, and where the line ended.
fn reader() -> &'static str {
    "printf 'hello world\\n'; while IFS= read -r l; do echo \"[$l]\"; done"
}

/// The whole primary-selection round trip: drag a run, middle-click, and the
/// run arrives at the child as typing. Nothing in between touches a display.
#[test]
fn a_middle_click_types_the_primary_selection_at_the_child() {
    let mut surface = surface(reader(), 24);
    wait_for_screen(&mut surface, "hello world");

    surface.mouse_pressed(MouseButton::Left, at(0, 0), none());
    surface.cursor_moved(at(5, 0), none());
    surface.mouse_released(MouseButton::Left, at(5, 0), none());
    assert_eq!(surface.last_selection(), Some("hello"));

    surface.mouse_pressed(MouseButton::Middle, at(0, 2), none());
    surface.key_input(&Key::Named(NamedKey::Enter), None, none());

    wait_for_screen(&mut surface, "[hello]");
}

/// Selecting fills the primary selection and leaves the clipboard where it
/// was. The two slots are the point of the pair, and a headless surface's
/// store is where both of them live.
#[test]
fn a_drag_writes_the_primary_selection_and_leaves_the_clipboard_alone() {
    let mut surface = surface("printf 'hello world\\n'", 24);
    wait_for_screen(&mut surface, "hello world");

    surface.mouse_pressed(MouseButton::Left, at(0, 0), none());
    surface.cursor_moved(at(5, 0), none());
    surface.mouse_released(MouseButton::Left, at(5, 0), none());

    let store = surface.clipboard_store();
    assert_eq!(store.last(Target::Primary), Some("hello"));
    assert_eq!(
        store.last(Target::Clipboard),
        None,
        "a selection must not cost the user what they had copied"
    );
}

/// Three presses on one cell take the whole line, without a drag.
#[test]
fn a_triple_click_takes_the_whole_line() {
    let mut surface = surface("printf 'hello world\\n'", 24);
    wait_for_screen(&mut surface, "hello world");

    for _ in 0..3 {
        surface.mouse_pressed(MouseButton::Left, at(7, 0), none());
        surface.mouse_released(MouseButton::Left, at(7, 0), none());
    }

    assert_eq!(
        surface.last_selection(),
        Some("hello world\n"),
        "a whole line carries the line ending, so pasting it enters the command"
    );
}

/// A fourth press on the same cell is a new click, not a fourth stage: the
/// run restarts, and the drag that follows selects what it crossed.
#[test]
fn a_fourth_press_starts_a_fresh_selection() {
    let mut surface = surface("printf 'hello world\\n'", 24);
    wait_for_screen(&mut surface, "hello world");

    for _ in 0..3 {
        surface.mouse_pressed(MouseButton::Left, at(0, 0), none());
        surface.mouse_released(MouseButton::Left, at(0, 0), none());
    }
    assert_eq!(
        surface.last_selection(),
        Some("hello world\n"),
        "a whole line carries the line ending, so pasting it enters the command"
    );

    surface.mouse_pressed(MouseButton::Left, at(0, 0), none());
    surface.cursor_moved(at(5, 0), none());
    surface.mouse_released(MouseButton::Left, at(5, 0), none());

    assert_eq!(surface.last_selection(), Some("hello"));
}
