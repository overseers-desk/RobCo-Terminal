//! A whole crossing, and a whole day of them, driven by a clock made up on
//! the spot.
//!
//! Nothing here needs a terminal, a window or a GPU, because the crate is
//! arithmetic: the fake clock below is the same trick `crt::Pacing::tick_by`
//! plays, and it is what lets a test watch a duck cross ten thousand times
//! in a millisecond.
//!
//! Two claims are worth more than the rest and are made several ways. A
//! crossing **ends** -- there is no size of screen, no resize, no cadence of
//! calls that leaves a critter standing on somebody's text. And a critter is
//! **off any cell within about a second**, which is the whole of what makes
//! an uninvited animation acceptable over a line somebody is reading.

use std::time::{Duration, Instant};

use critters::{Critters, Crossing, ART};

fn epoch() -> Instant {
    Instant::now()
}

/// One step of a walk: how far along, and what it wanted painted.
type Step = (u32, Vec<(usize, usize, char)>);

/// Walk one crossing by hand, without a scheduler: every step of it, on a
/// screen this size.
fn walk(name: &str, cols: usize, rows: usize) -> Vec<Step> {
    let art = ART.iter().find(|a| a.name == name).unwrap();
    let facing_left = art.right.is_empty();
    let mut out = Vec::new();
    let mut crossing = Crossing {
        art,
        facing_left,
        top: 2,
        step: 0,
    };
    while !crossing.done(cols) {
        let mut cells = Vec::new();
        crossing.paint(cols, rows, &mut cells);
        out.push((crossing.step, cells));
        crossing.step += 1;
    }
    out
}

#[test]
fn a_crossing_goes_on_at_one_edge_and_off_at_the_other() {
    for art in &ART {
        let steps = walk(art.name, 80, 24);
        assert_eq!(
            steps.len() as u32,
            80 + u32::from(art.width),
            "{} takes the wrong number of steps",
            art.name
        );
        // It is off the glass at both ends and on it in the middle.
        assert!(
            steps.first().unwrap().1.is_empty(),
            "{} starts on screen",
            art.name
        );
        assert!(
            steps[steps.len() / 2].1.len() > 3,
            "{} is invisible halfway across",
            art.name
        );
        // The last drawn step is a legitimate sliver at the far edge; the
        // step after it is the retirement, and paints nothing at all.
        let facing_left = art.right.is_empty();
        let mut after = Vec::new();
        Crossing {
            art,
            facing_left,
            top: 2,
            step: 80 + u32::from(art.width),
        }
        .paint(80, 24, &mut after);
        assert!(
            after.is_empty(),
            "{} is still on screen once retired",
            art.name
        );
    }
}

/// The promise the feature rests on, measured rather than asserted of the
/// table: for every piece, follow one column of the screen through a whole
/// crossing and time the longest unbroken run of steps in which that column
/// is painted.
#[test]
fn no_piece_stands_on_one_cell_for_longer_than_a_second() {
    for art in &ART {
        let steps = walk(art.name, 80, 24);
        let mut worst = 0u32;
        for col in 0..80usize {
            let mut run = 0u32;
            for (_, cells) in &steps {
                if cells.iter().any(|(_, c, _)| *c == col) {
                    run += 1;
                    worst = worst.max(run);
                } else {
                    run = 0;
                }
            }
        }
        let ms = worst * u32::from(art.step_ms);
        assert!(
            ms <= 1100,
            "{} covers a cell for {ms} ms ({worst} steps)",
            art.name
        );
    }
}

#[test]
fn the_same_seed_paints_the_same_screen() {
    let (t0, mut a, mut b) = (
        epoch(),
        Critters::new(42, true, Duration::from_secs(60), [true; ART.len()]),
        Critters::new(42, true, Duration::from_secs(60), [true; ART.len()]),
    );
    for i in 0..20_000u64 {
        let now = t0 + Duration::from_millis(i * 25);
        assert_eq!(a.tick(now, 80, 24), b.tick(now, 80, 24));
        assert_eq!(a.cells(), b.cells());
    }
}

/// A day of it. Every crossing that starts finishes, the glass is its own
/// again between them, and the count is near what the mean promises.
#[test]
fn a_day_of_crossings_all_end() {
    let t0 = epoch();
    let mut critters = Critters::new(9, true, Duration::from_secs(900), [true; ART.len()]);
    let (mut seen, mut standing, mut on_screen) = (0u32, None, 0u32);
    // A day at the shipped redraw cadence.
    for i in 0..(24 * 60 * 60 * 1000 / 50) {
        let now = t0 + Duration::from_millis(i * 50);
        critters.tick(now, 80, 24);
        let name = critters.crossing();
        if name != standing {
            if name.is_some() {
                seen += 1;
            }
            standing = name;
        }
        if !critters.cells().is_empty() {
            on_screen += 1;
        }
    }
    // 96 quarter-hours in a day, and the draw is memoryless, so the count is
    // near it rather than equal to it.
    assert!((60..140).contains(&seen), "{seen} crossings in a day");
    assert!(critters.crossing().is_none(), "one was left standing");
    // The glass is quiet the overwhelming majority of the time: a few
    // seconds of critter per quarter hour.
    let ticks = 24 * 60 * 60 * 1000 / 50;
    assert!(
        f64::from(on_screen) / (ticks as f64) > 0.0,
        "nothing ever appeared"
    );
    assert!(
        f64::from(on_screen) / (ticks as f64) < 0.02,
        "the glass was busy {on_screen} ticks of {ticks}"
    );
}

/// The user's own `effects_frame_skip` sets the cadence, so the same
/// crossing has to survive being asked about six times as rarely.
#[test]
fn a_coarse_cadence_still_crosses_and_still_ends() {
    for gap in [16u64, 50, 167] {
        let t0 = epoch();
        let mut critters = Critters::new(5, true, Duration::from_secs(30), [true; ART.len()]);
        let mut ever = false;
        for i in 0..(60 * 60 * 1000 / gap) {
            critters.tick(t0 + Duration::from_millis(i * gap), 80, 24);
            ever |= !critters.cells().is_empty();
        }
        assert!(ever, "nothing crossed at a {gap} ms cadence");
        assert!(
            critters.crossing().is_none(),
            "one stood at a {gap} ms cadence"
        );
    }
}

#[test]
fn a_resize_mid_crossing_stays_in_bounds_and_still_ends() {
    let t0 = epoch();
    let mut critters = Critters::new(11, true, Duration::from_secs(5), [true; ART.len()]);
    let mut sizes = [(80usize, 24usize), (40, 8), (200, 60), (3, 1), (80, 24)]
        .into_iter()
        .cycle();
    let mut size = sizes.next().unwrap();
    for i in 0..200_000u64 {
        if i % 97 == 0 {
            size = sizes.next().unwrap();
        }
        critters.tick(t0 + Duration::from_millis(i * 13), size.0, size.1);
        for &(row, col, _) in critters.cells() {
            assert!(
                row < size.1 && col < size.0,
                "{row},{col} is off a {size:?} screen"
            );
        }
    }
}

#[test]
fn a_screen_with_no_cells_hosts_nothing_and_loses_nothing() {
    let t0 = epoch();
    let mut critters = Critters::new(3, true, Duration::from_secs(1), [true; ART.len()]);
    for i in 0..5_000u64 {
        critters.tick(t0 + Duration::from_millis(i * 20), 0, 0);
        assert!(critters.cells().is_empty());
    }
    // The crossing that was due while the window had no cells is still due.
    let mut ever = false;
    for i in 5_000..8_000u64 {
        critters.tick(t0 + Duration::from_millis(i * 20), 80, 24);
        ever |= !critters.cells().is_empty();
    }
    assert!(ever, "the glass never got its critter back");
}

#[test]
fn taller_than_the_glass_shows_a_band_of_itself() {
    let steps = walk("locomotive", 80, 4);
    let mid = &steps[steps.len() / 2].1;
    assert!(
        !mid.is_empty(),
        "a ten-row locomotive vanished on a four-row screen"
    );
    assert!(mid.iter().all(|(r, _, _)| *r < 4));
}

#[test]
fn wider_than_the_glass_still_crosses() {
    let steps = walk("locomotive", 20, 24);
    assert_eq!(steps.len(), 20 + 54);
    assert!(steps[steps.len() / 2].1.iter().all(|(_, c, _)| *c < 20));
}

#[test]
fn switched_off_it_is_a_build_without_it() {
    let t0 = epoch();
    let mut critters = Critters::new(7, false, Duration::from_secs(1), [true; ART.len()]);
    for i in 0..100_000u64 {
        critters.tick(t0 + Duration::from_millis(i * 10), 80, 24);
        assert!(critters.cells().is_empty());
        assert!(critters.crossing().is_none());
        assert!(critters.wake_at().is_none());
    }
}

#[test]
fn switched_off_mid_crossing_it_leaves_at_once() {
    let t0 = epoch();
    let mut critters = Critters::new(13, true, Duration::from_secs(1), [true; ART.len()]);
    let mut i = 0u64;
    while critters.cells().is_empty() {
        critters.tick(t0 + Duration::from_millis(i * 10), 80, 24);
        i += 1;
        assert!(i < 100_000, "nothing ever crossed");
    }
    critters.configure(false, Duration::from_secs(1), [true; ART.len()]);
    critters.tick(t0 + Duration::from_millis(i * 10), 80, 24);
    assert!(critters.cells().is_empty());
}

/// The caller sleeps on this, so it may never be later than the next thing
/// that changes the picture.
#[test]
fn it_never_asks_to_sleep_through_its_own_next_step() {
    let t0 = epoch();
    let mut critters = Critters::new(17, true, Duration::from_secs(2), [true; ART.len()]);
    for i in 0..50_000u64 {
        let now = t0 + Duration::from_millis(i * 7);
        critters.tick(now, 80, 24);
        let wake = critters
            .wake_at()
            .expect("enabled, so it always has a next");
        if let Some(name) = critters.crossing() {
            let art = ART.iter().find(|a| a.name == name).unwrap();
            assert!(
                wake <= now + Duration::from_millis(u64::from(art.step_ms)),
                "{name} would sleep past its next column"
            );
        }
    }
}

#[test]
fn a_retired_piece_never_comes_and_the_others_take_its_turns() {
    let t0 = epoch();
    let only = ART.iter().position(|a| a.name == "swan").unwrap();
    let mut allowed = [false; ART.len()];
    allowed[only] = true;
    let mut critters = Critters::new(23, true, Duration::from_secs(2), allowed);
    let mut seen = 0;
    for i in 0..200_000u64 {
        critters.tick(t0 + Duration::from_millis(i * 10), 80, 24);
        if let Some(name) = critters.crossing() {
            assert_eq!(name, "swan", "a retired piece crossed");
            seen += 1;
        }
    }
    assert!(seen > 0, "the one piece left on never came");
}

#[test]
fn every_piece_retired_is_the_same_silence_as_switched_off() {
    let t0 = epoch();
    let mut critters = Critters::new(29, true, Duration::from_secs(1), [false; ART.len()]);
    for i in 0..200_000u64 {
        critters.tick(t0 + Duration::from_millis(i * 10), 80, 24);
        assert!(critters.cells().is_empty());
        assert!(critters.crossing().is_none());
    }
}
