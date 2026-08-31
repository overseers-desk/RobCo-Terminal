//! Selection across an eviction: the gesture must keep naming the text the
//! pointer covered once the scrollback ring is at capacity.
//!
//! The Konsole selection model stores absolute line indices (0 = oldest
//! line still in scrollback). Once history has reached `scrollback`, every
//! new line evicts one off the top of the ring and re-names every absolute
//! index to content one line later; output arriving between the drag and
//! the release therefore shifted the extracted text downward by exactly
//! the evicted count, until the model learned to rebase its coordinates on
//! the grid's own eviction counter (`Konsole::rebase`,
//! `Grid::lines_evicted`). These pin that end to end through the pointer
//! path, against a control run below capacity where absolutes are stable.

use std::time::{Duration, Instant};

use app::shell::Surface;
use app::window::TerminalSurface;
use term::{CellSize, SessionConfig, Viewport};
use winit::dpi::PhysicalPosition;
use winit::event::MouseButton;
use winit::keyboard::ModifiersState;

const CELL_W: f64 = 9.0;
const CELL_H: f64 = 18.0;
const COLS: u32 = 80;
const ROWS: u32 = 24;

/// A child that fills the scrollback, prints a target line, then prints
/// five more lines for every line it is fed.
fn scripted(fill: usize, scrollback: usize) -> SessionConfig {
    let script = format!(
        "i=1; while [ $i -le {fill} ]; do echo fill-$i; i=$((i+1)); done; \
         echo 'TARGET alpha beta'; \
         while IFS= read -r t; do j=1; while [ $j -le 5 ]; do echo more-$j; j=$((j+1)); done; done"
    );
    SessionConfig {
        program: Some("/bin/sh".to_string()),
        args: vec!["-c".to_string(), script],
        working_directory: None,
        env: vec![
            ("TERM".to_string(), "xterm-256color".to_string()),
            ("ENV".to_string(), String::new()),
        ],
        scrollback,
        grapheme_clustering: false,
        rate: None,
    }
}

fn surface(fill: usize, scrollback: usize) -> TerminalSurface {
    let viewport = Viewport::new(
        COLS * CELL_W as u32,
        ROWS * CELL_H as u32,
        1.0,
        CellSize::new(CELL_W as f32, CELL_H as f32),
    );
    TerminalSurface::headless(&scripted(fill, scrollback), viewport)
}

fn at(column: u32, row: u32) -> PhysicalPosition<f64> {
    PhysicalPosition::new(
        f64::from(column) * CELL_W + CELL_W / 2.0,
        f64::from(row) * CELL_H + CELL_H / 2.0,
    )
}

fn none() -> ModifiersState {
    ModifiersState::empty()
}

/// Pump until some visible row contains `text`; answer that row index.
fn wait_for(surface: &mut TerminalSurface, text: &str) -> usize {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        surface.pump();
        if let Some(row) = surface
            .viewport_text()
            .iter()
            .position(|l| l.contains(text))
        {
            return row;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!(
        "timed out waiting for {text:?}\n--- screen ---\n{}",
        surface.viewport_text().join("\n")
    );
}

fn history(surface: &TerminalSurface) -> usize {
    surface
        .channels()
        .session()
        .expect("a session on the air")
        .term()
        .history_size()
}

fn display_offset(surface: &TerminalSurface) -> usize {
    surface
        .channels()
        .session()
        .expect("a session on the air")
        .term()
        .grid
        .display_offset()
}

/// How many lines have been evicted off the top of the ring so far: the
/// counter the selection model rebases its coordinates on
/// (`Grid::lines_evicted`).
fn evicted(surface: &TerminalSurface) -> u64 {
    surface
        .channels()
        .session()
        .expect("a session on the air")
        .term()
        .grid
        .lines_evicted()
}

/// One drag over the word "alpha", with five lines of output arriving
/// between the drag and the release. `fill` decides whether the ring is at
/// capacity when they do.
struct Outcome {
    text: String,
    history_at_press: usize,
    history_at_release: usize,
    /// Lines evicted off the ring between the drag and the release.
    evicted_during_gesture: u64,
    /// How many viewport rows the target line moved up between the drag
    /// and the release: how far below the gesture the extraction read.
    target_rows_climbed: i64,
}

fn drag_alpha_while_streaming(fill: usize, scrollback: usize) -> Outcome {
    let mut surface = surface(fill, scrollback);
    let row = wait_for(&mut surface, "TARGET alpha beta") as u32;

    let history_at_press = history(&surface);
    let evicted_at_press = evicted(&surface);
    // The gesture: press on the 'a' of "alpha" (column 7), drag to one
    // past its end. Identical to pointer.rs's proven gesture arithmetic.
    let line = &surface.viewport_text()[row as usize];
    assert_eq!(
        &line[7..12],
        "alpha",
        "the harness misplaced the target word"
    );
    surface.mouse_pressed(MouseButton::Left, at(7, row), none());
    surface.cursor_moved(at(12, row), none());

    // Output arrives while the button is still down: feed the child one
    // line, which answers with five. At capacity each one evicts a line
    // off the top of the ring.
    surface.write(b"go\n");
    wait_for(&mut surface, "more-5");

    let history_at_release = history(&surface);
    assert_eq!(
        display_offset(&surface),
        0,
        "the view is live throughout; no scroll state is involved"
    );
    let row_now = wait_for(&mut surface, "TARGET alpha beta") as i64;

    // The word has scrolled up but is still on the glass; release over the
    // cell the pointer never left.
    surface.mouse_released(MouseButton::Left, at(12, row), none());
    let text = surface.last_selection().unwrap_or_default().to_string();
    Outcome {
        text,
        history_at_press,
        history_at_release,
        evicted_during_gesture: evicted(&surface) - evicted_at_press,
        target_rows_climbed: i64::from(row) - row_now,
    }
}

/// Control: with the ring far from capacity, the same gesture over the same
/// streaming child extracts the word the drag covered. Absolute indices are
/// stable while nothing has been evicted.
#[test]
fn a_drag_over_streaming_output_selects_the_dragged_word_below_capacity() {
    let outcome = drag_alpha_while_streaming(30, 1000);
    assert!(
        outcome.history_at_press < 1000,
        "the control must run below capacity"
    );
    assert_eq!(outcome.evicted_during_gesture, 0);
    assert_eq!(outcome.text, "alpha");
}

/// The gesture that used to go wrong: at scrollback capacity, extraction
/// read cells exactly as many rows below the drag as lines were evicted
/// between the drag and the release. The rebase keeps it on the word.
#[test]
fn a_drag_over_streaming_output_still_selects_the_dragged_word_at_capacity() {
    // 100 fill lines into a 50-line ring under a 24-row screen: the ring
    // is full well before the target line prints.
    let outcome = drag_alpha_while_streaming(100, 50);
    assert_eq!(
        outcome.history_at_press, 50,
        "the repro must start at capacity (history == scrollback)"
    );
    assert_eq!(
        outcome.history_at_release, 50,
        "eviction, not growth: history is pinned at the cap"
    );
    assert!(
        outcome.evicted_during_gesture > 0,
        "output between the drag and the release must evict lines"
    );
    // The renaming really happened: the target climbed the viewport by
    // exactly the evicted count. Without the rebase, the extraction below
    // came back that many rows below the word ("\n", in this layout).
    assert_eq!(
        outcome.target_rows_climbed, outcome.evicted_during_gesture as i64,
        "at capacity, each new line renames every absolute index one line later"
    );
    assert_eq!(outcome.text, "alpha");
}
