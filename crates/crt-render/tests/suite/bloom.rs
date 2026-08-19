//! Standalone proof + done-tests for `shaders/bloom/{bloom_h,bloom_v}.slang`.
//!
//! A deliberate choice, recorded in `bloom_h.slang`'s header: a
//! radius-parameterized blur feeding the same composite in
//! terminal_static.slang, built as a standard two-pass separable Gaussian
//! rather than a box-blur pyramid. Being this crate's own construction, it
//! is fully analytic and exactly specified, so the done-tests assert
//! against closed-form properties of a Gaussian blur.

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

/// Exact analytic match: `bloom_h` alone, radius chosen (14) so all 15 taps'
/// offsets (`i * radius/7` texels = `2*i` texels) land exactly on texel
/// centers, which removes bilinear interpolation from the comparison
/// entirely -- the oracle's `gaussian_1d_15tap` samples the same discrete
/// array `nearest`, and that is provably identical to what the GPU's bilinear
/// sampler does when every tap already lands on a texel center.
#[test]
fn bloom_h_matches_oracle_exactly_on_texel_aligned_taps() {
    let gpu = headless::Gpu::new().expect("headless wgpu device");
    let preset = shader_dir().join("bloom_h.slangp");

    // A bright column at x=32, constant down every row, so any row is a
    // valid 1-D cross-section.
    let mut input = vec![0u8; (W * H * 4) as usize];
    for y in 0..H {
        for x in 0..W {
            let v = if x == 32 { 255 } else { 0 };
            let i = ((y * W + x) * 4) as usize;
            input[i] = v;
            input[i + 1] = v;
            input[i + 2] = v;
            input[i + 3] = 255;
        }
    }

    let radius = 14.0f32;
    let out = headless::render_single_pass(&gpu, &preset, &[("radius", radius)], W, H, &input);

    let texel = 1.0 / W as f32;
    for &c in &[28u32, 32, 36] {
        let r = 10u32;
        let uv = uv_of(c, r, W, H);
        let expected = oracle::gaussian_1d_15tap(radius, texel, |offset| {
            // offset is in uv units; convert back to a texel index and read
            // the discrete (nearest, since taps are texel-aligned) source.
            let sample_uv_x = uv[0] + offset;
            let texel_idx = (sample_uv_x * W as f32 - 0.5).round() as i32;
            let clamped = texel_idx.clamp(0, W as i32 - 1) as u32;
            if clamped == 32 {
                1.0
            } else {
                0.0
            }
        });
        let px = out[headless::px_index(W, c, r)];
        assert!(
            (px[0] - expected).abs() < 0.01,
            "column {c}: gpu={} oracle={expected}",
            px[0]
        );
    }
}
