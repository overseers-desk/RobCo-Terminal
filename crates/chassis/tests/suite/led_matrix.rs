//! Standalone proof + done-test for `shaders/led_matrix/led_matrix.slang`.
//!
//! Snapshot/threshold tier, not full analytic replication: this pass gathers
//! a Gaussian neighborhood of texture samples for its spill term
//! (`edgeBrightness`) and uses screen-space derivatives (`fwidth`) for the
//! dot's antialiasing, neither of which this crate reproduces bit-for-bit on
//! the CPU. For the grid, sample points are chosen deep inside a lit/dark
//! cell (far past the antialiasing band, outside the spill margin), where
//! the disk/halo shape has already saturated to 1 or 0 and the output is
//! fully determined by which color it mixes toward -- so this checks "the
//! grid reads lit cells as lit and dark cells as dark, not swapped or
//! garbage", not exact pixel values. For the spill band, the check is a
//! property: over a checkerboard raster the band's brightness is the same
//! at every point along an edge, which holds only while the spill kernel's
//! taps are no farther apart than the lamps they read.

use crt::harness::render_single_pass_io;
use gpu::harness::{px_index, Locked};
use std::path::PathBuf;

const GRID_W: u32 = 8;
const GRID_H: u32 = 4;
const OUT_W: u32 = 128;
const OUT_H: u32 = 64;

#[test]
fn led_matrix_lit_and_dark_cells_read_correctly() {
    let preset =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("shaders/led_matrix/led_matrix.slangp");
    let gpu = Locked::new().expect("headless wgpu device");

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

    let out = render_single_pass_io(
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
            let px = out[px_index(OUT_W, c, r)];
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

/// The spill band over a checkerboard raster reads the same at every point
/// along an edge. Every tap of the spill kernel lands on a lamp; with the
/// taps at most one lamp apart, each point of the band averages lit and
/// dark lamps alike, and the band is flat. Taps spaced wider than a lamp
/// would land on lamps of one parity and the band would switch between
/// bright and dark along the edge with the lamp pattern.
#[test]
fn led_matrix_spill_band_is_flat_along_an_edge_over_a_checkerboard() {
    const GRID: u32 = 8;
    const OUT: u32 = 128;
    let preset =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("shaders/led_matrix/led_matrix.slangp");
    let gpu = Locked::new().expect("headless wgpu device");

    let mut input = vec![0u8; (GRID * GRID * 4) as usize];
    for gy in 0..GRID {
        for gx in 0..GRID {
            let v = if (gx + gy) % 2 == 0 { 255 } else { 0 };
            let i = ((gy * GRID + gx) * 4) as usize;
            input[i] = v;
            input[i + 1] = v;
            input[i + 2] = v;
            input[i + 3] = 255;
        }
    }

    // A quarter of the output on every side is spill band; the grid sits in
    // the middle half.
    let margin = 0.25f32;
    let params: &[(&str, f32)] = &[
        ("gridSizeX", GRID as f32),
        ("gridSizeY", GRID as f32),
        ("litColorR", 1.0),
        ("litColorG", 0.55),
        ("litColorB", 0.10),
        ("litColorA", 1.0),
        ("dimColorR", 0.2),
        ("dimColorG", 0.1),
        ("dimColorB", 0.05),
        ("dimColorA", 1.0),
        ("panelColorR", 0.05),
        ("panelColorG", 0.05),
        ("panelColorB", 0.05),
        ("panelColorA", 1.0),
        ("dotRadius", 0.35),
        ("threshold", 0.5),
        ("glow", 0.5),
        ("spillMarginX", margin),
        ("spillMarginY", margin),
        ("spillStrength", 0.6),
        ("spillDeadX", 0.2),
        ("spillDeadY", 0.2),
    ];

    let out = render_single_pass_io(&gpu, &preset, params, GRID, GRID, OUT, OUT, &input);

    // Halfway out into the bottom band (v = 0.875), across the columns the
    // grid spans, a lamp's width in from either end of the band.
    let r = (0.875 * OUT as f32) as u32;
    let px_per_lamp = (OUT as f32 * (1.0 - 2.0 * margin) / GRID as f32) as u32;
    let c0 = (margin * OUT as f32) as u32 + px_per_lamp;
    let c1 = ((1.0 - margin) * OUT as f32) as u32 - px_per_lamp;
    let band: Vec<f32> = (c0..c1)
        .map(|c| out[px_index(OUT, c, r)][0])
        .collect();
    let mean = band.iter().sum::<f32>() / band.len() as f32;
    let (lo, hi) = band
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), &v| {
            (lo.min(v), hi.max(v))
        });
    assert!(mean > 0.05, "the band is lit at all: mean {mean}");
    assert!(
        hi - lo < 0.1 * mean,
        "band brightness along the edge ranges {lo}..{hi} around a mean of {mean}"
    );
}
