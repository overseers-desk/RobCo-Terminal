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
use app::window::TerminalSurface;
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

    // Tick until the echo lands and buys its frame.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(Instant::now() < deadline, "cat never echoed");
        if surface.tick().redraw {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }

    // More output, immediately: the next frame is not due yet, so however
    // many ticks run inside the cadence, none of them redraws. The window
    // is kept well under the 16.7 ms cadence so the assertion is exact.
    surface.write(b"second\r");
    let inside_cadence = Instant::now() + Duration::from_millis(8);
    let mut redraws = 0;
    while Instant::now() < inside_cadence {
        if surface.tick().redraw {
            redraws += 1;
        }
    }
    assert_eq!(redraws, 0, "output redrew inside the frame it already bought");

    // The pending output is not dropped: once the cadence elapses, the
    // frame it waited for is asked for.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(
            Instant::now() < deadline,
            "the pending output never got its frame"
        );
        if surface.tick().redraw {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}
