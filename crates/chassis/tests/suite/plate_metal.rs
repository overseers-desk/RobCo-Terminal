//! Standalone proof + done-test for `shaders/wgsl/plate_metal.wgsl`.
//! Fully analytic, same reasoning as `chassis_metal.rs` (no `sin` anywhere
//! in the surface math).

use chassis::params::{plate_record, record_bytes};
use gpu::harness::{px_index, render_wgsl_quad, Locked};
use oracle;

const W: u32 = 128;
const H: u32 = 32;

fn source() -> String {
    format!(
        "{}{}{}",
        chassis::shaders::COMMON_WGSL,
        chassis::shaders::PLATE_METAL_WGSL,
        "@group(0) @binding(0) var<uniform> p: PlateParams;\n\
         fn shade(uv: vec2<f32>) -> vec4<f32> { return plate_metal(uv, p); }\n",
    )
}

/// See `tests/suite/chassis_metal.rs`'s `uv_of` for the empirical confirmation
/// that readback row maps to texcoord v with no flip.
fn uv_of(c: u32, r: u32) -> [f32; 2] {
    [(c as f32 + 0.5) / W as f32, (r as f32 + 0.5) / H as f32]
}

#[test]
fn plate_metal_matches_oracle() {
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

    let input = vec![0u8; (W * H * 4) as usize];
    let record = record_bytes(&plate_record(&chassis::furniture::plate_params(&params)));
    let out = render_wgsl_quad(&gpu, &source(), &record, W, H, &input);

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
