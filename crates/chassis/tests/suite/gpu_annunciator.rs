//! Optional GPU-backed done-test, kept because it strengthens the claim
//! cheaply: renders the annunciator shell's `chassis_metal` region
//! through the real shader body (`shaders/wgsl/chassis_metal.wgsl`) using
//! parameters this crate's drawing-recipe function produced, and checks the
//! readback against [`oracle`] -- the same cross-check
//! `tests/suite/chassis_metal.rs` runs, here anchored to this shell's actual fixed
//! parameters instead of arbitrary test values.

use oracle;
use chassis::shells::annunciator;
use chassis::shells::common::Rect;
use gpu::harness::{px_index, render_wgsl_quad, Locked};

const W: u32 = 64;
const H: u32 = 64;

/// See `tests/suite/chassis_metal.rs`'s `uv_of` for the confirmation that the
/// readback row maps to texcoord `v` with no flip, on the native quad and on
/// librashader's alike.
fn uv_of(c: u32, r: u32) -> [f32; 2] {
    [(c as f32 + 0.5) / W as f32, (r as f32 + 0.5) / H as f32]
}

fn source() -> String {
    format!(
        "{}{}{}",
        chassis::shaders::COMMON_WGSL,
        chassis::shaders::CHASSIS_METAL_WGSL,
        "@group(0) @binding(0) var<uniform> p: ChassisParams;\n\
         fn shade(uv: vec2<f32>) -> vec4<f32> { return chassis_metal(uv, p); }\n",
    )
}

#[test]
fn annunciator_chassis_metal_region_renders_as_the_oracle_predicts() {
    let gpu = match Locked::new() {
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

    let input = vec![0u8; (W * H * 4) as usize];
    let record: Vec<u8> = params
        .record([W as f32, H as f32])
        .iter()
        .flat_map(|f| f.to_ne_bytes())
        .collect();
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
    }
}
