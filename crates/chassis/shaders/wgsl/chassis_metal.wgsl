// The casting under the bank column: procedural metal, no input texture.
//
// The WGSL twin of `chassis_metal.slang`, line for line and constant for
// constant, with one thing said out loud that the slang pass had to smuggle:
// the field's viewport is a **named parameter**, not the size of whatever
// view the pass happens to render into.
//
// That matters because the casting is measured in the *bezel's* coordinates
// rather than its own, so the two read as one poured piece across the seam.
// `field_scale`/`field_offset` place the bank inside the screen well's field,
// and `viewport_size` is that well in **logical** pixels, the ruler
// `frame_metal` reaches by dividing its own OutputSize by
// `windowScaling * DPR`. The slang pass took it from OutputSize, which meant
// declaring a view at one size and drawing a rectangle of another, in units
// that were physical everywhere else in the mount.
//
// Requires `common.wgsl` ahead of it, and a host that supplies a
// `ChassisParams` value: this file declares no bindings and no entry points.

struct ChassisParams {
    field_scale: vec2<f32>,
    field_offset: vec2<f32>,
    light_dir: vec2<f32>,
    // The screen well in logical pixels: the field this casting continues.
    viewport_size: vec2<f32>,
    chassis_color: vec4<f32>,
    grain_amount: f32,
    mottle_amount: f32,
    scratch_amount: f32,
    vignette_strength: f32,
};

fn chassis_metal(uv: vec2<f32>, p: ChassisParams) -> vec4<f32> {
    let static_coords = uv * p.field_scale + p.field_offset;
    let px = static_coords * p.viewport_size;

    let l = normalize(p.light_dir);
    let light_pos = clamp(vec2<f32>(0.5, 0.5) + l * 0.45, vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0));
    let away = clamp(dot(static_coords - light_pos, -l), 0.0, 2.0);
    var vig = 1.0 - p.vignette_strength * smoothstep(0.0, 1.1, away);
    let corner_d = length((static_coords - 0.5) * 2.0) / 1.414;
    vig = vig * (1.0 - p.vignette_strength * 0.55 * smoothstep(0.62, 1.0, corner_d));

    let m = MetalParams(p.grain_amount, p.mottle_amount, p.scratch_amount);
    let metal = p.chassis_color.rgb * metal_field(px, m, 3.1) * vig;

    return vec4<f32>(metal, 1.0);
}
