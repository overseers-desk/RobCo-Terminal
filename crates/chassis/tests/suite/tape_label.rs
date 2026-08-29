//! Standalone proof + done-test for `shaders/wgsl/tape_label.wgsl`.
//!
//! Snapshot/threshold tier, not full analytic replication: `punched()`
//! dilates the glyph mask by sampling `Source` at eight offset points, which
//! this crate does not reproduce bit-for-bit on the CPU (bilinear filtering
//! at those offsets). Sample points are chosen deep inside a solid mask
//! block (letter interior, far past `dilatePx`/`bevelPx` from any mask edge)
//! and deep inside the plastic body (far from the mask, the strip's rounded
//! edge, and the top/bottom rim bands), where the composited formula
//! saturates to "this pixel is fully letter" or "fully plastic" and the
//! output color is dominated by `letterColor`/`tapeColor` respectively --
//! checking "letters read as letters, body reads as body", not exact values.

use chassis::params::{record_bytes, tape_record};
use gpu::harness::{px_index, render_wgsl_quad, Locked};

fn source() -> String {
    format!(
        "{}{}{}",
        chassis::shaders::COMMON_WGSL,
        chassis::shaders::TAPE_LABEL_WGSL,
        "@group(0) @binding(0) var<uniform> p: TapeParams;\n\
         fn shade(uv: vec2<f32>) -> vec4<f32> { return tape_label(uv, p); }\n",
    )
}

const SIZE_W: u32 = 160;
const SIZE_H: u32 = 32;

/// See `uv_of` in `robco-chassis`'s `tests/chassis_metal.rs` for the empirical
/// confirmation that readback row maps to texcoord v with no flip. (Unused here beyond
/// documentation -- `tape_label`'s glyph mask is uploaded with the matching
/// no-flip convention, so mask row `y` lines up with output row `y`.)
#[allow(dead_code)]
fn uv_of(c: u32, r: u32) -> [f32; 2] {
    [
        (c as f32 + 0.5) / SIZE_W as f32,
        (r as f32 + 0.5) / SIZE_H as f32,
    ]
}

#[test]
fn tape_label_letter_and_body_read_correctly() {
    let gpu = Locked::new().expect("headless wgpu device");

    // Glyph mask (alpha channel): a solid block from x in [60,100), y in
    // [8,24) -- comfortably larger than dilatePx (0.8px) and bevelPx (1.5px)
    // on every side, so its center is unambiguously "solid letter" and the
    // surrounding body, sampled far outside the block, is unambiguously
    // plastic. One texel of mask per output pixel (mask resolution ==
    // strip resolution), so glyphRectPx covers the whole strip 1:1.
    let mut mask = vec![0u8; (SIZE_W * SIZE_H * 4) as usize];
    for y in 0..SIZE_H {
        for x in 0..SIZE_W {
            let inside = (60..100).contains(&x) && (8..24).contains(&y);
            let i = ((y * SIZE_W + x) * 4) as usize;
            mask[i + 3] = if inside { 255 } else { 0 };
        }
    }

    let tape_color = [0.03, 0.03, 0.05];
    let letter_color = [0.92, 0.92, 0.90];
    let params: &[(&str, f32)] = &[
        ("sizePxX", SIZE_W as f32),
        ("sizePxY", SIZE_H as f32),
        ("lightDirX", 0.35),
        ("lightDirY", -0.6),
        ("tapeColorR", tape_color[0]),
        ("tapeColorG", tape_color[1]),
        ("tapeColorB", tape_color[2]),
        ("tapeColorA", 1.0),
        ("letterColorR", letter_color[0]),
        ("letterColorG", letter_color[1]),
        ("letterColorB", letter_color[2]),
        ("letterColorA", 1.0),
        ("glyphRectPxX", 0.0),
        ("glyphRectPxY", 0.0),
        ("glyphRectPxZ", SIZE_W as f32),
        ("glyphRectPxW", SIZE_H as f32),
        ("bevelPx", 1.5),
        ("dilatePx", 0.8),
        ("sheenAmount", 0.6),
        ("grainAmount", 0.05),
        ("seed", 0.0),
    ];

    let record = record_bytes(&tape_record(params, [0.0, 0.0, 1.0, 1.0]));
    let out = render_wgsl_quad(&gpu, &source(), &record, SIZE_W, SIZE_H, &mask);

    // Deep letter interior. The shader never outputs `letterColor` raw: deep
    // inside a solid mask (`mC == mToward == mAway == mAwayFar == 1`, true
    // here since the block is >>bevelPx*1.8 from its own edge), the letter
    // term reduces to `face = letterColor * (0.74 + 0.10 * faceGrain)` (a
    // deterministic `hash12`-based grain in `[-0.5, 0.5]`) plus a small
    // sheen add-on -- so the expected value is `letterColor * 0.74`, with
    // tolerance wide enough for the grain's `+/-0.05` swing and the sheen.
    let letter_uv = uv_of(80, 16);
    let letter_px = out[px_index(SIZE_W, 80, 16)];
    let _ = letter_uv;
    let letter_expected = [
        letter_color[0] * 0.74,
        letter_color[1] * 0.74,
        letter_color[2] * 0.74,
    ];
    let letter_tol = 0.10;
    assert!(
        (letter_px[0] - letter_expected[0]).abs() < letter_tol
            && (letter_px[1] - letter_expected[1]).abs() < letter_tol
            && (letter_px[2] - letter_expected[2]).abs() < letter_tol,
        "letter interior: gpu={:?} expected~={letter_expected:?}",
        &letter_px[0..3]
    );

    let tol = 0.12;

    // Deep plastic body, far from the mask block and from the strip's
    // rounded ends / top-bottom rim.
    let body_px = out[px_index(SIZE_W, 20, 16)];
    assert!(
        (body_px[0] - tape_color[0]).abs() < tol
            && (body_px[1] - tape_color[1]).abs() < tol
            && (body_px[2] - tape_color[2]).abs() < tol,
        "plastic body: gpu={:?} expected~={tape_color:?}",
        &body_px[0..3]
    );

    // Both should be fully opaque (body == 1 this deep inside the strip).
    assert!(
        letter_px[3] > 0.95,
        "letter alpha should saturate near 1.0, got {}",
        letter_px[3]
    );
    assert!(
        body_px[3] > 0.95,
        "body alpha should saturate near 1.0, got {}",
        body_px[3]
    );
}
