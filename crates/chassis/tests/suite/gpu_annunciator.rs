//! Optional GPU-backed done-test, kept because it strengthens the claim
//! cheaply: renders the annunciator shell's `chassis_metal` region
//! through the real preset (`shaders/metal/chassis_metal.slangp`) using
//! parameters this crate's drawing-recipe function produced, and checks the
//! readback against [`oracle`] -- the same cross-check
//! `tests/suite/chassis_metal.rs` runs, here anchored to this shell's actual fixed
//! parameters instead of arbitrary test values.

use std::path::PathBuf;

use oracle;
use chassis::shells::annunciator;
use chassis::shells::common::Rect;
use crt_burnin::headless;

const W: u32 = 64;
const H: u32 = 64;

/// See `crt-render/tests/chassis_metal.rs`'s `uv_of` for the empirical
/// confirmation that wgpu/librashader's readback row maps to texcoord `v`
/// with no flip.
fn uv_of(c: u32, r: u32) -> [f32; 2] {
    [(c as f32 + 0.5) / W as f32, (r as f32 + 0.5) / H as f32]
}

#[test]
fn annunciator_chassis_metal_region_renders_as_the_oracle_predicts() {
    // `shaders/metal/` moved here with the metals; this is the same file
    // `tests/suite/chassis_metal.rs` mounts.
    let preset =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("shaders/metal/chassis_metal.slangp");
    assert!(preset.is_file(), "expected {preset:?} to exist");

    let gpu = match headless::Gpu::new() {
        Ok(gpu) => gpu,
        Err(e) => {
            eprintln!("skipping: no headless wgpu device ({e})");
            return;
        }
    };

    // A chassis region 349x1080 inside a 1200x1080 frame region, offset
    // 851px in -- a plausible real layout for this shell's bank sitting
    // right of the frame's screen well.
    let chassis = Rect::new(851.0, 0.0, 349.0, 1080.0);
    let frame_region = Rect::new(0.0, 0.0, 1200.0, 1080.0);
    let params = annunciator::chassis_metal_params(chassis, Some(frame_region));

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
    }
}
