//! Standalone proof + done-tests for `shaders/bloom/{bloom_h,bloom_v}.slang`.
//!
//! A deliberate choice, recorded in `bloom_h.slang`'s header: a
//! radius-parameterized blur feeding the same composite in
//! terminal_static.slang, built as a standard two-pass separable Gaussian
//! rather than a box-blur pyramid. Being this crate's own construction, it
//! is fully analytic and exactly specified, so the done-tests assert
//! against closed-form properties of a Gaussian blur, and against
//! `oracle::gaussian_blur_1d`, the kernel as a continuous integral rather
//! than as the shader's taps.
//!
//! Two presets stand in for the chain's two regimes. `bloom_h.slangp` and
//! `bloom.slangp` render at the input's own size, where an output pixel is a
//! source texel. `bloom_half.slangp` renders at half of it, the chain's
//! default `bloomQuality`, where an output pixel spans two source texels and
//! the taps must tighten to keep reading every one of them; the chain's
//! default radius there is 40, and `bloom_half_*` tests use that number.

use oracle;
use crt_burnin::headless;
use std::path::PathBuf;

const W: u32 = 64;
const H: u32 = 64;

/// See `uv_of` in `robco-chassis`'s `tests/chassis_metal.rs` for the empirical
/// confirmation that readback row maps to texcoord v with no flip.
fn uv_of(c: u32, r: u32, w: u32, h: u32) -> [f32; 2] {
    [(c as f32 + 0.5) / w as f32, (r as f32 + 0.5) / h as f32]
}

fn shader_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("shaders/bloom")
}

/// An RGBA8 image of `w*h` pixels, opaque, with `lit(x, y)` deciding which
/// pixels are white and which black.
fn mask_image(w: u32, h: u32, lit: impl Fn(u32, u32) -> bool) -> Vec<u8> {
    let mut input = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let v = if lit(x, y) { 255 } else { 0 };
            let i = ((y * w + x) * 4) as usize;
            input[i] = v;
            input[i + 1] = v;
            input[i + 2] = v;
            input[i + 3] = 255;
        }
    }
    input
}

/// The oracle's view of a lit column band `x0..=x1` of a `src_w`-wide source,
/// read as the box-reconstructed texture it is: 1 inside the band's texels,
/// 0 outside.
fn column_band(src_w: u32, x0: u32, x1: u32, uv_x: f32) -> impl Fn(f32) -> f32 {
    move |offset| {
        let texel_idx = ((uv_x + offset) * src_w as f32).floor() as i64;
        if texel_idx >= x0 as i64 && texel_idx <= x1 as i64 {
            1.0
        } else {
            0.0
        }
    }
}

/// A blur's weights sum to 1: a constant field must come back unchanged.
#[test]
fn bloom_constant_field_is_unchanged() {
    let gpu = headless::Gpu::new().expect("headless wgpu device");
    let preset = shader_dir().join("bloom.slangp");
    let mut input = vec![0u8; (W * H * 4) as usize];
    for px in input.chunks_exact_mut(4) {
        px[0] = 128;
        px[1] = 200;
        px[2] = 40;
        px[3] = 255;
    }
    let out = headless::render_single_pass(&gpu, &preset, &[("radius", 16.0)], W, H, &input);
    let (c, r) = (32, 32);
    let px = out[headless::px_index(W, c, r)];
    let expected = [128.0 / 255.0, 200.0 / 255.0, 40.0 / 255.0];
    for i in 0..3 {
        assert!(
            (px[i] - expected[i]).abs() < 0.01,
            "channel {i}: gpu={} expected={}",
            px[i],
            expected[i]
        );
    }
}

/// A linear ramp sampled with a symmetric kernel returns the ramp's own
/// value at the sample point (bilinear texture lookups of a piecewise-linear
/// texture are themselves exactly linear between texel centers, and a
/// symmetric weighted average of a linear function equals its center value).
/// Checked away from both the horizontal and vertical edges, where
/// `clamp_to_edge` would break the linearity assumption.
#[test]
fn bloom_linear_ramp_is_unchanged_away_from_edges() {
    let gpu = headless::Gpu::new().expect("headless wgpu device");
    let preset = shader_dir().join("bloom.slangp");
    let mut input = vec![0u8; (W * H * 4) as usize];
    for y in 0..H {
        for x in 0..W {
            let v = (x as f32 / (W - 1) as f32 * 255.0).round() as u8;
            let i = ((y * W + x) * 4) as usize;
            input[i] = v;
            input[i + 1] = v;
            input[i + 2] = v;
            input[i + 3] = 255;
        }
    }
    let radius = 16.0;
    let out = headless::render_single_pass(&gpu, &preset, &[("radius", radius)], W, H, &input);

    // radius 16 taps reach +/-16px; stay >20px from every edge.
    for &c in &[24u32, 32, 40] {
        let r = 32u32;
        let px = out[headless::px_index(W, c, r)];
        let expected = c as f32 / (W - 1) as f32;
        assert!(
            (px[0] - expected).abs() < 0.02,
            "column {c}: gpu={} expected~={expected}",
            px[0]
        );
    }
}

/// `bloom_h` alone at the input's own size: a one-texel bright column comes
/// out as the oracle's Gaussian of it, at the column and either side of it.
#[test]
fn bloom_h_matches_oracle() {
    let gpu = headless::Gpu::new().expect("headless wgpu device");
    let preset = shader_dir().join("bloom_h.slangp");
    let input = mask_image(W, H, |x, _| x == 32);

    let radius = 14.0f32;
    let out = headless::render_single_pass(&gpu, &preset, &[("radius", radius)], W, H, &input);

    let texel = 1.0 / W as f32;
    for &c in &[28u32, 32, 36] {
        let r = 10u32;
        let uv = uv_of(c, r, W, H);
        let expected = oracle::gaussian_blur_1d(radius, texel, column_band(W, 32, 32, uv[0]));
        let px = out[headless::px_index(W, c, r)];
        assert!(
            (px[0] - expected).abs() < 0.01,
            "column {c}: gpu={} oracle={expected}",
            px[0]
        );
    }
}

/// The chain's regime: `bloom_h` into a framebuffer half the source's size,
/// radius 40. A three-texel bright column comes out as the oracle's Gaussian
/// at every output pixel across the halo, including the ones between where
/// taps spaced `radius / 7` would have landed, and falls off monotonically
/// from the column: no echo of the column at any pitch.
#[test]
fn bloom_half_h_halo_is_the_oracle_gaussian_and_monotone() {
    const SRC_W: u32 = 192;
    const SRC_H: u32 = 16;
    let gpu = headless::Gpu::new().expect("headless wgpu device");
    let preset = shader_dir().join("bloom_half_h.slangp");
    let input = mask_image(SRC_W, SRC_H, |x, _| (95..=97).contains(&x));

    let radius = 40.0f32;
    let (out_w, out_h) = (SRC_W / 2, SRC_H / 2);
    let out = headless::render_single_pass_io(
        &gpu,
        &preset,
        &[("radius", radius)],
        SRC_W,
        SRC_H,
        out_w,
        out_h,
        &input,
    );

    let texel = 1.0 / out_w as f32;
    let r = out_h / 2;
    let centre = out_w / 2; // output pixel 48 spans source texels 96..=97
    let mut previous = f32::INFINITY;
    for c in centre..=centre + 44 {
        let uv = uv_of(c, r, out_w, out_h);
        let expected = oracle::gaussian_blur_1d(radius, texel, column_band(SRC_W, 95, 97, uv[0]));
        let px = out[headless::px_index(out_w, c, r)];
        assert!(
            (px[0] - expected).abs() < 0.01,
            "column {c}: gpu={} oracle={expected}",
            px[0]
        );
        // One 8-bit level of slack for the framebuffer's quantisation.
        assert!(
            px[0] <= previous + 1.0 / 255.0 + 1e-4,
            "column {c}: gpu={} rose above column {}'s {previous}",
            px[0],
            c - 1
        );
        previous = px[0];
    }
}

/// Both passes into a half-size framebuffer on a lit block: the halo is the
/// product of the two 1-D oracles (a separable kernel on a separable input),
/// checked along the row and the column through the block, which is where a
/// comb in either pass would put its echoes. A 9x9 block and radius 16 keep
/// the two-pass product well clear of the framebuffer's 8-bit quantisation;
/// the chain's own radius is covered one pass at a time above.
#[test]
fn bloom_half_block_halo_is_separable_oracle_product() {
    const SRC: u32 = 160;
    let gpu = headless::Gpu::new().expect("headless wgpu device");
    let preset = shader_dir().join("bloom_half.slangp");
    let input = mask_image(SRC, SRC, |x, y| {
        (76..=84).contains(&x) && (76..=84).contains(&y)
    });

    let radius = 16.0f32;
    let out_size = SRC / 2;
    let out = headless::render_single_pass_io(
        &gpu,
        &preset,
        &[("radius", radius)],
        SRC,
        SRC,
        out_size,
        out_size,
        &input,
    );

    let texel = 1.0 / out_size as f32;
    let centre = out_size / 2;
    let profile =
        |uv_axis: f32| oracle::gaussian_blur_1d(radius, texel, column_band(SRC, 76, 84, uv_axis));
    let at_centre = profile(uv_of(centre, centre, out_size, out_size)[0]);
    for d in 0..=20u32 {
        for (c, r) in [(centre + d, centre), (centre, centre + d)] {
            let uv = uv_of(c, r, out_size, out_size);
            let expected = profile(uv[0]) * profile(uv[1]);
            let px = out[headless::px_index(out_size, c, r)];
            assert!(
                (px[0] - expected).abs() < 0.01,
                "({c},{r}): gpu={} oracle={expected} (peak {at_centre})",
                px[0]
            );
        }
    }
}
