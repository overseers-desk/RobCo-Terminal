//! The LED strip's width-quantisation contract.
//!
//! These functions read every one of their settings as explicit arguments
//! (dot pitch, pad cells, side pad cells, besides the cell itself) rather
//! than a settings singleton, since this crate has no live settings object
//! to close over.
//!
//! This is the crate's one home for this arithmetic:
//! [`crate::metrics::LedMetrics`]'s `DisplayMetrics` impl calls straight
//! through to these functions with its own fields, so a kit built at a
//! non-default pitch, or the cell a live profile's lamp font implies, runs
//! this arithmetic and not a second copy of it -- the seam drag that calls
//! `width_for_units` on every pointer move reaches it the same way, through
//! [`crate::seam::SeamDrag`] and [`crate::Cabinet`]'s display kit rather
//! than through here directly. The fixed constants ([`super::LED_DOT_PITCH`]
//! etc.) are the schema's own defaults; [`crate::led_metrics`] is where a
//! live profile turns into the arguments these functions take.

use crate::js;

/// One character's width on the panel, unrounded.
pub fn unit_width(cell_width: u32, dot_pitch: f64) -> f64 {
    cell_width as f64 * dot_pitch
}

pub fn min_units(min_characters: u32) -> u32 {
    min_characters
}

/// A strip's height for a given band of unlit rows above and below the
/// glyphs.
pub fn height_for_pad_cells(cell_height: u32, pad_cells_y: u32, dot_pitch: f64) -> i64 {
    js::round(((cell_height + pad_cells_y) as f64) * dot_pitch) as i64
}

/// Rounded, not truncated -- a floored strip would leave the row half a
/// pixel short of the window it has to hold. `n` counts visible characters;
/// the side pad cells ride on top.
pub fn width_for_units(cell_width: u32, n: u32, side_pad_cells: u32, dot_pitch: f64) -> i64 {
    js::round(cell_width as f64 * (n + 2 * side_pad_cells) as f64 * dot_pitch) as i64
}

/// A fixture names the hole it punched in pixels, the display answers in
/// lamp rows -- never less than the settings' own floor.
pub fn pad_cells_for_hole(
    cell_height: u32,
    hole_height: f64,
    dot_pitch: f64,
    floor_pad_cells: u32,
) -> i64 {
    let from_hole = js::round(hole_height / dot_pitch) as i64 - cell_height as i64;
    (floor_pad_cells as i64).max(from_hole)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::displays::led::{LED_DOT_PITCH, LED_PAD_CELLS, LED_SIDE_PAD_CELLS};

    // Hand-computed, at two sampled cell sizes: UNSCII 8x8-ish (8, 9) and
    // IBM VGA 8x16-ish (8, 16), and the schema's own default pitch/pad
    // constants -- the same ones a live profile always supplies
    // (`crate::led_metrics`).

    #[test]
    fn unit_width_matches_the_defining_formula() {
        assert_eq!(unit_width(8, LED_DOT_PITCH), 12.0); // 8 * 1.5
        assert_eq!(unit_width(9, LED_DOT_PITCH), 13.5); // 9 * 1.5
    }

    #[test]
    fn min_units_matches_the_defining_formula() {
        assert_eq!(min_units(8), 8);
    }

    #[test]
    fn height_for_pad_cells_matches_the_defining_formula() {
        // (8 + 4) * 1.5 = 18
        assert_eq!(height_for_pad_cells(8, 4, LED_DOT_PITCH), 18);
        // (16 + 4) * 1.5 = 30
        assert_eq!(height_for_pad_cells(16, 4, LED_DOT_PITCH), 30);
        // A fractional pitch rounds up, not down: (9 + 1) * 1.5 = 15 exactly,
        // but (9 + 2) * 1.5 = 16.5 rounds to 17, never floors to 16.
        assert_eq!(height_for_pad_cells(9, 2, LED_DOT_PITCH), 17);
    }

    #[test]
    fn width_for_units_matches_the_defining_formula() {
        // 8 * (12 + 2*1) * 1.5 = 8 * 14 * 1.5 = 168
        assert_eq!(
            width_for_units(8, 12, LED_SIDE_PAD_CELLS, LED_DOT_PITCH),
            168
        );
        // 8 * (8 + 2) * 1.5 = 120
        assert_eq!(
            width_for_units(8, 8, LED_SIDE_PAD_CELLS, LED_DOT_PITCH),
            120
        );
    }

    #[test]
    fn pad_cells_for_hole_matches_the_defining_formula() {
        // round(60 / 1.5) - 8 = 40 - 8 = 32, above the floor of 4.
        assert_eq!(
            pad_cells_for_hole(8, 60.0, LED_DOT_PITCH, LED_PAD_CELLS),
            32
        );
        // round(6 / 1.5) - 8 = 4 - 8 = -4, below the floor: floor wins.
        assert_eq!(pad_cells_for_hole(8, 6.0, LED_DOT_PITCH, LED_PAD_CELLS), 4);
    }
}
