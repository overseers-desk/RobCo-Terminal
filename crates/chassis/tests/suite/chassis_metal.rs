//! Standalone proof + done-test for `shaders/wgsl/chassis_metal.wgsl`.
//!
//! Fully analytic: `metal_field`'s noise (`hash12`/`vnoise`/`fbm`) uses only
//! `fract`/`floor`/`dot`, no `sin`/`cos`, so it is bit-reproducible (modulo
//! ordinary float rounding) between the GPU and this crate's `oracle`
//! module -- unlike terminal_frame's dither, no term needs to be excluded.

use gpu::harness::{px_index, render_wgsl_quad, Locked};
use oracle;

const W: u32 = 64;
const H: u32 = 64;

/// The native quad's convention, which is the one librashader's quad landed
/// on too: readback row `r` (row 0 first out of `copy_texture_to_buffer`)
/// corresponds to texcoord `v = (r + 0.5) / H` directly, no flip. It was
/// confirmed empirically against this oracle for the slang mount (the flipped
/// hypothesis was off by ~0.05 at every off-center sample point, the
/// un-flipped one by <0.0001) and is confirmed again here for the WGSL one,
/// which is what says the picture did not turn over when the pass left the
/// chain.
fn uv_of(c: u32, r: u32) -> [f32; 2] {
    [(c as f32 + 0.5) / W as f32, (r as f32 + 0.5) / H as f32]
}

/// The shader body with the one-piece glue a measurement rig needs: the
/// parameter block at binding 0, and `shade` over it.
///
/// The mount in `app::chrome` supplies its own, reading the same block out of
/// a per-piece storage buffer, which is what the two arms share.
fn source() -> String {
    format!(
        "{}{}{}",
        chassis::shaders::COMMON_WGSL,
        chassis::shaders::CHASSIS_METAL_WGSL,
        "@group(0) @binding(0) var<uniform> p: ChassisParams;\n\
         fn shade(uv: vec2<f32>) -> vec4<f32> { return chassis_metal(uv, p); }\n",
    )
}

fn record_bytes(record: &[f32; 16]) -> Vec<u8> {
    record.iter().flat_map(|f| f.to_ne_bytes()).collect()
}

#[test]
fn chassis_metal_matches_oracle() {
    let gpu = Locked::new().expect("headless wgpu device");

    let params = oracle::ChassisMetalParams {
        field_scale: [1.0, 1.0],
        field_offset: [0.0, 0.0],
        light_dir: [0.35, -0.6],
        chassis_color: [0.30, 0.30, 0.32],
        metal: oracle::MetalParams {
            grain_amount: 0.06,
            mottle_amount: 0.35,
            scratch_amount: 0.08,
        },
        vignette_strength: 0.35,
    };

    let input = vec![0u8; (W * H * 4) as usize];
    let record = record_bytes(&params.record([W as f32, H as f32]));
    let out = render_wgsl_quad(&gpu, &source(), &record, W, H, &input);

    for &(c, r) in &[(0u32, 0u32), (32, 32), (63, 0), (10, 50), (50, 10)] {
        let uv = uv_of(c, r);
        let expected = oracle::chassis_metal(uv, [W as f32, H as f32], &params);
        let px = out[px_index(W, c, r)];
        let tol = 0.01;
        assert!(
            (px[0] - expected[0]).abs() < tol
                && (px[1] - expected[1]).abs() < tol
                && (px[2] - expected[2]).abs() < tol,
            "mismatch at ({c},{r}) uv={uv:?}: gpu={:?} oracle={expected:?}",
            &px[0..3]
        );
        assert!(
            (px[3] - 1.0).abs() < 1e-6,
            "alpha should be 1.0, got {}",
            px[3]
        );
    }
}
