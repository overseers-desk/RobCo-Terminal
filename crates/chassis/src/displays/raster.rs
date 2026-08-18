//! The one conversion both display kits need: `term::fonts::led::LedRaster`
//! (a coverage byte per pixel) to the RGBA8 bytes a wgpu texture upload
//! wants.
//!
//! The convention is premultiplied white, `[a, a, a, a]` per pixel -- both
//! shaders read `Source` this way: `led_matrix.slang` takes
//! `max(glyph.r, glyph.g, glyph.b)`, `tape_label.slang` takes only
//! `texture(Source, guv).a` -- so one conversion serves both.

use term::fonts::led::LedRaster;

/// `raster.alpha` widened to RGBA8, `[a, a, a, a]` per pixel.
pub fn to_rgba8(raster: &LedRaster) -> Vec<u8> {
    let mut out = Vec::with_capacity(raster.alpha.len() * 4);
    for &a in &raster.alpha {
        out.extend_from_slice(&[a, a, a, a]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widens_one_alpha_byte_to_four() {
        let raster = LedRaster {
            width: 2,
            height: 1,
            alpha: vec![255, 0],
        };
        assert_eq!(to_rgba8(&raster), vec![255, 255, 255, 255, 0, 0, 0, 0]);
    }
}
