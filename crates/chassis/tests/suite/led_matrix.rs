//! Standalone proof + done-test for `shaders/led_matrix/led_matrix.slang`.
//!
//! Snapshot/threshold tier, not full analytic replication: this pass gathers
//! a 5x5 neighborhood of bilinear texture samples for its spill term
//! (`edgeBrightness`) and uses screen-space derivatives (`fwidth`) for the
//! dot's antialiasing, neither of which this crate reproduces bit-for-bit on
//! the CPU. Sample points are instead chosen deep inside a lit/dark cell
//! (far past the antialiasing band, outside the spill margin), where the
//! disk/halo shape has already saturated to 1 or 0 and the output is fully
//! determined by which color it mixes toward -- so this checks "the grid
//! reads lit cells as lit and dark cells as dark, not swapped or garbage",
//! not exact pixel values.

use crt_burnin::headless;
use std::path::PathBuf;

const GRID_W: u32 = 8;
const GRID_H: u32 = 4;
const OUT_W: u32 = 128;
const OUT_H: u32 = 64;

#[test]
fn led_matrix_lit_and_dark_cells_read_correctly() {
    let preset =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("shaders/led_matrix/led_matrix.slangp");
    let gpu = headless::Gpu::new().expect("headless wgpu device");

    // Checkerboard glyph raster, one texel per grid cell.
    let mut input = vec![0u8; (GRID_W * GRID_H * 4) as usize];
    for gy in 0..GRID_H {
        for gx in 0..GRID_W {
            let lit = (gx + gy) % 2 == 0;
            let v = if lit { 255 } else { 0 };
            let i = ((gy * GRID_W + gx) * 4) as usize;
            input[i] = v;
            input[i + 1] = v;
            input[i + 2] = v;
            input[i + 3] = 255;
        }
    }

    let lit_color = [1.00, 0.55, 0.10];
    let dim_color = [0.20, 0.10, 0.05];
    let params: &[(&str, f32)] = &[
        ("gridSizeX", GRID_W as f32),
        ("gridSizeY", GRID_H as f32),
        ("litColorR", lit_color[0]),
        ("litColorG", lit_color[1]),
        ("litColorB", lit_color[2]),
        ("litColorA", 1.0),
        ("dimColorR", dim_color[0]),
        ("dimColorG", dim_color[1]),
        ("dimColorB", dim_color[2]),
        ("dimColorA", 1.0),
        ("panelColorR", 0.05),
        ("panelColorG", 0.05),
        ("panelColorB", 0.05),
        ("panelColorA", 1.0),
        ("dotRadius", 0.35),
        ("threshold", 0.5),
        ("glow", 0.5),
        // Zero margin removes the outer spill band's coordinate remap
        // entirely (`window` becomes exactly 1, `uv == vTexCoord`), so
        // `cell = uv * gridSize` lines up with this test's raw-texcoord cell
        // math without a second transform to account for.
        ("spillMarginX", 0.0),
        ("spillMarginY", 0.0),
        ("spillStrength", 0.6),
        ("spillDeadX", 0.2),
        ("spillDeadY", 0.2),
    ];

    let out = headless::render_single_pass_io(
        &gpu, &preset, params, GRID_W, GRID_H, OUT_W, OUT_H, &input,
    );

    let px_per_cell_x = OUT_W / GRID_W; // 16
    let px_per_cell_y = OUT_H / GRID_H; // 16

    // wgpu/librashader's quad + framebuffer convention: readback row maps to
    // texcoord v with no flip (empirically confirmed in
    // robco-chassis's tests/chassis_metal.rs), so the shader's
    // `cell = uv * gridSize` lines up with a plain row-major mapping from
    // output row to grid row.
    for gy in 0..GRID_H {
        for gx in 0..GRID_W {
            let lit = (gx + gy) % 2 == 0;
            let c = gx * px_per_cell_x + px_per_cell_x / 2;
            let r = gy * px_per_cell_y + px_per_cell_y / 2;
            let px = out[headless::px_index(OUT_W, c, r)];
            let expected = if lit { lit_color } else { dim_color };
            let tol = 0.06;
            assert!(
                (px[0] - expected[0]).abs() < tol
                    && (px[1] - expected[1]).abs() < tol
                    && (px[2] - expected[2]).abs() < tol,
                "cell ({gx},{gy}) lit={lit}: gpu={:?} expected~={expected:?}",
                &px[0..3]
            );
        }
    }
}
