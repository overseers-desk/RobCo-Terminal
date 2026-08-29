//! Standalone proof + done-test for `shaders/metal/plate_metal.slang`.
//! Fully analytic, same reasoning as `chassis_metal.rs` (no `sin` anywhere
//! in the surface math).

use oracle;
use crt::harness::render_single_pass;
use gpu::harness::{px_index, Locked};
use std::path::PathBuf;

const W: u32 = 128;
const H: u32 = 32;

/// See `tests/chassis_metal.rs`'s `uv_of` for the empirical confirmation
/// that readback row maps to texcoord v with no flip.
fn uv_of(c: u32, r: u32) -> [f32; 2] {
    [(c as f32 + 0.5) / W as f32, (r as f32 + 0.5) / H as f32]
}

#[test]
fn plate_metal_matches_oracle() {
    let preset = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("shaders/metal/plate_metal.slangp");
    let gpu = Locked::new().expect("headless wgpu device");

    let params = oracle::PlateMetalParams {
        size_px: [W as f32, H as f32],
        light_dir: [0.35, -0.6],
        base_color: [0.30, 0.30, 0.32],
        highlight_color: [0.85, 0.85, 0.88],
        shadow_color: [0.05, 0.05, 0.06],
        corner_radius: 4.0,
        bevel_px: 3.0,
        metal: oracle::MetalParams {
            grain_amount: 0.06,
            mottle_amount: 0.35,
            scratch_amount: 0.08,
        },
        vignette_strength: 0.35,
        wear_amount: 0.30,
        seam_gain: 0.50,
        seed: 7.0,
    };

    let gpu_params: &[(&str, f32)] = &[
        ("sizePxX", params.size_px[0]),
        ("sizePxY", params.size_px[1]),
        ("lightDirX", params.light_dir[0]),
        ("lightDirY", params.light_dir[1]),
        ("baseColorR", params.base_color[0]),
        ("baseColorG", params.base_color[1]),
        ("baseColorB", params.base_color[2]),
        ("baseColorA", 1.0),
        ("highlightColorR", params.highlight_color[0]),
        ("highlightColorG", params.highlight_color[1]),
        ("highlightColorB", params.highlight_color[2]),
        ("highlightColorA", 1.0),
        ("shadowColorR", params.shadow_color[0]),
        ("shadowColorG", params.shadow_color[1]),
        ("shadowColorB", params.shadow_color[2]),
        ("shadowColorA", 1.0),
        ("cornerRadius", params.corner_radius),
        ("bevelPx", params.bevel_px),
        ("grainAmount", params.metal.grain_amount),
        ("mottleAmount", params.metal.mottle_amount),
        ("scratchAmount", params.metal.scratch_amount),
        ("vignetteStrength", params.vignette_strength),
        ("wearAmount", params.wear_amount),
        ("seamGain", params.seam_gain),
        ("seed", params.seed),
    ];

    let input = vec![0u8; (W * H * 4) as usize];
    let out = render_single_pass(&gpu, &preset, gpu_params, W, H, &input);

    // Interior points only (away from the rounded-rect edge / coverage
    // falloff, which is intentionally a hard-edged antialiasing band the
    // oracle reproduces exactly too, but interior points keep this test
    // from also having to pin the AA band's sub-pixel behavior).
    for &(c, r) in &[(64u32, 16u32), (20, 16), (100, 16), (64, 8), (64, 24)] {
        let uv = uv_of(c, r);
        let (color, coverage) = oracle::plate_metal(uv, &params);
        let px = out[px_index(W, c, r)];

        let tol = 0.01;
        assert!(
            (px[0] - color[0] * coverage).abs() < tol
                && (px[1] - color[1] * coverage).abs() < tol
                && (px[2] - color[2] * coverage).abs() < tol,
            "mismatch at ({c},{r}) uv={uv:?}: gpu={:?} oracle={:?} coverage={coverage}",
            &px[0..3],
            color
        );
        assert!(
            (px[3] - coverage).abs() < tol,
            "alpha mismatch at ({c},{r}): gpu={} oracle={coverage}",
            px[3]
        );
    }
}
