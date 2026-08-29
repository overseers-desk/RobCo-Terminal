// Shared metal-surface math, the WGSL twin of `metal_common.slang`.
//
// The slang half stays: `frame_metal.slang` is mounted inside the CRT chain,
// because the bezel is the one piece of chassis that sits within the
// curvature, and an include of the chain's is a slang include. Everything
// drawn after the chain reads this file instead. Both arms are pinned to the
// same CPU reference in the `shader-oracle` crate, so a constant that drifts
// on one side fails a test rather than changing a picture.
//
// This file declares no bindings and no entry points. A host concatenates it
// with a shader body and its own glue, so the same body serves a per-piece
// instanced pass and a single-quad measurement rig.

struct MetalParams {
    grain_amount: f32,
    mottle_amount: f32,
    scratch_amount: f32,
};

fn hash12(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash12(i), hash12(i + vec2<f32>(1.0, 0.0)), u.x),
               mix(hash12(i + vec2<f32>(0.0, 1.0)), hash12(i + vec2<f32>(1.0, 1.0)), u.x), u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    var q = p;
    var a = 0.5;
    var s = 0.0;
    for (var i = 0; i < 4; i = i + 1) {
        s = s + a * vnoise(q);
        q = q * 2.13 + vec2<f32>(11.7, 5.3);
        a = a * 0.5;
    }
    return s;
}

// `speck_seed` is the one point the metals differ on: the plate hashes its
// specks from a different lattice offset so its dust does not repeat the
// casting's.
fn metal_field(px: vec2<f32>, m: MetalParams, speck_seed: f32) -> f32 {
    let f0 = fbm(px * 0.006);
    let f2 = fbm(px * 0.02 + 31.4);
    var tone = 1.0 + m.mottle_amount * (0.9 * (f0 - 0.5) + 0.5 * (f2 - 0.5));
    let stain = smoothstep(0.60, 0.85, fbm(px * 0.004 + 7.7));
    tone = tone - m.mottle_amount * 0.35 * stain;
    let g = (vnoise(px * 0.9) - 0.5) + 0.6 * (hash12(px) - 0.5);
    tone = tone + m.grain_amount * g;
    let streak = vnoise(vec2<f32>(px.x * 0.012, px.y * 0.4));
    tone = tone + m.scratch_amount * 0.3 * (streak - 0.5);
    let fine = vnoise(vec2<f32>(px.x * 0.5, px.y * 0.05) + 3.7);
    tone = tone + m.scratch_amount * 0.22 * (fine - 0.5);
    let pit = smoothstep(0.78, 0.95, vnoise(px * 0.09 + 13.1));
    tone = tone - m.mottle_amount * 0.3 * pit;
    let speck = step(0.9975, hash12(floor(px * 0.7) + speck_seed));
    tone = tone + m.scratch_amount * speck * 1.4;
    return max(tone, 0.0);
}

fn rrect_px(p: vec2<f32>, top_left: vec2<f32>, bottom_right: vec2<f32>, rad: f32) -> f32 {
    let c = (top_left + bottom_right) * 0.5;
    let half_size = (bottom_right - top_left) * 0.5 - vec2<f32>(rad, rad);
    let d = abs(p - c) - half_size;
    return length(max(d, vec2<f32>(0.0, 0.0))) + min(max(d.x, d.y), 0.0) - rad;
}
