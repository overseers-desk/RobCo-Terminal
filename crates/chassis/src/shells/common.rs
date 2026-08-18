//! Geometry every shell's chassis and frame repeat verbatim.
//!
//! Held once here rather than three times, byte-for-byte identical across
//! the three shells' chassis and frame field-mapping formulas.

use crate::color::{hex_literal_to_color, with_alpha, Rgba};
use crate::layout::Rect as Rect2;
use crate::paint::{ArcOp, Fill, Painting, RectOp, Stop};

/// A rectangle in the coordinate space of the item it is measured against,
/// pixels, top-left origin: the convention every chassis/frame anchor
/// expression below is read against.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// The three uniforms a chassis derives so its `chassis_metal` region
/// continues the frame's own procedural field instead of keeping a UV space
/// of its own: viewport size, field scale, field offset.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldMapping {
    pub viewport: [f32; 2],
    pub scale: [f32; 2],
    pub offset: [f32; 2],
}

/// The chassis's viewport size / field scale / field offset formula:
///
/// ```text
/// viewport_size = (field_width, field_height)
/// field_scale = (width / field_width, height / field_height)
/// field_offset = frame_region
///     ? ((x - frame_region.x) / field_width, (y - frame_region.y) / field_height)
///     : (0, 0)
/// field_width = frame_region ? max(1, frame_region.width) : 1
/// field_height = frame_region ? max(1, frame_region.height) : 1
/// ```
///
/// `frame_region: None` is the pre-load beat before the frame region has
/// resolved (the same beat every other measure in this crate treats the
/// same way): field width/height fall back to `1`, offset to `(0, 0)`.
pub fn field_mapping(chassis: Rect, frame_region: Option<Rect>) -> FieldMapping {
    let field_width = frame_region.map_or(1.0, |r| r.width.max(1.0));
    let field_height = frame_region.map_or(1.0, |r| r.height.max(1.0));
    let offset = frame_region.map_or([0.0, 0.0], |r| {
        [
            ((chassis.x - r.x) / field_width) as f32,
            ((chassis.y - r.y) / field_height) as f32,
        ]
    });
    FieldMapping {
        viewport: [field_width as f32, field_height as f32],
        scale: [
            (chassis.width / field_width) as f32,
            (chassis.height / field_height) as f32,
        ],
        offset,
    }
}

/// The frame's viewport size (identical in all three shells): `(width /
/// window_scaling, height / window_scaling)`. `width`/`height` are the
/// frame item's own size, in window (device) pixels; `window_scaling` is
/// the profile's window-scaling setting, this shell's metrics contract
/// knows nothing about it and it is supplied by the caller.
pub fn frame_viewport_size(width: f64, height: f64, window_scaling: f64) -> [f32; 2] {
    [
        (width / window_scaling) as f32,
        (height / window_scaling) as f32,
    ]
}

/// Every shell colour this crate reads through `rgb` is a hex colour
/// literal (a shell's casting colour, and its chassis's/frame's base,
/// highlight, shadow, bezel and ridge colours), never a runtime-parsed
/// string, so this goes through [`crate::color::hex_literal_to_color`]
/// (divisor 255), not [`crate::color::str_to_color`] (divisor 256). See
/// `crate::color`'s module doc for the rule both functions follow. Narrowed
/// to the RGB triple the metal oracles take: every shell color below is
/// opaque, so the alpha channel carries no information here.
pub fn rgb(hex: &str) -> [f32; 3] {
    let c = hex_literal_to_color(hex);
    [c.r, c.g, c.b]
}

/// The cheap hash off a numeral that sways one window's lip brightness and
/// rim tone a few percent, so no two windows on the plate came off the mill
/// identical.
///
/// ```text
/// h = 0
/// for each character c in the numeral:
///     h = (h * 31 + code_of(c)) % 997
/// return h / 997
/// ```
///
/// Kept to the byte, modulus included: the whole point of the value is that
/// row 07 and row 08 differ, so an approximation of it would be a different
/// panel rather than a rounding error.
pub fn jitter(numeral: &str) -> f64 {
    let mut h: i64 = 0;
    for c in numeral.chars() {
        h = (h * 31 + c as i64) % 997;
    }
    h as f64 / 997.0
}

/// The slotted screw head every chassis kit bolts its plate down with.
///
/// A canvas painting, not a shader, which is why it is painted here rather
/// than passed to one of [`crate::oracle`]'s metals: a countersink ring, a
/// domed head lit from the caller's own light direction, a lit rim arc with a
/// shadow arc opposite it, and a slot cut across the dome at the caller's
/// angle -- no two screws left a factory aligned.
///
/// `size` is the item's width and height, which every caller leaves at 28;
/// `slot_angle` is in degrees. The painting is in the screw's own
/// coordinates, so a caller places it by the piece's rectangle.
pub fn screw_head(size: f64, slot_angle: f64) -> Painting {
    screw_head_with(size, slot_angle, ScrewColors::default(), (-0.6, -0.8))
}

/// The four metals a screw head is turned from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrewColors {
    pub metal_light: Rgba,
    pub metal_mid: Rgba,
    pub metal_dark: Rgba,
    pub glint: Rgba,
}

impl Default for ScrewColors {
    /// The kit's own defaults, which the annunciator's four screws take
    /// whole; the switchboard's sixteen and the slide rule's three each
    /// override all four.
    fn default() -> Self {
        Self {
            metal_light: hex_literal_to_color("#c1a585"),
            metal_mid: hex_literal_to_color("#5c4f40"),
            metal_dark: hex_literal_to_color("#171008"),
            glint: hex_literal_to_color("#f5efe2"),
        }
    }
}

/// [`screw_head`] with the colours and the light direction the caller
/// overrides, which is what every switchboard screw does: that shell turns its
/// screws from cold steel and lights them with its own casting's key.
pub fn screw_head_with(
    size: f64,
    slot_angle: f64,
    colors: ScrewColors,
    light: (f64, f64),
) -> Painting {
    let ScrewColors {
        metal_light,
        metal_mid,
        metal_dark,
        glint,
    } = colors;
    let (light_x, light_y) = light;

    let r = size / 2.0;
    let (cx, cy) = (r, r);
    let ll = (light_x * light_x + light_y * light_y).sqrt();
    let ll = if ll == 0.0 { 1.0 } else { ll };
    let (lx, ly) = (light_x / ll, light_y / ll);

    let mut p = Painting::new();

    // The countersink, a dark ring the boss sinks into. A concentric radial
    // gradient over the whole disc.
    let clear = Rgba::new(0.0, 0.0, 0.0, 0.0);
    p.rect(RectOp {
        fill: Fill::Radial {
            from: (cx, cy, r * 0.55),
            to: (cx, cy, r),
            stops: vec![
                Stop::new(0.0, clear),
                Stop::new(0.75, Rgba::new(0.0, 0.0, 0.0, 0.55)),
                Stop::new(1.0, Rgba::new(0.0, 0.0, 0.0, 0.1)),
            ],
        },
        ..RectOp::solid(Rect2::new(0.0, 0.0, size, size), r, clear)
    });

    // The head, domed, its focus pulled toward the light.
    let hr = r * 0.78;
    let gx = cx + lx * hr * 0.45;
    let gy = cy + ly * hr * 0.45;
    p.rect(RectOp {
        fill: Fill::Radial {
            from: (gx, gy, hr * 0.08),
            to: (cx, cy, hr),
            stops: vec![
                Stop::new(0.0, glint),
                Stop::new(0.22, metal_light),
                Stop::new(0.62, metal_mid),
                Stop::new(1.0, metal_dark),
            ],
        },
        ..RectOp::solid(
            Rect2::new(cx - hr, cy - hr, 2.0 * hr, 2.0 * hr),
            hr,
            metal_dark,
        )
    });

    // A lit arc toward the light, a shadow arc away from it.
    let la = ly.atan2(lx);
    let line_width = (r * 0.09).max(1.0);
    p.arc(ArcOp {
        center: (cx, cy),
        radius: hr - line_width / 2.0,
        line_width,
        start: la - 1.2,
        end: la + 1.2,
        color: with_alpha(glint, 0.7),
    });
    p.arc(ArcOp {
        center: (cx, cy),
        radius: hr - line_width / 2.0,
        line_width,
        start: la + std::f64::consts::PI - 1.3,
        end: la + std::f64::consts::PI + 1.3,
        color: Rgba::new(0.0, 0.0, 0.0, 0.55),
    });

    // The slot, cut through the dome at the caller's angle, its far wall
    // catching light and its near wall dark.
    let angle = slot_angle * std::f64::consts::PI / 180.0;
    let sw = hr * 1.5;
    let sh = (r * 0.2).max(2.0);
    let pivot = (cx, cy);
    p.rect(
        RectOp::solid(
            Rect2::new(cx - sw / 2.0, cy - sh / 2.0, sw, sh),
            0.0,
            metal_dark,
        )
        .rotated(angle, pivot),
    );
    p.rect(
        RectOp::solid(
            Rect2::new(cx - sw / 2.0, cy + sh / 2.0 - 1.0, sw, 1.0),
            0.0,
            with_alpha(metal_light, 0.8),
        )
        .rotated(angle, pivot),
    );
    p.rect(
        RectOp::solid(
            Rect2::new(cx - sw / 2.0, cy - sh / 2.0, sw, 1.0),
            0.0,
            Rgba::new(0.0, 0.0, 0.0, 0.8),
        )
        .rotated(angle, pivot),
    );
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_mapping_matches_the_defining_formula_with_frame_region() {
        // chassis at (200, 0, 350, 1080) inside a frame region at
        // (0, 0, 1200, 1080): roughly annunciator's bank sitting right of
        // its own frame's left margin, window 1200x1080.
        let chassis = Rect::new(200.0, 0.0, 350.0, 1080.0);
        let frame_region = Rect::new(0.0, 0.0, 1200.0, 1080.0);
        let fm = field_mapping(chassis, Some(frame_region));
        assert_eq!(fm.viewport, [1200.0, 1080.0]);
        assert!((fm.scale[0] - 350.0 / 1200.0).abs() < 1e-6);
        assert!((fm.scale[1] - 1080.0 / 1080.0).abs() < 1e-6);
        assert!((fm.offset[0] - 200.0 / 1200.0).abs() < 1e-6);
        assert_eq!(fm.offset[1], 0.0);
    }

    #[test]
    fn field_mapping_falls_back_before_frame_region_resolves() {
        let chassis = Rect::new(200.0, 0.0, 350.0, 1080.0);
        let fm = field_mapping(chassis, None);
        // field_width/field_height fall back to 1, field_offset to (0, 0).
        assert_eq!(fm.viewport, [1.0, 1.0]);
        assert_eq!(fm.scale, [350.0, 1080.0]);
        assert_eq!(fm.offset, [0.0, 0.0]);
    }

    #[test]
    fn field_mapping_floors_a_zero_width_frame_region_at_one() {
        // max(1, frame_region.width): a still-collapsing frame region
        // (width 0) must not divide the chassis's own scale by zero.
        let chassis = Rect::new(0.0, 0.0, 100.0, 50.0);
        let frame_region = Rect::new(0.0, 0.0, 0.0, 0.0);
        let fm = field_mapping(chassis, Some(frame_region));
        assert_eq!(fm.viewport, [1.0, 1.0]);
        assert_eq!(fm.scale, [100.0, 50.0]);
    }

    #[test]
    fn frame_viewport_size_matches_the_defining_formula_at_sampled_sizes() {
        // width / window_scaling, height / window_scaling, at a few (size,
        // scaling) pairs including a non-1 window_scaling (a HiDPI profile).
        for &(w, h, scaling) in &[
            (1200.0, 1080.0, 1.0),
            (2400.0, 2160.0, 2.0),
            (900.0, 600.0, 1.5),
        ] {
            let got = frame_viewport_size(w, h, scaling);
            assert_eq!(got, [(w / scaling) as f32, (h / scaling) as f32]);
        }
    }

    #[test]
    fn rgb_divides_by_255_like_a_hex_literal() {
        // The annunciator's casting colour is a hex colour literal, parsed
        // at /255, not a runtime-parsed string. Pinned again here since
        // every shell color goes through this function.
        let c = rgb("#16130f");
        assert!((c[0] - 0x16 as f32 / 255.0).abs() < 1e-6);
        assert!((c[1] - 0x13 as f32 / 255.0).abs() < 1e-6);
        assert!((c[2] - 0x0f as f32 / 255.0).abs() < 1e-6);
    }
}
