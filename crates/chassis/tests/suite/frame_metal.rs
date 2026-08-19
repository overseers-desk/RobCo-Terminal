//! Standalone proof + done-test for `shaders/metal/frame_metal.slang`.
//! Fully analytic, no `sin` in this shader either.

use oracle;
use crt_burnin::headless;
use std::path::PathBuf;

const W: u32 = 96;
const H: u32 = 64;

/// See `tests/chassis_metal.rs`'s `uv_of` for the empirical confirmation
/// that this mapping (no y-flip between readback row and texcoord v) is
/// what wgpu/librashader's quad actually produces.
fn uv_of(c: u32, r: u32) -> [f32; 2] {
    [(c as f32 + 0.5) / W as f32, (r as f32 + 0.5) / H as f32]
}

#[test]
fn frame_metal_matches_oracle() {
    let preset = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("shaders/metal/frame_metal.slangp");
    let gpu = headless::Gpu::new().expect("headless wgpu device");

    let params = oracle::FrameMetalParams {
        screen_curvature: 0.30,
        frame_size: 0.05,
        screen_radius: 8.0,
        ambient_light: 0.30,
        frame_shininess: 0.10,
        light_dir: [0.35, -0.6],
        bezel_color: [0.32, 0.32, 0.34],
        chassis_color: [0.30, 0.30, 0.32],
        ridge_color: [0.55, 0.55, 0.58],
        bezel_margins: [8.0, 8.0, 8.0, 8.0],
        outer_radius: 6.0,
        well_depth: 18.0,
        well_floor: 0.55,
        ridge_gain: 0.6,
        metal: oracle::MetalParams {
            grain_amount: 0.06,
            mottle_amount: 0.35,
            scratch_amount: 0.08,
        },
        vignette_strength: 0.35,
        fill_gain: 0.35,
        trough_gain: 0.4,
        face_band_px: 0.0,
        rim_dist_px: 0.0,
        rim_gain: 0.0,
    };

    let gpu_params: &[(&str, f32)] = &[
        ("screenCurvature", params.screen_curvature),
        ("frameSize", params.frame_size),
        ("screenRadius", params.screen_radius),
        ("ambientLight", params.ambient_light),
        ("frameShininess", params.frame_shininess),
        ("lightDirX", params.light_dir[0]),
        ("lightDirY", params.light_dir[1]),
        ("bezelColorR", params.bezel_color[0]),
        ("bezelColorG", params.bezel_color[1]),
        ("bezelColorB", params.bezel_color[2]),
        ("bezelColorA", 1.0),
        ("chassisColorR", params.chassis_color[0]),
        ("chassisColorG", params.chassis_color[1]),
        ("chassisColorB", params.chassis_color[2]),
        ("chassisColorA", 1.0),
        ("ridgeColorR", params.ridge_color[0]),
        ("ridgeColorG", params.ridge_color[1]),
        ("ridgeColorB", params.ridge_color[2]),
        ("ridgeColorA", 1.0),
        ("bezelMarginsX", params.bezel_margins[0]),
        ("bezelMarginsY", params.bezel_margins[1]),
        ("bezelMarginsZ", params.bezel_margins[2]),
        ("bezelMarginsW", params.bezel_margins[3]),
        ("outerRadius", params.outer_radius),
        ("wellDepth", params.well_depth),
        ("wellFloor", params.well_floor),
        ("ridgeGain", params.ridge_gain),
        ("grainAmount", params.metal.grain_amount),
        ("mottleAmount", params.metal.mottle_amount),
        ("scratchAmount", params.metal.scratch_amount),
        ("vignetteStrength", params.vignette_strength),
        ("fillGain", params.fill_gain),
        ("troughGain", params.trough_gain),
        ("faceBandPx", params.face_band_px),
        ("rimDistPx", params.rim_dist_px),
        ("rimGain", params.rim_gain),
    ];

    let input = vec![0u8; (W * H * 4) as usize];
    let out = headless::render_single_pass(&gpu, &preset, gpu_params, W, H, &input);

    // Sample outside the bezel (chassis), inside the bezel plate, near the
    // screen center (glass), and near a screen edge (frame shadow/seam).
    for &(c, r) in &[(2u32, 2u32), (20, 32), (48, 32), (10, 10)] {
        let uv = uv_of(c, r);
        let (color, alpha) = oracle::frame_metal(uv, [W as f32, H as f32], &params);
        let px = out[headless::px_index(W, c, r)];

        let tol = 0.015;
        assert!(
            (px[0] - color[0]).abs() < tol
                && (px[1] - color[1]).abs() < tol
                && (px[2] - color[2]).abs() < tol,
            "color mismatch at ({c},{r}) uv={uv:?}: gpu={:?} oracle={color:?}",
            &px[0..3]
        );
        assert!(
            (px[3] - alpha).abs() < 0.01,
            "alpha mismatch at ({c},{r}) uv={uv:?}: gpu={} oracle={alpha}",
            px[3]
        );
    }
}
