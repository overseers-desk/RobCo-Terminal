//! Done-test: each shell's region layout, pure geometry, sampled at a few
//! window sizes. No GPU needed -- these are the rect/field-mapping formulas
//! each shell's chassis and frame compute from margins and offsets, checked
//! against hand-derived expected values at each sampled size.

use chassis::shells::common::{field_mapping, frame_viewport_size, Rect};
use chassis::shells::{annunciator, slide_rule, switchboard};

/// Chassis sizes sampled: a small bank, the mock's own approximate size, and
/// a wide one (the seam-drag's stretch direction is handled elsewhere, but
/// the region formulas must hold regardless of how wide the bank gets dragged).
const CHASSIS_SIZES: &[(f64, f64)] = &[(357.0, 1080.0), (500.0, 900.0), (357.0, 400.0)];

#[test]
fn annunciator_regions_at_sampled_sizes() {
    for &(w, h) in CHASSIS_SIZES {
        // chassis_rect: no margins of its own -- covers the whole item.
        let chassis = annunciator::chassis_rect((w, h));
        assert_eq!(chassis, Rect::new(0.0, 0.0, w, h));

        // plate_rect: fills the parent with left margin 8, top margin 2,
        // right margin 0, bottom margin 8.
        let plate = annunciator::plate_rect((w, h));
        assert_eq!(plate, Rect::new(8.0, 2.0, w - 8.0, h - 10.0));
    }

    // A degenerate chassis (smaller than its own margins) floors at zero
    // width/height rather than going negative -- `Rect::new` widths would be
    // meaningless past that point on the real anchors too, but the port must
    // not silently produce a negative rect.
    let tiny = annunciator::plate_rect((4.0, 4.0));
    assert_eq!(tiny, Rect::new(8.0, 2.0, 0.0, 0.0));
}

#[test]
fn slide_rule_regions_at_sampled_sizes() {
    for &(w, h) in CHASSIS_SIZES {
        let chassis = slide_rule::chassis_rect((w, h));
        assert_eq!(chassis, Rect::new(0.0, 0.0, w, h));

        // rail_rect: x 29, width `metrics::TRACK_WIDTH` (41), top/bottom
        // margin 29 each.
        let rail = slide_rule::rail_rect((w, h));
        assert_eq!(rail, Rect::new(29.0, 29.0, 41.0, h - 58.0));
    }

    // Same floor-at-zero behavior as the annunciator's plate, for a chassis
    // shorter than the rail's own 58px of top+bottom margin.
    let tiny = slide_rule::rail_rect((100.0, 40.0));
    assert_eq!(tiny, Rect::new(29.0, 29.0, 41.0, 0.0));
}

#[test]
fn switchboard_has_no_plate_or_rail_region() {
    // The whole chassis paints as one `chassis_metal` region, no nested
    // plate/rail region -- the region layout done-test for this shell is
    // exactly this: there is only ever the one region, at every sampled
    // size.
    for &(w, h) in CHASSIS_SIZES {
        let chassis = switchboard::chassis_rect((w, h));
        assert_eq!(chassis, Rect::new(0.0, 0.0, w, h));
    }
}

/// Frame-region field mapping at sampled (chassis, frame) size pairs,
/// modelling the bank sitting to the right of a frame that spans the whole
/// window. Formula is identical across all three shells
/// (`shells::common::field_mapping`); this test samples a few realistic
/// window widths rather than re-deriving the math per shell.
#[test]
fn field_mapping_at_sampled_window_sizes() {
    for &(window_w, window_h, bank_w) in &[
        (1200.0, 1080.0, 357.0),
        (1600.0, 900.0, 420.0),
        (900.0, 700.0, 300.0),
    ] {
        let frame_region = Rect::new(0.0, 0.0, window_w, window_h);
        let bank_x = window_w - bank_w;
        let chassis = Rect::new(bank_x, 0.0, bank_w, window_h);

        let fm = field_mapping(chassis, Some(frame_region));
        assert_eq!(fm.viewport, [window_w as f32, window_h as f32]);
        assert!((fm.scale[0] - (bank_w / window_w) as f32).abs() < 1e-5);
        assert!((fm.scale[1] - 1.0).abs() < 1e-5);
        assert!((fm.offset[0] - (bank_x / window_w) as f32).abs() < 1e-5);
        assert_eq!(fm.offset[1], 0.0);
    }
}

/// The frame's viewport-size formula (`width / window_scaling, height /
/// window_scaling`), re-checked through each shell's own re-export, at
/// sampled window sizes including a HiDPI `window_scaling`.
#[test]
fn frame_viewport_at_sampled_sizes_matches_shared_formula() {
    for &(w, h, scaling) in &[(1200.0, 1080.0, 1.0), (2400.0, 2160.0, 2.0)] {
        let expected = frame_viewport_size(w, h, scaling);
        assert_eq!(annunciator::frame_viewport(w, h, scaling), expected);
        assert_eq!(slide_rule::frame_viewport(w, h, scaling), expected);
        assert_eq!(switchboard::frame_viewport(w, h, scaling), expected);
    }
}
