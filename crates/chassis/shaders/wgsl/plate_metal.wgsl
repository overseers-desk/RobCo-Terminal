// The raised plate a shell screws over the casting: procedural cast metal,
// no input texture.
//
// The WGSL twin of `plate_metal.slang`, line for line and constant for
// constant. `size_px` is the plate's own pixel size in item-local pixels
// rather than the target's, because a plate is drawn at a size smaller than
// the surface it stands on.
//
// The returned colour is premultiplied by the plate's coverage, so the
// rounded corners and the edge band composite as source-over.
//
// Requires `common.wgsl` ahead of it. This file declares no bindings and no
// entry points.

struct PlateParams {
    size_px: vec2<f32>,
    light_dir: vec2<f32>,
    base_color: vec4<f32>,
    highlight_color: vec4<f32>,
    shadow_color: vec4<f32>,
    corner_radius: f32,
    bevel_px: f32,
    grain_amount: f32,
    mottle_amount: f32,
    scratch_amount: f32,
    vignette_strength: f32,
    wear_amount: f32,
    seam_gain: f32,
    seed: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

fn plate_metal(uv: vec2<f32>, p: PlateParams) -> vec4<f32> {
    let size_px = p.size_px;
    let px = uv * size_px;
    let d = rrect_px(px, vec2<f32>(0.5, 0.5), size_px - vec2<f32>(0.5, 0.5), p.corner_radius);
    let coverage = 1.0 - smoothstep(-1.0, 0.5, d);

    let uvn = uv;
    let l = normalize(p.light_dir);

    let m = MetalParams(p.grain_amount, p.mottle_amount, p.scratch_amount);
    let surf = metal_field(px + vec2<f32>(p.seed * 137.0, p.seed * 61.0), m, 9.2);
    var col = p.base_color.rgb * surf;

    let blotch = smoothstep(0.60, 0.92, fbm(px * 0.008 + p.seed * 31.0));
    col = mix(col, p.highlight_color.rgb * 0.42, p.wear_amount * 0.55 * blotch);

    let edge_dist = -d;
    let near_edge = smoothstep(p.bevel_px + 6.0, 0.0, edge_dist);
    let wear_noise = fbm(px * 0.03 + p.seed * 17.0);
    col = mix(col, p.highlight_color.rgb * (0.45 + 0.8 * wear_noise),
              p.wear_amount * near_edge * smoothstep(0.35, 0.8, wear_noise));

    let bt = 1.0 - smoothstep(0.0, p.bevel_px, px.y);
    let bl = 1.0 - smoothstep(0.0, p.bevel_px, px.x);
    let br = 1.0 - smoothstep(0.0, p.bevel_px, size_px.x - px.x);
    let bb = 1.0 - smoothstep(0.0, p.bevel_px, size_px.y - px.y);
    let lit = bt * clamp(-l.y, 0.0, 1.0) + bl * clamp(-l.x, 0.0, 1.0)
            + br * clamp(l.x, 0.0, 1.0) + bb * clamp(l.y, 0.0, 1.0);
    let shad = bt * clamp(l.y, 0.0, 1.0) + bl * clamp(l.x, 0.0, 1.0)
             + br * clamp(-l.x, 0.0, 1.0) + bb * clamp(-l.y, 0.0, 1.0);
    col = col + p.highlight_color.rgb * clamp(lit, 0.0, 1.0) * 0.9;
    col = mix(col, p.shadow_color.rgb, clamp(shad, 0.0, 1.0) * 0.7);

    let seam = 1.0 - smoothstep(0.8, 2.8, abs(edge_dist - (p.bevel_px + 2.0)));
    col = col * (1.0 - 0.35 * p.seam_gain * seam);

    let light_pos = clamp(vec2<f32>(0.5, 0.5) + l * 0.5, vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0));
    let away = clamp(dot(uvn - light_pos, -l), 0.0, 2.0);
    var vig = 1.0 - p.vignette_strength * smoothstep(0.0, 1.2, away);
    let corner_d = length((uvn - 0.5) * 2.0) / 1.414;
    vig = vig * (1.0 - p.vignette_strength * 0.5 * smoothstep(0.6, 1.0, corner_d));
    col = col * vig;

    return vec4<f32>(col, 1.0) * coverage;
}
