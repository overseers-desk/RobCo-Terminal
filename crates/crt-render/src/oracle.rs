//! CPU-side reimplementations of the shaders' math, used as the oracle
//! side of the done-tests: the GPU chain renders a pixel, this module
//! computes what that pixel should be from the same closed-form expressions
//! the shader itself uses, and the test compares the two.
//!
//! Every function here is a line-for-line translation of the corresponding
//! `.slang` file, not an independent re-derivation. Kept `#[cfg(test)]`-free
//! (used from `tests/*.rs` too) but not part of the crate's public API
//! surface beyond what the integration tests need.
//!
//! The three procedural metals' oracles left this module when `crates/chassis`
//! came into being (`chassis::oracle`): the chain stops at the glass, and metal
//! is chrome. `distort_coordinates` and `rounded_rect_sdf_pixels` are the two
//! that exist in both places, because each of `terminal_frame.slang` and
//! `frame_metal.slang` carries its own copy of them.

fn frac(x: f32) -> f32 {
    x - x.floor()
}

fn mix1(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}

fn smoothstep(min: f32, max: f32, x: f32) -> f32 {
    let t = ((x - min) / (max - min)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// `distort_coordinates`, shared by terminal_frame.slang and frame_metal.slang.
pub fn distort_coordinates(coords: [f32; 2], frame_size: f32, screen_curvature: f32) -> [f32; 2] {
    let padded = [
        coords[0] * (1.0 + frame_size * 2.0) - frame_size,
        coords[1] * (1.0 + frame_size * 2.0) - frame_size,
    ];
    let cc = [padded[0] - 0.5, padded[1] - 0.5];
    let dist = (cc[0] * cc[0] + cc[1] * cc[1]) * screen_curvature;
    [
        padded[0] + cc[0] * (1.0 + dist) * dist,
        padded[1] + cc[1] * (1.0 + dist) * dist,
    ]
}

pub fn rounded_rect_sdf_pixels(
    p: [f32; 2],
    top_left: [f32; 2],
    bottom_right: [f32; 2],
    radius_pixels: f32,
    viewport_size: [f32; 2],
) -> f32 {
    let size_px = [
        (bottom_right[0] - top_left[0]) * viewport_size[0],
        (bottom_right[1] - top_left[1]) * viewport_size[1],
    ];
    let center_px = [
        (top_left[0] + bottom_right[0]) * 0.5 * viewport_size[0],
        (top_left[1] + bottom_right[1]) * 0.5 * viewport_size[1],
    ];
    let local_px = [
        p[0] * viewport_size[0] - center_px[0],
        p[1] * viewport_size[1] - center_px[1],
    ];
    let half_size = [
        size_px[0] * 0.5 - radius_pixels,
        size_px[1] * 0.5 - radius_pixels,
    ];
    let d = [
        (local_px[0].abs() - half_size[0]).max(0.0),
        (local_px[1].abs() - half_size[1]).max(0.0),
    ];
    let inside = (local_px[0].abs() - half_size[0])
        .max(local_px[1].abs() - half_size[1])
        .min(0.0);
    (d[0] * d[0] + d[1] * d[1]).sqrt() + inside - radius_pixels
}

fn rand2(v: [f32; 2]) -> f32 {
    frac((v[0] * 12.9898 + v[1] * 78.233).sin() * 43758.5453)
}

pub struct TerminalFrameParams {
    pub screen_curvature: f32,
    pub frame_color: [f32; 4],
    pub frame_size: f32,
    pub screen_radius: f32,
    pub ambient_light: f32,
    pub frame_shininess: f32,
}

/// `terminal_frame.slang::main`, minus `noise` (a `sin`-based hash the CPU and
/// GPU will not agree on bit-for-bit; the test that uses this oracle checks
/// the noise-free channels and gives the noise-bearing ones a wider band).
pub fn terminal_frame(
    uv: [f32; 2],
    viewport: [f32; 2],
    p: &TerminalFrameParams,
) -> ([f32; 3], f32) {
    let coords = distort_coordinates(uv, p.frame_size, p.screen_curvature);
    let screen_radius_px = p.screen_radius;
    let edge_soft_px = 1.0f32;
    let seam_width = screen_radius_px.max(0.5) / viewport[0].min(viewport[1]);

    let e = smoothstep(-seam_width, seam_width, coords[0] - coords[1]).min(smoothstep(
        -seam_width,
        seam_width,
        coords[0] - (1.0 - coords[1]),
    ));
    let s = smoothstep(-seam_width, seam_width, coords[1] - coords[0]).min(smoothstep(
        -seam_width,
        seam_width,
        coords[0] - (1.0 - coords[1]),
    ));
    let w = smoothstep(-seam_width, seam_width, coords[1] - coords[0]).min(smoothstep(
        -seam_width,
        seam_width,
        (1.0 - coords[0]) - coords[1],
    ));
    let n = smoothstep(-seam_width, seam_width, coords[0] - coords[1]).min(smoothstep(
        -seam_width,
        seam_width,
        (1.0 - coords[0]) - coords[1],
    ));

    let dist_px =
        rounded_rect_sdf_pixels(coords, [0.0, 0.0], [1.0, 1.0], screen_radius_px, viewport);
    let mut frame_shadow = e * 0.66 + w * 0.66 + n * 0.33 + s;
    frame_shadow *= smoothstep(0.0, edge_soft_px * 5.0, dist_px);

    let frame_alpha = 1.0 - p.frame_shininess * 0.4;
    let in_screen = smoothstep(0.0, edge_soft_px, -dist_px);
    let alpha = mix1(frame_alpha, mix1(0.0, 0.3, p.ambient_light), in_screen);
    let glass = (p.ambient_light
        * (coords[0] * (1.0 - coords[1]) * coords[1] * (1.0 - coords[0]) * 25.0)
            .max(0.0)
            .powf(0.5)
        * in_screen)
        .clamp(0.0, 1.0);
    let frame_tint = [
        p.frame_color[0] * frame_shadow,
        p.frame_color[1] * frame_shadow,
        p.frame_color[2] * frame_shadow,
    ];
    let color = [
        mix1(frame_tint[0], glass, in_screen),
        mix1(frame_tint[1], glass, in_screen),
        mix1(frame_tint[2], glass, in_screen),
    ];
    (color, alpha)
}

/// Same computation, but returns the pieces needed to add the tolerance band
/// the `rand2`-based noise term needs: the un-noised tint plus the noise
/// value itself (computed the same way the shader does, `sin`-precision
/// caveats and all -- see the test for how the tolerance is set).
pub fn terminal_frame_noise(uv: [f32; 2], viewport: [f32; 2]) -> f32 {
    rand2([uv[0] * viewport[0], uv[1] * viewport[1]]) - 0.5
}

/// Analytic Gaussian used by `bloom_h.slang` / `bloom_v.slang`: 15 taps,
/// `i` in `[-7, 7]`, `sigma = radius / 3`. Given a 1-D input profile sampled
/// at `sample_fn`, returns the blurred value at texel offset `0`.
pub fn gaussian_1d_15tap(radius: f32, texel: f32, sample_fn: impl Fn(f32) -> f32) -> f32 {
    let sigma = (radius / 3.0).max(1e-4);
    let mut sum = 0.0f32;
    let mut wsum = 0.0f32;
    for i in -7..=7 {
        let x = i as f32 * (radius / 7.0);
        let w = (-0.5 * (x * x) / (sigma * sigma)).exp();
        sum += sample_fn(x * texel) * w;
        wsum += w;
    }
    sum / wsum.max(1e-6)
}
