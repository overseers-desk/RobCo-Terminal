//! Done-test: each shell's fixed geometry, held in `src/shells/*.rs`,
//! checked against hand-recorded expected values. One table per shell, so a
//! drift in either place shows up as a single failing row rather than a
//! silent mismatch.

use chassis::shells::{annunciator, slide_rule, switchboard};

#[test]
fn annunciator_metrics_match_the_recorded_metrics() {
    use annunciator::metrics as m;
    assert_eq!(m::BANK_PADDING, 3);
    assert_eq!(m::TOP_PADDING, 61);
    assert_eq!(m::BOTTOM_PADDING, 8);
    assert_eq!(m::RIGHT_PADDING, 14);
    assert_eq!(m::ROW_SPACING, 2.0);
    assert_eq!(m::COLUMN_GAP, 16);
    assert_eq!(m::NUMERAL_WIDTH, 46);
    assert_eq!(m::STRIP_PADDING, 4);
    assert_eq!(m::MIN_ROW_HEIGHT, 43);
    assert_eq!(m::TRACK_WIDTH, 14);
    assert_eq!(m::CASTING_LIGHT_DIR, [0.8, -0.6]);
    assert_eq!(m::CASTING_COLOR, "#16130f");

    // The invariant this shell's constants encode: the strip fills the
    // punched hole inside min_row_height, pitch lands at 45 regardless of
    // padding sums.
    assert_eq!(m::MIN_ROW_HEIGHT as f64 + m::ROW_SPACING, 45.0);
}

#[test]
fn slide_rule_metrics_match_the_recorded_metrics() {
    use slide_rule::metrics as m;
    assert_eq!(m::BANK_PADDING, 76);
    assert_eq!(m::TOP_PADDING, 64);
    assert_eq!(m::BOTTOM_PADDING, 63);
    assert_eq!(m::RIGHT_PADDING, 8);
    assert_eq!(m::ROW_SPACING, 17.8);
    assert_eq!(m::COLUMN_GAP, 8);
    assert_eq!(m::NUMERAL_WIDTH, 36);
    assert_eq!(m::STRIP_PADDING, 2);
    assert_eq!(m::MIN_ROW_HEIGHT, 43);
    assert!(m::CHASSIS_CARRIES_TRACK);
    assert_eq!(m::TRACK_X, 29);
    assert_eq!(m::TRACK_WIDTH, 41);
    assert_eq!(m::CASTING_LIGHT_DIR, [-0.55, -0.85]);
    assert_eq!(m::CASTING_COLOR, "#453c2d");

    // The mock's row pitch, 60.8px, is MIN_ROW_HEIGHT + ROW_SPACING: the
    // air between windows carries the fraction.
    assert!((m::MIN_ROW_HEIGHT as f64 + m::ROW_SPACING - 60.8).abs() < 1e-9);

    // The rail's mock x-span, 29..69, is TRACK_X..TRACK_X + TRACK_WIDTH.
    assert_eq!(m::TRACK_X + m::TRACK_WIDTH, 70); // 29 + 41
}

#[test]
fn switchboard_metrics_match_the_recorded_metrics() {
    use switchboard::metrics as m;
    assert_eq!(m::BANK_PADDING, 8);
    assert_eq!(m::TOP_PADDING, 34);
    assert_eq!(m::BOTTOM_PADDING, 21);
    assert_eq!(m::RIGHT_PADDING, 29);
    assert_eq!(m::ROW_SPACING, 14.29);
    assert_eq!(m::COLUMN_GAP, 18);
    assert_eq!(m::NUMERAL_WIDTH, 170);
    assert_eq!(m::STRIP_PADDING, 2);
    assert_eq!(m::MIN_ROW_HEIGHT, 48);
    assert_eq!(m::SWITCH_WELL_X, 64);
    assert_eq!(m::SWITCH_WELL_WIDTH, 102);
    assert_eq!(m::SWITCH_WELL_HEIGHT, 52);
    assert_eq!(m::SWITCH_WELL_RADIUS, 10);
    assert_eq!(m::NUMERAL_CENTER_X, 32);
    assert_eq!(m::PAGER_HEIGHT, 104);
    assert_eq!(m::PAGER_ARROW_WIDTH, 74);
    assert_eq!(m::PAGER_ARROW_HEIGHT, 82);
    assert_eq!(m::PAGE_WINDOW_WIDTH, 122);
    assert_eq!(m::PAGE_WINDOW_HEIGHT, 40);
    assert_eq!(m::PAGE_WINDOW_RADIUS, 6);
    assert_eq!(m::CASTING_LIGHT_DIR, [-0.4, -0.9]);
    assert_eq!(m::CASTING_COLOR, "#232830");

    // The derivation: (908 - 36) / 14 == the mock's well-top pitch; less
    // MIN_ROW_HEIGHT (48) leaves ROW_SPACING, to within the artwork's own
    // couple-of-pixel drift.
    let mock_pitch = (908.0 - 36.0) / 14.0;
    assert!((mock_pitch - (m::MIN_ROW_HEIGHT as f64 + m::ROW_SPACING)).abs() < 0.01);

    // The switch well sits centered in the row band: MIN_ROW_HEIGHT -
    // SWITCH_WELL_HEIGHT is the 2*standoff -- standing 2px proud of the row
    // band top and bottom.
    assert_eq!(m::MIN_ROW_HEIGHT - m::SWITCH_WELL_HEIGHT, -4);
}
