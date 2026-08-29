//! Standalone proof + done-test for `shaders/terminal_frame/terminal_frame.slang`.
//!
//! Analytic: every term is closed-form except the `rand2` dither (a
//! `sin`-based hash whose low bits are not guaranteed to match between the
//! GPU's trig unit and Rust's `libm` sin -- GLSL leaves `sin` precision
//! implementation-defined). The oracle omits that term and the tolerance
//! below (0.035) absorbs its full amplitude (±0.02, `noise * 0.04` with
//! `noise` in `[-0.5, 0.5]`) plus float rounding.

use crt::harness::render_single_pass;
use gpu::harness::{px_index, Locked};
use std::path::PathBuf;

const W: u32 = 64;
const H: u32 = 64;

/// wgpu/librashader's quad + framebuffer convention: readback row `r`
/// corresponds to texcoord `v = (r + 0.5) / H` directly, no y-flip.
/// Empirically confirmed in `robco-chassis`'s `tests/suite/chassis_metal.rs`, where
/// the metals now live: the flipped hypothesis was off by ~0.05 at every
/// off-center sample against that oracle, the un-flipped one by <0.0001.
fn uv_of(c: u32, r: u32) -> [f32; 2] {
    [(c as f32 + 0.5) / W as f32, (r as f32 + 0.5) / H as f32]
}

#[test]
fn terminal_frame_matches_oracle() {
    let preset = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("shaders/terminal_frame/terminal_frame.slangp");
    let gpu = Locked::new().expect("headless wgpu device");

    let params = oracle::TerminalFrameParams {
        screen_curvature: 0.30,
        frame_color: [0.15, 0.15, 0.15, 1.0],
        frame_size: 0.05,
        screen_radius: 8.0,
        ambient_light: 0.30,
        frame_shininess: 0.20,
    };

    let gpu_params: &[(&str, f32)] = &[
        ("screenCurvature", params.screen_curvature),
        ("frameColorR", params.frame_color[0]),
        ("frameColorG", params.frame_color[1]),
        ("frameColorB", params.frame_color[2]),
        ("frameColorA", params.frame_color[3]),
        ("frameSize", params.frame_size),
        ("screenRadius", params.screen_radius),
        ("ambientLight", params.ambient_light),
        ("frameShininess", params.frame_shininess),
    ];

    // Input is irrelevant: the pass is fully procedural and never samples
    // `Source`. Feed black.
    let input = vec![0u8; (W * H * 4) as usize];
    let out = render_single_pass(&gpu, &preset, gpu_params, W, H, &input);

    // Center (dead in the middle of the screen, deep inside `inScreen`),
    // plus two points nearer the frame edge where `frameTint` (and its
    // noise dither) actually contributes.
    for &(c, r) in &[(32u32, 32u32), (4, 32), (32, 60)] {
        let uv = uv_of(c, r);
        let (color, alpha) = oracle::terminal_frame(uv, [W as f32, H as f32], &params);
        let px = out[px_index(W, c, r)];

        let tol = 0.035;
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
