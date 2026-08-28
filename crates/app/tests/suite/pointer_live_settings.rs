//! A settings edit changes the pointer's inverse-distortion mapping without
//! restarting the process.
//!
//! Structured like `settings_live_reload.rs`: a real `SettingsHandle`
//! watching a real temp-file config, an atomic write-temp-then-rename edit,
//! and a `wait_until` poll -- except the "did it take" probe here is the
//! pointer path (`TerminalSurface`, headless per `pointer.rs`), not
//! `handle.current()` directly, since the point here is that the *pointer*
//! reads the live handle, not merely that the handle itself reloads
//! (`settings_live_reload.rs` already proved that).
//!
//! `screen.margin` is the proof key: with curvature and frame size held at
//! (approximately) zero, `term::distortion::correct_distortion` reduces to
//! `x_grid = x_widget - margin` exactly (see
//! `crates/term/src/distortion.rs`'s `zero_curvature_reduces_to_the_margin_frame_linear_map`
//! test), so a margin change shifts which grid column a fixed widget pixel
//! lands on by a predictable number of cells -- turning a settings edit
//! into a different selection.

use std::fs;
use std::sync::Arc;
use std::time::{Duration, Instant};

use app::settings::{self, SettingsHandle};
use app::shell::Surface;
use app::window::TerminalSurface;
use term::{CellSize, SessionConfig, Viewport};
use winit::dpi::PhysicalPosition;
use winit::event::MouseButton;
use winit::keyboard::ModifiersState;

const CELL_W: f64 = 9.0;
const CELL_H: f64 = 18.0;
const COLS: u32 = 40;

/// Columns 0-25 read A, B, C, ... so a selection's starting letter says
/// which column the click landed on without needing to count characters.
const ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";

fn scripted() -> SessionConfig {
    SessionConfig {
        program: Some("/bin/sh".to_string()),
        args: vec![
            "-c".to_string(),
            format!("printf '{ALPHABET}\\n'; sleep 10"),
        ],
        working_directory: None,
        env: vec![
            ("TERM".to_string(), "xterm-256color".to_string()),
            ("ENV".to_string(), String::new()),
        ],
        scrollback: 1000,
        grapheme_clustering: false,
    }
}

fn none() -> ModifiersState {
    ModifiersState::empty()
}

/// The middle of a cell, in window pixels.
fn at(column: u32, row: u32) -> PhysicalPosition<f64> {
    PhysicalPosition::new(
        f64::from(column) * CELL_W + CELL_W / 2.0,
        f64::from(row) * CELL_H + CELL_H / 2.0,
    )
}

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

fn write_atomic(path: &std::path::Path, contents: &str) {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, contents).unwrap();
    fs::rename(&tmp, path).unwrap();
}

fn wait_until(mut predicate: impl FnMut() -> bool, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if predicate() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    predicate()
}

/// One drag-select at a fixed pixel span, reporting the selected text
/// (the letters under the drag).
fn select_at_fixed_pixels(surface: &mut TerminalSurface, from_col: u32, to_col: u32) -> String {
    surface.mouse_pressed(MouseButton::Left, at(from_col, 0), none());
    surface.cursor_moved(at(to_col, 0), none());
    surface.mouse_released(MouseButton::Left, at(to_col, 0), none());
    surface
        .last_selection()
        .expect("a press-drag-release should select something")
        .to_string()
}

#[test]
fn a_config_edit_changes_the_pointer_mapping_without_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    // Chassis off, zero curvature, zero raw margin/radius: the smallest
    // margin the schema can express (`distortion_margin` still adds a
    // couple of pixels from the screen-radius term, but nowhere near a
    // cell width), so an unedited click lands on the column it looks
    // like it lands on.
    write_atomic(
        &path,
        "[general]\nchassis_shown = false\n\
         [screen]\nmargin = 0.0\nscreen_curvature = 0.0\nscreen_radius = 0.0\nframe_size = 0.0\n",
    );

    let handle =
        Arc::new(SettingsHandle::spawn(path.clone(), |_, _, _| {}).expect("watcher should start"));
    let baseline_margin = settings::distortion_margin(&handle.current());
    assert!(
        baseline_margin < CELL_W,
        "test setup: baseline margin {baseline_margin}px should be well under one cell ({CELL_W}px)"
    );

    let viewport = Viewport::new(
        COLS * CELL_W as u32,
        24 * CELL_H as u32,
        1.0,
        CellSize::new(CELL_W as f32, CELL_H as f32),
    );
    let mut surface = TerminalSurface::headless(&scripted(), viewport);
    surface.set_settings(Arc::clone(&handle));
    wait_for_screen(&mut surface, ALPHABET);

    // Baseline: clicking columns 10..15 selects "KLMNO" (letters K..O),
    // margin barely nudging anything.
    let before = select_at_fixed_pixels(&mut surface, 10, 15);
    assert_eq!(before, "KLMNO", "baseline selection at the unedited margin");

    // A large margin edit: `distortion_margin` climbs from a couple of
    // pixels to `lint(1, 40, 1.0) + ... = 40 + ...`, `lint`'s `_margin`
    // term alone growing by exactly 39px (`lint(1, 40, 1.0) - lint(1, 40,
    // 0.0) == 40 - 1`) -- more than four cells (4 * 9px = 36px), enough to
    // shift a fixed pixel span onto different columns.
    write_atomic(
        &path,
        "[general]\nchassis_shown = false\n\
         [screen]\nmargin = 1.0\nscreen_curvature = 0.0\nscreen_radius = 0.0\nframe_size = 0.0\n",
    );
    assert!(wait_until(
        || settings::distortion_margin(&handle.current()) > baseline_margin + 4.0 * CELL_W,
        Duration::from_secs(5)
    ));

    let after = select_at_fixed_pixels(&mut surface, 10, 15);
    assert_ne!(
        after, before,
        "the pointer mapping did not change after the live config edit"
    );
    assert_eq!(
        after, "FGHIJ",
        "the same pixel span should read five columns earlier once the margin grew by ~5 cells"
    );
}

/// The same click, on the same glass, at two device pixel ratios.
///
/// `DistortionParams` is entirely physical: its `width`/`height` are the
/// viewport's physical pixels and `correct_distortion` subtracts `margin`
/// from a physical pointer position. The margin setting is logical, and every
/// other place that crosses that boundary scales it (`Viewport::margin` is
/// documented as physical and `ensure_margin` multiplies by `scale_factor`) --
/// but `distortion_params` passed it straight through. On a HiDPI screen the
/// pointer was therefore measured against half the inset the glass was drawn
/// with, and the cell the click reported drifted from the cell under the
/// cursor, by more the further right you clicked.
///
/// The margin here is deliberately large (`margin = 1.0`, about 40 logical
/// pixels, more than four cells) so a half-sized inset cannot hide inside
/// one cell's width.
#[test]
fn a_click_lands_on_the_same_cell_at_dpr_1_and_dpr_2() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    write_atomic(
        &path,
        "[general]\nchassis_shown = false\n\
         [screen]\nmargin = 1.0\nscreen_curvature = 0.0\nscreen_radius = 0.0\nframe_size = 0.0\n",
    );
    let handle =
        Arc::new(SettingsHandle::spawn(path.clone(), |_, _, _| {}).expect("watcher should start"));
    let margin = settings::distortion_margin(&handle.current());
    assert!(
        margin > 4.0 * CELL_W,
        "test setup: the margin ({margin}px) must be worth more than four cells"
    );

    // One window, described twice: the same logical size and the same logical
    // cell, once at DPR 1 and once at DPR 2. Both grids are the same size in
    // cells, so the same logical point names the same cell in both -- unless
    // something crossing the logical/physical boundary forgets to.
    let logical_w = COLS * CELL_W as u32;
    let logical_h = 24 * CELL_H as u32;
    let mut selections = Vec::new();
    for scale in [1.0f64, 2.0] {
        let viewport = Viewport::new(
            (f64::from(logical_w) * scale) as u32,
            (f64::from(logical_h) * scale) as u32,
            scale,
            CellSize::new(CELL_W as f32, CELL_H as f32),
        );
        let mut surface = TerminalSurface::headless(&scripted(), viewport);
        surface.set_settings(Arc::clone(&handle));
        wait_for_screen(&mut surface, ALPHABET);

        // A logical point becomes a physical one by the ratio, which is what
        // the platform hands `cursor_moved` on a HiDPI screen.
        let click = |column: u32| {
            let p = at(column, 0);
            PhysicalPosition::new(p.x * scale, p.y * scale)
        };
        surface.mouse_pressed(MouseButton::Left, click(10), none());
        surface.cursor_moved(click(15), none());
        surface.mouse_released(MouseButton::Left, click(15), none());
        selections.push(
            surface
                .last_selection()
                .expect("a press-drag-release should select something")
                .to_string(),
        );
    }

    assert_eq!(
        selections[0], selections[1],
        "the same logical click selected {:?} at DPR 1 but {:?} at DPR 2",
        selections[0], selections[1]
    );
    // Not vacuous: the margin really did move the mapping off the naive
    // column, so both scales are reporting a margin-corrected answer rather
    // than both ignoring it.
    assert_ne!(
        selections[0], "KLMNO",
        "the margin should have shifted the span off its unmargined columns"
    );
    assert!(
        !selections[0].is_empty(),
        "the drag selected nothing at either scale"
    );
}
