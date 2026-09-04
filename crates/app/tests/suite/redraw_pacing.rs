//! The output governor: child output asks for at most one frame per
//! `EFFECTS_BASE_FRAME` (60 Hz), however fast the PTY poll runs.
//!
//! The PTY is polled at ~125 Hz, and before the governor a flood painted at
//! that rate, though a 60 Hz panel never shows most of those frames
//! (measured at present-interval p50 = 3.3 ms during a `seq 1 200000` flood
//! on real hardware). A headless
//! surface has no glass, so the effects clock stays out of the way and the
//! ticks below isolate the governor.

use std::time::{Duration, Instant};

use app::shell::Surface;
use app::window::{TerminalSurface, EFFECTS_BASE_FRAME};
use term::{CellSize, SessionConfig, Viewport};

fn surface() -> TerminalSurface {
    let config = SessionConfig {
        program: Some("/bin/cat".to_string()),
        args: vec![],
        working_directory: None,
        env: vec![("TERM".to_string(), "xterm-256color".to_string())],
        scrollback: 100,
        grapheme_clustering: false,
        rate: None,
    };
    let viewport = Viewport::new(720, 490, 1.0, CellSize::new(9.0, 18.0));
    TerminalSurface::headless(&config, viewport)
}

#[test]
fn a_flood_of_output_buys_one_frame_per_cadence_not_one_per_tick() {
    let mut surface = surface();
    surface.write(b"first\r");

    // Tick until the echo lands and buys its frame. The stamp is taken
    // before the tick that redraws, so it is no later than the clock the
    // governor dated the frame by.
    let deadline = Instant::now() + Duration::from_secs(10);
    let bought = loop {
        assert!(Instant::now() < deadline, "cat never echoed");
        let stamp = Instant::now();
        if surface.tick().redraw {
            break stamp;
        }
        std::thread::sleep(Duration::from_millis(2));
    };

    // More output, immediately. The next frame is not due until a cadence
    // after the one just bought, so however many ticks run before then,
    // the one that redraws comes no sooner than that. Measured between
    // stamps rather than counted inside a window, because how many ticks
    // fit in a window, and whether a window ends before the cadence does,
    // is the scheduler's to decide. The pending output is not dropped
    // either: the frame it waited for is the one this loop ends on.
    surface.write(b"second\r");
    let deadline = Instant::now() + Duration::from_secs(10);
    let after = loop {
        assert!(
            Instant::now() < deadline,
            "the pending output never got its frame"
        );
        let redraw = surface.tick().redraw;
        let stamp = Instant::now();
        if redraw {
            break stamp;
        }
    };
    let waited = after.duration_since(bought);
    assert!(
        waited >= EFFECTS_BASE_FRAME,
        "output redrew {waited:?} after the frame it already bought, inside \
         the {EFFECTS_BASE_FRAME:?} cadence"
    );
}
