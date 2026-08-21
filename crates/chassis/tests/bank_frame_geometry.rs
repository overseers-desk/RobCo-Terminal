//! Done-test: the bank frame's geometry at sampled window sizes, against
//! values derived by hand from the layout's own formulas rather than by
//! calling the same code a second time.
//!
//! Every expectation below is written as the arithmetic the formulas state,
//! so a formula that drifts fails here even if the port drifts consistently
//! with itself.
//!
//! The formulas, once, so the tables can stay terse:
//!
//! - `implicit_width = content_x + numeral_width + column_gap + strip_width
//!   + right_padding`
//! - `content_x = bank_padding + (lane_in_chassis ? 0 : track_width) +
//!   track_gap`
//! - `track_width` and `track_gap` are the shell's only under the pointer law
//! - `width_for_units(n) = round(cell_width * (n + 2 * side_pad_cells) *
//!   dot_pitch)`
//! - the well takes what the bank leaves
//! - `normalized_screen_scale = 1024 / max(1, 0.5 * crt_region.width + 0.5 *
//!   crt_region.height)`
//! - `rows_visible`'s settle arithmetic

use chassis::bank::ChannelIndicator;
use chassis::metrics::{shells, LedMetrics, TapeMetrics};
use chassis::{BankGeometry, WindowLayout};

/// At the settings' own defaults (pitch 1.5, one side-pad cell each end)
/// over a 5px lamp cell: `round(5 * (n + 2) * 1.5)`.
fn led_strip_width(n: i32) -> i32 {
    (5.0 * (n + 2) as f64 * 1.5).round() as i32
}

/// The default window size and five others spanning what a real desktop
/// hands the window.
const WINDOW_SIZES: [(f64, f64); 6] = [
    (640.0, 480.0),
    (800.0, 600.0),
    (1024.0, 768.0),
    (1440.0, 900.0),
    (1920.0, 1080.0),
    (3840.0, 2160.0),
];

fn stock_bank() -> BankGeometry {
    BankGeometry::new(
        &shells::annunciator(),
        &LedMetrics::default(),
        12,
        ChannelIndicator::Glow,
    )
}

#[test]
fn the_bank_never_moves_with_the_window() {
    // Its width follows the LED strip settings alone and never the window's
    // size, so dragging the window edge cannot move the screen well's left
    // edge. This is the load-bearing claim of the whole layout, so it gets a
    // test of its own rather than an assertion inside one.
    let g = stock_bank();
    let expected = 3 + 46 + 16 + led_strip_width(12) + 14; // annunciator
    assert_eq!(expected, 184);
    for (w, h) in WINDOW_SIZES {
        let layout = WindowLayout::new(w, h, g.implicit_width as f64);
        assert_eq!(
            layout.bank.width, expected as f64,
            "bank moved in a {w}x{h} window"
        );
        assert_eq!(layout.bank.height, h);
    }
}

#[test]
fn the_well_and_its_scale_at_sampled_window_sizes() {
    let g = stock_bank();
    let bank = 184.0;

    // The well's geometry and scale formula, computed here rather than
    // called.
    let expected: [(f64, f64, f64); 6] = [
        (640.0, 480.0, 1024.0 / (0.5 * 456.0 + 0.5 * 480.0)),
        (800.0, 600.0, 1024.0 / (0.5 * 616.0 + 0.5 * 600.0)),
        (1024.0, 768.0, 1024.0 / (0.5 * 840.0 + 0.5 * 768.0)),
        (1440.0, 900.0, 1024.0 / (0.5 * 1256.0 + 0.5 * 900.0)),
        (1920.0, 1080.0, 1024.0 / (0.5 * 1736.0 + 0.5 * 1080.0)),
        (3840.0, 2160.0, 1024.0 / (0.5 * 3656.0 + 0.5 * 2160.0)),
    ];

    for (w, h, scale) in expected {
        let layout = WindowLayout::new(w, h, g.implicit_width as f64);
        assert_eq!(layout.crt.x, bank, "well origin in a {w}x{h} window");
        assert_eq!(layout.crt.width, w - bank, "well width in a {w}x{h} window");
        assert_eq!(layout.crt.height, h);
        assert!(
            (layout.normalized_screen_scale() - scale).abs() < 1e-12,
            "scale in a {w}x{h} window: {} vs {scale}",
            layout.normalized_screen_scale()
        );
    }
}

#[test]
fn rows_per_page_at_sampled_window_heights() {
    // The settle arithmetic reduces: rows_height adds the spacing back that
    // it just subtracted, so the count is
    // floor((height - top_padding - bottom_padding - pager_height) / pitch).
    // Deriving it that way here is a cross-check on the implementation,
    // which keeps this same two-step shape.
    let g = stock_bank();
    let pitch = 43.0 + 2.0; // min_row_height + row_spacing, annunciator
    for (_, h) in WINDOW_SIZES {
        for pager in [0, 40, 104] {
            let expected = ((h - 61.0 - 8.0 - pager as f64) / pitch).floor().max(1.0) as i32;
            assert_eq!(
                g.rows_visible(h as i32, pager),
                Some(expected),
                "rows at height {h} with a {pager}px pager"
            );
        }
    }
}

#[test]
fn every_shell_and_display_at_every_character_count_that_matters() {
    // Per shell, under the glow law the stock profiles all ask for.
    // Character counts: the LED floor, the default, and two wider ones.
    let annunciator = shells::annunciator();
    let slide_rule = shells::slide_rule();
    let switchboard = shells::switchboard();
    let led = LedMetrics::default();

    for n in [8, 12, 20, 40] {
        let strip = led_strip_width(n);

        // 3 + 46 + 16 + strip + 14
        let g = BankGeometry::new(&annunciator, &led, n, ChannelIndicator::Glow);
        assert_eq!(
            g.implicit_width,
            79 + strip,
            "annunciator at {n} characters"
        );

        // 76 + 36 + 8 + strip + 8; the rail is chassis furniture and costs
        // the rows nothing.
        let g = BankGeometry::new(&slide_rule, &led, n, ChannelIndicator::Glow);
        assert_eq!(
            g.implicit_width,
            128 + strip,
            "slide rule at {n} characters"
        );

        // 8 + 170 + 18 + strip + 29; the numeral column is the folded lane
        // that carries the switch well too.
        let g = BankGeometry::new(&switchboard, &led, n, ChannelIndicator::Switch);
        assert_eq!(
            g.implicit_width,
            225 + strip,
            "switchboard at {n} characters"
        );
    }

    // The tape kit, whose one character is a measured advance rather than a
    // lamp cell. Measured against the mock: twelve characters of Departure
    // Mono at 20px inside 12px of blank tape at either end fill the
    // switchboard's 177px label glass exactly.
    let advance = (177.0 - 2.0 * 12.0) / 12.0;
    let tape = TapeMetrics {
        unit_width: advance,
        end_pad: 12,
        min_characters: 8,
    };
    for n in [8, 12, 20, 40] {
        // round(n * advance + 2 * end_pad).
        let strip = (n as f64 * advance + 24.0).round() as i32;
        let g = BankGeometry::new(&switchboard, &tape, n, ChannelIndicator::Switch);
        assert_eq!(g.implicit_width, 225 + strip, "switchboard on tape at {n}");
    }
}

#[test]
fn the_pointer_law_at_every_shell() {
    // Three shells, three different answers, and the third is the one worth
    // having a test for.
    let led = LedMetrics::default();
    let strip = led_strip_width(12);

    // The annunciator declares a 14px rail and no chassis lane, so the bank
    // cuts one out of the rows and puts a column gap beside it: 3 + 14 + 16.
    let g = BankGeometry::new(&shells::annunciator(), &led, 12, ChannelIndicator::Pointer);
    assert_eq!(g.content_x, 33);
    assert_eq!(g.implicit_width, 33 + 46 + 16 + strip + 14);

    // The slide rule's rail is chassis furniture standing inside its own
    // shoulder, so the pointer law costs it nothing at all.
    let g = BankGeometry::new(&shells::slide_rule(), &led, 12, ChannelIndicator::Pointer);
    assert_eq!(g.content_x, 76);
    assert_eq!(g.implicit_width, 128 + strip);

    // The switchboard declares no rail: the thrown switch is the whole mark.
    // A profile that asked for the pointer law anyway would still get a gap
    // beside the rail that is not there: `track_gap` is conditioned on the
    // law and on the chassis lane, not on the rail having a width. Recorded
    // rather than corrected: no bundled profile pairs this shell with this
    // law, and quietly changing the behaviour here would be the worse trade.
    let g = BankGeometry::new(&shells::switchboard(), &led, 12, ChannelIndicator::Pointer);
    assert_eq!(g.track_width, 0);
    assert_eq!(g.track_gap, 18);
    assert_eq!(g.content_x, 8 + 18);
    assert_eq!(g.implicit_width, 243 + strip);
}

#[test]
fn the_minimum_window_holds_the_bank_and_the_least_screen() {
    // The hint every shell must carry, for each bundled shell at the
    // default character count, measured against the bare tube's floor: a
    // host that has resolved a font hands in its own, and the composition is
    // the same addition either way.
    let led = LedMetrics::default();
    for (shell, indicator, expected) in [
        (shells::annunciator(), ChannelIndicator::Glow, 184),
        (shells::slide_rule(), ChannelIndicator::Glow, 233),
        (shells::switchboard(), ChannelIndicator::Switch, 330),
    ] {
        let g = BankGeometry::new(&shell, &led, 12, indicator);
        assert_eq!(g.implicit_width, expected);
        assert_eq!(
            chassis::layout::min_inner_size(g.implicit_width, (320, 240)),
            (expected + 320, 240)
        );
    }
    // And with no chassis at all, the well's floor is the whole hint.
    assert_eq!(chassis::layout::min_inner_size(0, (320, 240)), (320, 240));
}
