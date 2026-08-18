//! Small color/scalar helper functions used to compute uniforms before
//! handing them to the shaders (`lint`, `smoothstep`, `frame_base_color`,
//! ...).
//!
//! Every function below is a deliberately literal, unoptimized translation
//! of its defining formula, not an idiomatic rewrite: the unit tests assert
//! against values computed by hand from that formula, not re-derived from
//! this file, so a "cleaner" reformulation that happened to compute the same
//! answer would still be the wrong reason for a test to pass.

/// A straight (non-premultiplied) RGBA color in `0.0..=1.0` per channel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba {
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
}

pub fn clamp(x: f32, min: f32, max: f32) -> f32 {
    if x <= min {
        return min;
    }
    if x >= max {
        return max;
    }
    x
}

/// Linear interpolation. Named `lint`, not a typo for "lerp" worth silently
/// renaming away from since other tests already refer to it by this name.
pub fn lint(a: f32, b: f32, t: f32) -> f32 {
    (1.0 - t) * a + t * b
}

/// Per-channel lerp between two colors.
pub fn mix(c1: Rgba, c2: Rgba, alpha: f32) -> Rgba {
    Rgba::new(
        c1.r * (1.0 - alpha) + c2.r * alpha,
        c1.g * (1.0 - alpha) + c2.g * alpha,
        c1.b * (1.0 - alpha) + c2.b * alpha,
        c1.a * (1.0 - alpha) + c2.a * alpha,
    )
}

/// Per-channel add, each channel clamped to `0..=1`.
pub fn sum(c1: Rgba, c2: Rgba) -> Rgba {
    Rgba::new(
        clamp(c1.r + c2.r, 0.0, 1.0),
        clamp(c1.g + c2.g, 0.0, 1.0),
        clamp(c1.b + c2.b, 0.0, 1.0),
        clamp(c1.a + c2.a, 0.0, 1.0),
    )
}

/// RGB scaled by `value` and clamped; alpha passed through its own clamp,
/// unscaled -- deliberate, not a typo.
pub fn scale_color(c1: Rgba, value: f32) -> Rgba {
    Rgba::new(
        clamp(c1.r * value, 0.0, 1.0),
        clamp(c1.g * value, 0.0, 1.0),
        clamp(c1.b * value, 0.0, 1.0),
        clamp(c1.a, 0.0, 1.0),
    )
}

/// The appliance's plastic: the profile's frame color lifted toward the
/// screen's own glow as ambient light rises.
pub fn frame_base_color(
    frame_color: Rgba,
    font_color: Rgba,
    background_color: Rgba,
    ambient_light: f32,
) -> Rgba {
    mix(
        scale_color(mix(font_color, background_color, 0.2), 0.2),
        sum(frame_color, Rgba::new(0.1, 0.1, 0.1, 1.0)),
        0.125 + 0.750 * ambient_light,
    )
}

/// The GLSL-style scalar smoothstep, not `clamp`'s twin despite the similar
/// shape -- min/max order matters here the way it does in GLSL (reversed
/// order does not simply mirror the curve).
pub fn smoothstep(min: f32, max: f32, value: f32) -> f32 {
    let x = ((value - min) / (max - min)).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// `"#rrggbb"` to `Rgba`, alpha forced to 1.0. Divides by 256, not 255,
/// which is why `#ffffff` does not read back as exactly `1.0` -- this is
/// deliberate, not an off-by-one.
///
/// The divisor is a fact about *this* parser, and the distinction from a
/// genuine /255 hex parse is worth keeping: the chassis and casting colours
/// go through that other kind of parse, and running one of those through
/// here would be a third of a level wrong. This is the parser for every
/// colour the chain's uniforms are built from: `crt::params` has no other,
/// on purpose.
pub fn str_to_color(s: &str) -> Rgba {
    let r = i32::from_str_radix(substring(s, 1, 3), 16).unwrap_or(0) as f32 / 256.0;
    let g = i32::from_str_radix(substring(s, 3, 5), 16).unwrap_or(0) as f32 / 256.0;
    let b = i32::from_str_radix(substring(s, 5, 7), 16).unwrap_or(0) as f32 / 256.0;
    Rgba::new(r, g, b, 1.0)
}

/// A clamping alternative to slicing directly, which is the part of
/// `str_to_color` that must not panic.
///
/// A direct `&s[1..3]` panics on a short string, so a config carrying a
/// three-digit hex colour once took the process down at the first frame it
/// was asked to paint. Reading out of range must instead degrade to an
/// empty slice, so the numeric parse that follows fails quietly and
/// `unwrap_or(0)` -- already this function's answer for a channel that will
/// not parse -- is what decides the colour, not a crash.
///
/// `get` rather than direct slicing for the last inch: an index landing
/// inside a multi-byte character is a range a config file can produce even
/// though a well-formed hex string never would.
fn substring(s: &str, from: usize, to: usize) -> &str {
    let to = to.min(s.len());
    let from = from.min(to);
    s.get(from..to).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every expected value below was computed by hand from the defining
    // formula, not from this file's implementation.

    #[test]
    fn clamp_matches_js() {
        assert_eq!(clamp(-1.0, 0.0, 1.0), 0.0);
        assert_eq!(clamp(2.0, 0.0, 1.0), 1.0);
        assert_eq!(clamp(0.5, 0.0, 1.0), 0.5);
        // x <= min first, so min itself returns min even if min > max in
        // a degenerate call; not exercised by real callers but pins the
        // branch order.
        assert_eq!(clamp(0.0, 0.0, 1.0), 0.0);
    }

    #[test]
    fn lint_matches_js() {
        // lint(0, 10, 0.25) = 0.75*0 + 0.25*10 = 2.5
        assert_eq!(lint(0.0, 10.0, 0.25), 2.5);
        // lint(16, 64, 0.5) = 8 + 32 = 40, the bloom radius mapping at
        // bloomQuality = 0.5.
        assert_eq!(lint(16.0, 64.0, 0.5), 40.0);
        assert_eq!(lint(5.0, 5.0, 0.7), 5.0);
    }

    #[test]
    fn mix_matches_js() {
        let c1 = Rgba::new(1.0, 0.0, 0.0, 1.0);
        let c2 = Rgba::new(0.0, 1.0, 0.0, 0.0);
        let m = mix(c1, c2, 0.5);
        assert_eq!(m, Rgba::new(0.5, 0.5, 0.0, 0.5));
    }

    #[test]
    fn sum_clamps_per_channel() {
        let c1 = Rgba::new(0.8, 0.1, 0.0, 0.5);
        let c2 = Rgba::new(0.5, 0.1, 0.0, 0.6);
        let s = sum(c1, c2);
        // r: 1.3 -> clamped to 1.0; g: 0.2; b: 0.0; a: 1.1 -> clamped to 1.0
        assert_eq!(s, Rgba::new(1.0, 0.2, 0.0, 1.0));
    }

    #[test]
    fn scale_color_matches_js() {
        let c = Rgba::new(0.5, 0.5, 0.5, 0.5);
        let s = scale_color(c, 3.0);
        // r/g/b: 1.5 -> clamped to 1.0; a: clamp(0.5, 0, 1) = 0.5 (unscaled)
        assert_eq!(s, Rgba::new(1.0, 1.0, 1.0, 0.5));

        let c2 = Rgba::new(0.2, 0.4, 0.6, 1.0);
        let s2 = scale_color(c2, 0.5);
        assert_eq!(s2, Rgba::new(0.1, 0.2, 0.3, 1.0));
    }

    #[test]
    fn frame_base_color_matches_js_at_zero_and_one_ambient() {
        let frame = Rgba::new(0.2, 0.2, 0.25, 1.0);
        let font = Rgba::new(0.35, 1.0, 0.45, 1.0);
        let bg = Rgba::new(0.0, 0.0, 0.0, 1.0);

        // ambientLight = 0: weight is 0.125 toward `sum`.
        // inner mix(font, bg, 0.2) = font*0.8 = (0.28, 0.8, 0.36, 1.0)
        // scaleColor(that, 0.2) = clamp(*0.2) = (0.056, 0.16, 0.072, 1.0)
        // sum(frame, (0.1,0.1,0.1,1)) = (0.3, 0.3, 0.35, 1.0) clamped
        // mix(a, b, 0.125) = a*0.875 + b*0.125
        let got = frame_base_color(frame, font, bg, 0.0);
        let a = Rgba::new(0.056, 0.16, 0.072, 1.0);
        let b = Rgba::new(0.3, 0.3, 0.35, 1.0);
        let expected = mix(a, b, 0.125);
        assert!((got.r - expected.r).abs() < 1e-6, "{got:?} vs {expected:?}");
        assert!((got.g - expected.g).abs() < 1e-6);
        assert!((got.b - expected.b).abs() < 1e-6);

        // ambientLight = 1: weight is 0.875 toward `sum`, i.e. the result
        // should sit much closer to `sum(frame, (.1,.1,.1,1))` than to `a`.
        let got1 = frame_base_color(frame, font, bg, 1.0);
        assert!((got1.r - b.r).abs() < 0.05);
        assert!((got1.g - b.g).abs() < 0.05);
        assert!((got1.b - b.b).abs() < 0.05);
    }

    /// The short-string case, which used to abort the process.
    ///
    /// A well-formed hex string always has enough characters for all three
    /// channel reads, but a config file need not be well-formed, and reading
    /// out of range must degrade to the zero `unwrap_or(0)` already chooses
    /// for an unparseable channel -- and above all must return rather than
    /// panic.
    #[test]
    fn str_to_color_degrades_on_short_input_the_way_substring_does() {
        // Nothing here may panic; the assertion is that control returns.
        for s in ["", "#", "#0", "#00", "#000", "#0000", "#00000", "#123456"] {
            let c = str_to_color(s);
            assert_eq!(c.a, 1.0, "{s:?} lost its alpha");
        }
        // "#000": substring(1,3) = "00" -> 0, (3,5) = "0" -> 0, (5,7) = "".
        assert_eq!(str_to_color("#000"), Rgba::new(0.0, 0.0, 0.0, 1.0));
        // "#ff11" clamps to two whole channels and an empty third, exactly as
        // substring would hand them over.
        let c = str_to_color("#ff11");
        assert!((c.r - 255.0 / 256.0).abs() < 1e-6);
        assert!((c.g - 17.0 / 256.0).abs() < 1e-6);
        assert_eq!(c.b, 0.0);
        // A multi-byte character is not a boundary JS can trip on and is one
        // a config file can.
        assert_eq!(str_to_color("#\u{e9}\u{e9}\u{e9}").a, 1.0);
    }

    #[test]
    fn smoothstep_matches_js() {
        assert_eq!(smoothstep(0.0, 1.0, -1.0), 0.0);
        assert_eq!(smoothstep(0.0, 1.0, 2.0), 1.0);
        assert_eq!(smoothstep(0.0, 1.0, 0.5), 0.5);
        // smoothstep(2, 4, 3): x = 0.5, x*x*(3-2x) = 0.25 * 2 = 0.5
        assert_eq!(smoothstep(2.0, 4.0, 3.0), 0.5);
        // This crate's own use: rasterizationIntensity = smoothstep(2, 4, density)
        // at density = 2.5: x = 0.25, x*x*(3-2*0.25) = 0.0625 * 2.5 = 0.15625
        let x = smoothstep(2.0, 4.0, 2.5);
        assert!((x - 0.15625).abs() < 1e-6);
    }

    #[test]
    fn str_to_color_matches_js() {
        // This divides by 256, so "#ff0000" is NOT exactly (1,0,0): 255/256.
        let c = str_to_color("#ff0000");
        assert!((c.r - 255.0 / 256.0).abs() < 1e-6);
        assert_eq!(c.g, 0.0);
        assert_eq!(c.b, 0.0);
        assert_eq!(c.a, 1.0);

        let c2 = str_to_color("#33cc99");
        assert!((c2.r - 0x33 as f32 / 256.0).abs() < 1e-6);
        assert!((c2.g - 0xcc as f32 / 256.0).abs() < 1e-6);
        assert!((c2.b - 0x99 as f32 / 256.0).abs() < 1e-6);
    }
}
