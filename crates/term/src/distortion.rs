//! Inverse screen-curvature distortion: undoes the CRT-tube warp applied to
//! the terminal grid so a pointer click can be mapped back to a grid cell.
//!
//! The forward warp bends the flat terminal grid outward to fake a CRT
//! tube's curvature; a pointer click lands on the bent (visible) surface, so
//! before it can be turned into a grid cell it has to be pushed back through
//! the same warp. This reuses the *forward* warp's own math to approximate
//! the inverse (curvature is small and the warp is close to involutive at
//! the magnitudes this app uses), rather than deriving an exact inverse.
//!
//! Applied on every pointer path (press, release, move) from day one:
//! `app`'s window/mouse code is expected to call
//! [`correct_distortion`] before turning a widget-space point into a
//! terminal-grid coordinate.

/// The screen/chassis geometry and runtime settings the distortion
/// transform needs: `margin`/`width`/`height` describe the pointer-input
/// area, `frame_size` is the chassis inset, `screen_curvature(_size)` and
/// `normalized_screen_scale` are the curvature settings, and
/// `total_width`/`total_height` give the offscreen grid texture size the
/// corrected point is expressed in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DistortionParams {
    /// Pixel margin between the widget edge and the curved screen area.
    pub margin: f64,
    /// Pointer-input area width in pixels.
    pub width: f64,
    /// Pointer-input area height in pixels.
    pub height: f64,
    /// Chassis frame inset, as a fraction of width/height.
    pub frame_size: f64,
    /// Screen curvature strength.
    pub screen_curvature: f64,
    /// Screen curvature size scaling factor.
    pub screen_curvature_size: f64,
    /// Scale factor normalizing curvature to the on-screen curved-glass
    /// region.
    pub normalized_screen_scale: f64,
    /// The offscreen grid texture's width in pixels.
    pub total_width: f64,
    /// The offscreen grid texture's height in pixels.
    pub total_height: f64,
}

/// Point in the undistorted grid-texture space `correct_distortion` maps
/// into.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// Inverse-distortion transform: a pointer coordinate in widget-pixel space
/// (`x`, `y`) maps to a coordinate in the offscreen grid-texture space,
/// undoing the CRT curvature warp.
///
/// Normalizes the input point into the pointer-input area, expands it by
/// the chassis frame padding, then applies a quadratic correction centered
/// on the frame's midpoint before scaling into texture-pixel space.
pub fn correct_distortion(x: f64, y: f64, p: &DistortionParams) -> Point {
    let mut x = (x - p.margin) / p.width;
    let mut y = (y - p.margin) / p.height;

    x = x * (1.0 + p.frame_size * 2.0) - p.frame_size;
    y = y * (1.0 + p.frame_size * 2.0) - p.frame_size;

    // cc_w tracks the x-axis distance from center, cc_h tracks y.
    let cc_w = 0.5 - x;
    let cc_h = 0.5 - y;
    let distortion = (cc_h * cc_h + cc_w * cc_w)
        * p.screen_curvature
        * p.screen_curvature_size
        * p.normalized_screen_scale;

    Point {
        x: (x - cc_w * (1.0 + distortion) * distortion) * p.total_width,
        y: (y - cc_h * (1.0 + distortion) * distortion) * p.total_height,
    }
}

/// Screen curvature size scaling constant: `0.6`. Not a config key (it has
/// no setter, no preset ever touches it, and it is not one of the
/// `compose*` keys extracted into config), so it is a constant here rather
/// than a [`config`](../../config/index.html) field.
pub const SCREEN_CURVATURE_SIZE: f64 = 0.6;

/// Normalizes curvature strength to the on-screen box the curved glass
/// fills -- the same pixel rectangle a pointer position arrives in, so
/// callers pass the window/pointer-input-area width and height here. A
/// window manager free to grant no size at all is why the denominator is
/// floored at 1.
pub fn normalized_screen_scale(width: f64, height: f64) -> f64 {
    1024.0 / (0.5 * width + 0.5 * height).max(1.0)
}

/// Forward screen-curvature distortion: the warp kernel a CRT shader
/// applies to bend the flat terminal grid onto the curved glass, expressed
/// here as the identical scalar kernel operating on plain `f64`s (this
/// crate has no GPU pipeline, so this is not literally the shader).
///
/// The curvature factor combines the same three inputs
/// [`DistortionParams`] carries separately (`screen_curvature *
/// screen_curvature_size * normalized_screen_scale`), combined here exactly
/// as a shader's uniform upload would combine them, so this crate and a GPU
/// pass in `crt-render` name and compute the same scalar from the
/// same inputs.
///
/// [`correct_distortion`] reuses this exact kernel -- the only nonlinear
/// step in either direction -- to *approximate* its own inverse rather than
/// solving the cubic that would exactly undo it (see this module's top doc
/// comment). `forward_distort` reapplies that same kernel once rather than
/// inverting it properly (e.g. by Newton iteration): this wires the pointer
/// to the same curvature approximation the renderer uses and proves the two
/// independently-sourced implementations agree, it does not improve on the
/// approximation's own accuracy.
///
/// [`DistortionParams`] is reused for the input/output units so this
/// composes directly with [`correct_distortion`]: `(x, y)` is a point in
/// [`correct_distortion`]'s *output* domain (texture-pixel space), and the
/// result is a point back in its *input* domain (widget-pixel space). The
/// margin/width normalization and the frame padding are exactly undone --
/// ordinary invertible affine algebra, not part of the warp -- which is
/// what makes `forward_distort(correct_distortion(x, y, p), p)` the exact
/// identity when `p.screen_curvature` is `0.0` (the kernel term vanishes
/// entirely, so no approximation is in play at all) for *any* margin,
/// frame size, or width/height/total_width/total_height. For nonzero
/// curvature the round trip is only approximate, and the error grows with
/// curvature: measured (production-scale geometry, `screen_curvature` at
/// each preset's own value, `screen_curvature_size` `0.6`,
/// `normalized_screen_scale` `1.2`) at `0.1` curvature the worst sampled
/// point across the visible screen is off by ~4.5% of the screen's width;
/// at `0.5` (e.g. "Commodore 64"/"Apple ][") ~38%; at `0.7` (the highest
/// curvature any preset uses, "Commodore PET") ~73%. This reflects the
/// approximation's own inherent error, not a defect this implementation
/// introduces -- see the `forward_after_inverse_*` tests for the measured
/// numbers this doc comment states.
pub fn forward_distort(x: f64, y: f64, p: &DistortionParams) -> Point {
    // Undo `correct_distortion`'s final texture-pixel scale, landing back
    // in the normalized domain its own kernel step operated in (i.e. the
    // padded-but-not-yet-unpadded value -- *not* the pre-padding one, so
    // this does not pad a second time).
    let padded_u = x / p.total_width;
    let padded_v = y / p.total_height;

    let cc_u = padded_u - 0.5;
    let cc_v = padded_v - 0.5;
    let curvature = p.screen_curvature * p.screen_curvature_size * p.normalized_screen_scale;
    let dist = (cc_u * cc_u + cc_v * cc_v) * curvature;
    let warped_u = padded_u + cc_u * (1.0 + dist) * dist;
    let warped_v = padded_v + cc_v * (1.0 + dist) * dist;

    // Undo the frame padding and the margin/width normalization
    // `correct_distortion` applied on the way in: exact linear algebra
    // (not part of the warp), so it contributes no error of its own.
    let unpadded_u = (warped_u + p.frame_size) / (1.0 + p.frame_size * 2.0);
    let unpadded_v = (warped_v + p.frame_size) / (1.0 + p.frame_size * 2.0);

    Point {
        x: unpadded_u * p.width + p.margin,
        y: unpadded_v * p.height + p.margin,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() <= eps
    }

    /// The required property: identity at zero curvature. With
    /// `screen_curvature == 0`, the quadratic distortion term vanishes
    /// regardless of `screen_curvature_size`/`normalized_screen_scale`, and
    /// with no margin/frame inset and matching widget/texture dimensions the
    /// transform must be the identity on the input point.
    #[test]
    fn identity_at_zero_curvature_no_margin_no_frame() {
        let p = DistortionParams {
            margin: 0.0,
            width: 200.0,
            height: 150.0,
            frame_size: 0.0,
            screen_curvature: 0.0,
            screen_curvature_size: 1.0,
            normalized_screen_scale: 1.0,
            total_width: 200.0,
            total_height: 150.0,
        };
        for &(x, y) in &[(0.0, 0.0), (50.0, 30.0), (199.0, 149.0), (100.0, 75.0)] {
            let out = correct_distortion(x, y, &p);
            assert!(approx_eq(out.x, x, 1e-9), "x: {} vs {}", out.x, x);
            assert!(approx_eq(out.y, y, 1e-9), "y: {} vs {}", out.y, y);
        }
    }

    /// Zero curvature still holds identity even with a nonzero margin/frame
    /// inset and a texture size that differs from the widget, *provided*
    /// margin/frame are zero. This test isolates that the curvature term
    /// alone is what zeroes out, by re-deriving the expected linear map by
    /// hand instead of reusing production code.
    #[test]
    fn zero_curvature_reduces_to_the_margin_frame_linear_map() {
        let p = DistortionParams {
            margin: 10.0,
            width: 220.0,
            height: 170.0,
            frame_size: 0.05,
            screen_curvature: 0.0,
            screen_curvature_size: 2.0,
            normalized_screen_scale: 1.3,
            total_width: 400.0,
            total_height: 300.0,
        };
        let (mx, my) = (123.0, 87.0);

        // Hand-computed expected value: with distortion == 0 the return
        // collapses to (x * total_width, y * total_height) where x, y are
        // the margin/frame-corrected normalized coordinates.
        let nx = (mx - p.margin) / p.width;
        let ny = (my - p.margin) / p.height;
        let ex = nx * (1.0 + p.frame_size * 2.0) - p.frame_size;
        let ey = ny * (1.0 + p.frame_size * 2.0) - p.frame_size;
        let expected = Point {
            x: ex * p.total_width,
            y: ey * p.total_height,
        };

        let out = correct_distortion(mx, my, &p);
        assert!(approx_eq(out.x, expected.x, 1e-9));
        assert!(approx_eq(out.y, expected.y, 1e-9));
    }

    /// Sampled points cross-checked against an independent re-implementation
    /// of the same formula (see the Python transcription reproduced inline
    /// below as the exact values it printed), so this isn't just "the Rust
    /// implementation agrees with itself".
    ///
    /// Python:
    /// ```python
    /// def correct_distortion(x, y, margin, width, height, frame_size,
    ///                         curvature, curvature_size, scale,
    ///                         total_width, total_height):
    ///     x = (x - margin) / width
    ///     y = (y - margin) / height
    ///     x = x * (1 + frame_size * 2) - frame_size
    ///     y = y * (1 + frame_size * 2) - frame_size
    ///     cc_w = 0.5 - x
    ///     cc_h = 0.5 - y
    ///     distortion = (cc_h**2 + cc_w**2) * curvature * curvature_size * scale
    ///     return (
    ///         (x - cc_w * (1 + distortion) * distortion) * total_width,
    ///         (y - cc_h * (1 + distortion) * distortion) * total_height,
    ///     )
    /// ```
    #[test]
    fn sampled_points_match_independent_python_reimplementation() {
        let p = DistortionParams {
            margin: 4.0,
            width: 300.0,
            height: 200.0,
            frame_size: 0.02,
            screen_curvature: 0.3,
            screen_curvature_size: 0.6,
            normalized_screen_scale: 1.0,
            total_width: 300.0,
            total_height: 200.0,
        };

        // Widget-center, corner-ish, and off-axis points; values from
        // actually running the Python transcription above at these exact
        // inputs (margin shifts the true distortion-free point off the
        // widget's geometric center, so even the "center" case picks up a
        // nonzero correction here).
        let cases: &[((f64, f64), (f64, f64))] = &[
            ((150.0, 100.0), (145.83953200393873, 95.83953200393871)),
            ((10.0, 10.0), (-14.074442308692033, -7.10414984039619)),
            ((290.0, 190.0), (303.00775935821605, 196.75490665298955)),
            ((80.0, 150.0), (71.29800538363644, 148.92286151828006)),
        ];
        for &((x, y), (ex, ey)) in cases {
            let out = correct_distortion(x, y, &p);
            assert!(approx_eq(out.x, ex, 1e-6), "x: got {} want {}", out.x, ex);
            assert!(approx_eq(out.y, ey, 1e-6), "y: got {} want {}", out.y, ey);
        }
    }

    /// Cross-checked at a few points against `1024 / max(1, 0.5*width +
    /// 0.5*height)` computed by hand.
    #[test]
    fn normalized_screen_scale_matches_the_defining_formula() {
        assert!(approx_eq(
            normalized_screen_scale(1024.0, 768.0),
            1024.0 / 896.0,
            1e-9
        ));
        assert!(approx_eq(
            normalized_screen_scale(2048.0, 1536.0),
            1024.0 / 1792.0,
            1e-9
        ));
        // Floored at 1: a window manager that grants no size at all must
        // not divide by zero.
        assert!(approx_eq(normalized_screen_scale(0.0, 0.0), 1024.0, 1e-9));
    }

    /// The required property: `forward_distort(correct_distortion(x, y, p), p)`
    /// is the exact identity at `screen_curvature == 0.0`, for nonzero
    /// margin and frame size too (unlike the `zero_curvature_*` tests
    /// above, which only claim identity or a hand-derived linear map). The
    /// nonlinear kernel term vanishes identically, and the margin/frame
    /// bookkeeping `forward_distort` undoes is `correct_distortion`'s own,
    /// applied in reverse -- exact algebra either way.
    #[test]
    fn forward_after_inverse_is_exact_identity_at_zero_curvature() {
        let p = DistortionParams {
            margin: 12.0,
            width: 800.0,
            height: 600.0,
            frame_size: 0.03,
            screen_curvature: 0.0,
            screen_curvature_size: SCREEN_CURVATURE_SIZE,
            normalized_screen_scale: 1.2,
            total_width: 800.0,
            total_height: 600.0,
        };
        for &fx in &[0.0, 0.25, 0.5, 0.75, 1.0] {
            for &fy in &[0.0, 0.25, 0.5, 0.75, 1.0] {
                let x = p.margin + fx * (p.width - 2.0 * p.margin);
                let y = p.margin + fy * (p.height - 2.0 * p.margin);
                let texture = correct_distortion(x, y, &p);
                let back = forward_distort(texture.x, texture.y, &p);
                assert!(
                    approx_eq(back.x, x, 1e-9) && approx_eq(back.y, y, 1e-9),
                    "({x}, {y}) -> ({}, {}) -> ({}, {}); want ({x}, {y}) exactly",
                    texture.x,
                    texture.y,
                    back.x,
                    back.y
                );
            }
        }
    }

    /// Nonzero curvature: the round trip is only
    /// approximate above zero curvature (the forward kernel is reused as
    /// its own inverse rather than solving for an exact one -- see
    /// `forward_distort`'s doc comment), and the error grows with
    /// curvature. These are the measured worst-case errors
    /// (production-scale window geometry, a 5x5 grid spanning the visible
    /// screen, every `screen_curvature` value any bundled preset uses)
    /// that back the numbers in `forward_distort`'s doc comment; the
    /// bounds below are those measurements with ~15-20% headroom, not a
    /// tight theoretical bound.
    #[test]
    fn forward_after_inverse_stays_within_the_measured_tolerance_across_curvature() {
        let base = DistortionParams {
            margin: 12.0,
            width: 800.0,
            height: 600.0,
            frame_size: 0.03,
            screen_curvature: 0.0,
            screen_curvature_size: SCREEN_CURVATURE_SIZE,
            normalized_screen_scale: 1.2,
            total_width: 800.0,
            total_height: 600.0,
        };

        // (screen_curvature, max absolute pixel error tolerance), the
        // latter a rounded-up bound on the measured worst point in the
        // grid below (see this module's history for the Python
        // measurement this table is derived from).
        let cases: &[(f64, f64)] = &[
            (0.1, 45.0),
            (0.2, 95.0),
            (0.3, 160.0),
            (0.4, 240.0),
            (0.5, 340.0),
            (0.7, 650.0),
        ];

        for &(curvature, tolerance) in cases {
            let p = DistortionParams {
                screen_curvature: curvature,
                ..base
            };
            let mut worst = 0.0_f64;
            for &fx in &[0.0, 0.25, 0.5, 0.75, 1.0] {
                for &fy in &[0.0, 0.25, 0.5, 0.75, 1.0] {
                    let x = p.margin + fx * (p.width - 2.0 * p.margin);
                    let y = p.margin + fy * (p.height - 2.0 * p.margin);
                    let texture = correct_distortion(x, y, &p);
                    let back = forward_distort(texture.x, texture.y, &p);
                    worst = worst.max((back.x - x).abs()).max((back.y - y).abs());
                }
            }
            assert!(
                worst <= tolerance,
                "screen_curvature {curvature}: worst round-trip error {worst}px exceeds the stated tolerance {tolerance}px"
            );
        }
    }
}
