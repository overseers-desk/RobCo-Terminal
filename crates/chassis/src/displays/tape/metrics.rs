//! The tape well's width-quantisation contract.
//!
//! The crate's one home for this arithmetic. [`crate::metrics::TapeMetrics`]
//! behind [`crate::metrics::DisplayMetrics`] is the shape on the live path (a
//! kit is swapped at runtime, and a free function cannot be); its impl calls
//! straight through to the functions here with its own fields, rather than
//! reimplementing the formulas.

use super::LETTER_PIXEL_SIZE;
use crate::js;

/// One character's width, unrounded -- the advance of `"M"` in Departure
/// Mono at the kit's fixed letter size
/// (`term::fonts::metrics::char_advance_px`, the proven 26.6 path) called
/// directly on the catalogue's own font bytes.
pub fn unit_width(font_data: &[u8]) -> f64 {
    term::fonts::metrics::char_advance_px(font_data, 'M', LETTER_PIXEL_SIZE).unwrap_or(0.0)
}

/// The same character-count floor the LED strip reads.
pub fn min_units() -> u32 {
    super::super::led::MIN_LED_CHARACTERS
}

pub fn width_for_units(unit_width: f64, n: u32, end_pad: u32) -> i64 {
    js::round(n as f64 * unit_width + 2.0 * end_pad as f64) as i64
}

/// Nought is no height at all here (unlike the LED panel, where it only
/// means no padding), so a fixture naming none gets the pass-through's
/// floor of the raw hole height, never a hidden minimum.
pub fn pad_cells_for_hole(hole_height: f64) -> i64 {
    js::round(hole_height.max(0.0)) as i64
}

/// A straight pass-through -- tape has nothing to count, so the hole's own
/// height travels through as the strip's height.
pub fn height_for_pad_cells(pad_cells: i64) -> i64 {
    pad_cells
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_width_is_the_departure_mono_m_advance_at_20px() {
        let entry = term::fonts::font_by_name(super::super::FONT_NAME).unwrap();
        let got = unit_width(entry.data());
        // A monospaced face's advance at 20px sits comfortably inside a
        // pixel-ish band; this pins it against silent regression without
        // hardcoding FreeType-exact fixed-point arithmetic a second time
        // (metrics.rs's own tests already do that).
        assert!(got > 5.0 && got < 20.0, "unit_width={got}");
    }

    #[test]
    fn min_units_matches_the_defining_formula() {
        assert_eq!(min_units(), 8);
    }

    #[test]
    fn width_for_units_matches_the_defining_formula() {
        // Sampled at the kit's own default: 12 characters, end_pad 12
        // (`super::super::END_PAD`).
        // round(12 * 10.0 + 2*12) = round(144) = 144
        assert_eq!(width_for_units(10.0, 12, super::super::END_PAD), 144);
        // A fractional unit_width still rounds, not truncates.
        // round(8 * 9.6 + 24) = round(100.8) = 101
        assert_eq!(width_for_units(9.6, 8, super::super::END_PAD), 101);
    }

    #[test]
    fn pad_cells_for_hole_matches_the_defining_formula() {
        assert_eq!(pad_cells_for_hole(44.0), 44);
        // Negative input floors at 0, not at some other minimum (tape has
        // no floor of its own, unlike the LED panel's ledPadCells).
        assert_eq!(pad_cells_for_hole(-5.0), 0);
        assert_eq!(pad_cells_for_hole(30.4), 30);
    }

    #[test]
    fn height_for_pad_cells_matches_the_defining_formula() {
        assert_eq!(height_for_pad_cells(44), 44);
        assert_eq!(height_for_pad_cells(0), 0);
    }
}
