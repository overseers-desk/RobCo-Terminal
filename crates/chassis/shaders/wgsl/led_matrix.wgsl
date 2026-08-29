// The lamp grid one channel strip is made of, over a glyph raster of one
// texel per lamp.
//
// The WGSL twin of `led_matrix.slang`, line for line and constant for
// constant, with two things the port had to move rather than change:
//
// - the raster is read through `chrome_sample(uv, atlas)`, which the host
//   defines, because the mount packs every strip's raster into one atlas and
//   a measurement rig binds a single texture. The body still asks in its own
//   0..1 coordinates.
// - `fwidth(cell_d)` is taken before the spill branch rather than after it. A
//   derivative may only be evaluated in uniform control flow, and the value
//   is the same either way: `cell_d` is a function of the fragment's own
//   coordinate and nothing the branch decides.
//
// A mount that switches on which body draws a piece still reaches these
// derivatives from inside its own branch, and that is sound where the switch
// is on a `@interpolate(flat)` value: a 2x2 derivative quad belongs to one
// primitive, so a per-instance value is the same in all four lanes.
//
// Requires nothing of `common.wgsl`, and declares no bindings or entry
// points of its own.

struct LedParams {
    grid_size: vec2<f32>,
    spill_margin: vec2<f32>,
    spill_dead: vec2<f32>,
    _pad0: vec2<f32>,
    // Where this strip's raster sits in the host's atlas, as origin and
    // extent in the atlas's own 0..1 coordinates.
    atlas: vec4<f32>,
    lit_color: vec4<f32>,
    dim_color: vec4<f32>,
    panel_color: vec4<f32>,
    dot_radius: f32,
    threshold: f32,
    glow: f32,
    spill_strength: f32,
};

// The spill glow's read of the lamps behind an edge: a Gaussian over the
// glyph raster, one texel per lamp, reaching `reach` lamps each way and
// widening with the distance out into the spill band. The taps sit at most
// one lamp apart -- the count grows with the reach, not the spacing --
// because taps spaced wider than the texels they read skip lamps, and the
// glow then follows which lamps the taps land on rather than how many are
// lit; along an edge that reads as the band flickering against the lamp
// pattern.
fn led_edge_brightness(edge: vec2<f32>, inward: vec2<f32>, d: f32, p: LedParams) -> f32 {
    let reach = 1.5 + 5.0 * d;
    let center = edge + inward * reach * 0.6 / p.grid_size;
    let n = i32(ceil(reach));
    let stride = reach / f32(n);
    var sum = 0.0;
    var weight = 0.0;
    for (var y = -n; y <= n; y = y + 1) {
        for (var x = -n; x <= n; x = x + 1) {
            let off = vec2<f32>(f32(x), f32(y)) * stride;
            let w = exp(-dot(off, off) / (reach * reach));
            let glyph = chrome_sample(
                clamp(center + off / p.grid_size, vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0)),
                p.atlas,
            ).rgb;
            sum = sum + w * max(glyph.r, max(glyph.g, glyph.b));
            weight = weight + w;
        }
    }
    return sum / max(weight, 1e-4);
}

fn led_matrix(tex_coord: vec2<f32>, p: LedParams) -> vec4<f32> {
    let window = max(vec2<f32>(1.0, 1.0) - 2.0 * p.spill_margin, vec2<f32>(1e-4, 1e-4));
    let uv = (tex_coord - p.spill_margin) / window;

    let tol = 0.05 * fwidth(uv);

    let cell = uv * p.grid_size;
    let idx = floor(cell);
    let cell_d = length(cell - idx - vec2<f32>(0.5, 0.5));
    let aa = max(0.5 * fwidth(cell_d), 0.002);

    if (any(uv < -tol) || any(uv > vec2<f32>(1.0, 1.0) - tol)) {
        let edge = clamp(uv, vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0));
        let away = (uv - edge) * window / max(p.spill_margin, vec2<f32>(1e-4, 1e-4));
        let d_phys = clamp(length(away), 0.0, 1.0);
        let live = max(abs(away) - p.spill_dead, vec2<f32>(0.0, 0.0))
                   / max(vec2<f32>(1.0, 1.0) - p.spill_dead, vec2<f32>(1e-4, 1e-4));
        let d = clamp(length(live), 0.0, 1.0);
        let falloff = 1.0 - d;
        let rise = smoothstep(0.0, 0.25, d);
        let inward = -normalize(away + vec2<f32>(1e-6, 1e-6));
        let a = p.spill_strength
                * clamp(2.2 * led_edge_brightness(edge, inward, d_phys, p), 0.0, 1.0)
                * rise * falloff * falloff * 0.6;
        return vec4<f32>(p.lit_color.rgb * a, a);
    }

    let glyph = chrome_sample((idx + vec2<f32>(0.5, 0.5)) / p.grid_size, p.atlas).rgb;
    let lit = step(p.threshold, max(glyph.r, max(glyph.g, glyph.b)));

    let disk = 1.0 - smoothstep(p.dot_radius - aa, p.dot_radius + aa, cell_d);
    let halo = p.glow * lit * (1.0 - smoothstep(p.dot_radius, p.dot_radius + 0.45, cell_d));

    var color = mix(p.panel_color.rgb, p.lit_color.rgb, halo);
    color = mix(color, mix(p.dim_color.rgb, p.lit_color.rgb, lit), disk);

    return vec4<f32>(color, 1.0);
}
