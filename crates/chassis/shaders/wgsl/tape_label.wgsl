// The embossed punch tape the switchboard's strips carry instead of lamps,
// over a glyph mask raster.
//
// The WGSL twin of `tape_label.slang`, line for line and constant for
// constant. Two things the port moved rather than changed: the mask is read
// through `chrome_sample(uv, atlas)`, which the host defines, because the
// mount packs every label's raster into one atlas; and `hash12` and
// `rrect_px` come from `common.wgsl` instead of being restated here, which
// is what the slang pass did because it declared no include.
//
// The returned colour is premultiplied by the tape's body, with the dropped
// shadow carried in the alpha alone, so a label composites as source-over
// onto whatever it lies on.
//
// Requires `common.wgsl` ahead of it. This file declares no bindings and no
// entry points.

struct TapeParams {
    size_px: vec2<f32>,
    light_dir: vec2<f32>,
    // The glyph box inside the label, in the label's own pixels: origin then
    // extent.
    glyph_rect_px: vec4<f32>,
    // Where this label's mask sits in the host's atlas, as origin and extent
    // in the atlas's own 0..1 coordinates.
    atlas: vec4<f32>,
    tape_color: vec4<f32>,
    letter_color: vec4<f32>,
    bevel_px: f32,
    dilate_px: f32,
    sheen_amount: f32,
    grain_amount: f32,
    seed: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

fn tape_mask_at(px: vec2<f32>, p: TapeParams) -> f32 {
    let guv = (px - p.glyph_rect_px.xy) / p.glyph_rect_px.zw;
    if (guv.x < 0.0 || guv.x > 1.0 || guv.y < 0.0 || guv.y > 1.0) {
        return 0.0;
    }
    return chrome_sample(guv, p.atlas).a;
}

fn tape_punched(px: vec2<f32>, p: TapeParams) -> f32 {
    var m = tape_mask_at(px, p);
    let r = p.dilate_px;
    m = max(m, tape_mask_at(px + vec2<f32>(r, 0.0), p));
    m = max(m, tape_mask_at(px + vec2<f32>(-r, 0.0), p));
    m = max(m, tape_mask_at(px + vec2<f32>(0.0, r), p));
    m = max(m, tape_mask_at(px + vec2<f32>(0.0, -r), p));
    let rd = r * 0.7071;
    m = max(m, tape_mask_at(px + vec2<f32>(rd, rd), p));
    m = max(m, tape_mask_at(px + vec2<f32>(rd, -rd), p));
    m = max(m, tape_mask_at(px + vec2<f32>(-rd, rd), p));
    m = max(m, tape_mask_at(px + vec2<f32>(-rd, -rd), p));
    return m;
}

fn tape_label(uv: vec2<f32>, p: TapeParams) -> vec4<f32> {
    let size_px = p.size_px;
    let px = uv * size_px;
    let l = normalize(p.light_dir);

    let inset = 1.5;
    let rad = size_px.y * 0.24;
    let d_tape = rrect_px(px, vec2<f32>(inset, inset), size_px - vec2<f32>(inset, inset), rad);
    let body = 1.0 - smoothstep(-0.8, 0.8, d_tape);
    let d_shadow = rrect_px(px + l * 2.2, vec2<f32>(inset, inset), size_px - vec2<f32>(inset, inset), rad);
    let shadow = (1.0 - smoothstep(-1.0, 3.0, d_shadow)) * (1.0 - body);

    var col = p.tape_color.rgb;
    let grain = hash12(px + p.seed * 91.0) - 0.5;
    col = col * (1.0 + p.grain_amount * grain);
    let v = px.y / size_px.y;
    let band = exp(-pow((v - 0.28) / 0.16, 2.0));
    let face_mask = smoothstep(1.0, 4.0, -d_tape);
    col = col + vec3<f32>(1.0, 1.0, 1.0) * p.sheen_amount * 0.05 * band * face_mask;

    let edge = 1.0 - smoothstep(0.0, 2.0, -d_tape);
    col = col * (1.0 - 0.35 * edge);
    let bt = 1.0 - smoothstep(0.0, 1.8, px.y);
    let bb = 1.0 - smoothstep(0.0, 1.8, size_px.y - px.y);
    let rim_lit = bt * clamp(-l.y, 0.0, 1.0) + bb * clamp(l.y, 0.0, 1.0);
    col = col + vec3<f32>(0.30, 0.30, 0.34) * rim_lit * body;

    let m_c = tape_punched(px, p);
    let m_toward = tape_punched(px + l * p.bevel_px, p);
    let m_away = tape_punched(px - l * p.bevel_px, p);
    let m_away_far = tape_punched(px - l * p.bevel_px * 1.8, p);

    let rim_hi = clamp(m_c - m_toward, 0.0, 1.0);
    let rim_sh = clamp(m_c - m_away, 0.0, 1.0) * 0.7
               + clamp(m_c - m_away_far, 0.0, 1.0) * 0.3;
    let drop = clamp(m_toward - m_c, 0.0, 1.0);

    let face_grain = hash12(px * 1.7 + p.seed * 47.0) - 0.5;
    let face = p.letter_color.rgb * (0.74 + 0.10 * face_grain);
    col = mix(col, face, m_c);
    col = mix(col, p.tape_color.rgb * 0.55, clamp(rim_sh, 0.0, 1.0) * 0.8);
    col = col + (vec3<f32>(1.0, 1.0, 1.0) - 0.05 * p.tape_color.rgb) * rim_hi * 0.85;
    col = mix(col, p.tape_color.rgb * 0.25, drop * 0.55);

    col = col + vec3<f32>(1.0, 1.0, 1.0) * p.sheen_amount * 0.03 * band * m_c;

    let shadow_alpha = shadow * 0.40;
    let alpha = clamp(body + shadow_alpha, 0.0, 1.0);
    return vec4<f32>(col * body, alpha);
}
