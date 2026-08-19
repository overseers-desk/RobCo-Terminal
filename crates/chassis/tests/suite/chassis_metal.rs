//! Standalone proof + done-test for `shaders/metal/chassis_metal.slang`.
//!
//! Fully analytic: `metalField`'s noise (`hash12`/`vnoise`/`fbm`) uses only
//! `fract`/`floor`/`dot`, no `sin`/`cos`, so it is bit-reproducible (modulo
//! ordinary float rounding) between the GPU and this crate's `oracle`
//! module -- unlike terminal_frame's dither, no term needs to be excluded.

use oracle;
use crt_burnin::headless;
use std::path::PathBuf;

const W: u32 = 64;
const H: u32 = 64;

/// wgpu/librashader's quad + framebuffer convention: readback row `r`
/// (row 0 first out of `copy_texture_to_buffer`) corresponds to texcoord
/// `v = (r + 0.5) / H` directly, no flip (empirically confirmed against this
/// oracle: the flipped hypothesis was off by ~0.05 at every off-center
/// sample point, the un-flipped one by <0.0001).
fn uv_of(c: u32, r: u32) -> [f32; 2] {
    [(c as f32 + 0.5) / W as f32, (r as f32 + 0.5) / H as f32]
}

#[test]
fn chassis_metal_matches_oracle() {
    let preset =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("shaders/metal/chassis_metal.slangp");
    let gpu = headless::Gpu::new().expect("headless wgpu device");

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

    let gpu_params: &[(&str, f32)] = &[
        ("fieldScaleX", params.field_scale[0]),
        ("fieldScaleY", params.field_scale[1]),
        ("fieldOffsetX", params.field_offset[0]),
        ("fieldOffsetY", params.field_offset[1]),
        ("lightDirX", params.light_dir[0]),
        ("lightDirY", params.light_dir[1]),
        ("chassisColorR", params.chassis_color[0]),
        ("chassisColorG", params.chassis_color[1]),
        ("chassisColorB", params.chassis_color[2]),
        ("chassisColorA", 1.0),
        ("grainAmount", params.metal.grain_amount),
        ("mottleAmount", params.metal.mottle_amount),
        ("scratchAmount", params.metal.scratch_amount),
        ("vignetteStrength", params.vignette_strength),
    ];

    let input = vec![0u8; (W * H * 4) as usize];
    let out = headless::render_single_pass(&gpu, &preset, gpu_params, W, H, &input);

    for &(c, r) in &[(0u32, 0u32), (32, 32), (63, 0), (10, 50), (50, 10)] {
        let uv = uv_of(c, r);
        let expected = oracle::chassis_metal(uv, [W as f32, H as f32], &params);
        let px = out[headless::px_index(W, c, r)];
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
