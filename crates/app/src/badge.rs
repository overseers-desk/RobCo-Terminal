//! Drawing for the transient badges: black rounded plates over the glass, one
//! per thing the appliance has to say for a moment.
//!
//! [`crate::overlay`] is the state machine (when, and how strongly); this is
//! the mount that puts its answer on the frame. The overlay holds no
//! renderer, and the renderer asks the overlay for text and opacity.
//!
//! # The stack
//!
//! Slot 0 is the size overlay's own position, and everything below it stacks
//! beneath that on the same convention. A [`draw`] takes a
//! *list* of entries and puts each one a badge-and-a-line below the last,
//! because the two things this appliance has to say -- the grid's new size and
//! a write queue shedding ([`crate::overlay::Notice`]) -- are independent and
//! can be true at the same moment. Stacking them is what keeps the second from
//! being invisible whenever the first is up.
//!
//! One list and one pass, deliberately: `queue.write_buffer` lands before the
//! whole submission, so two `draw` calls sharing one uniform buffer inside one
//! encoder would both render with the *last* uniforms written -- the same
//! librashader lesson restated at a second mount -- and it is why each quad
//! here carries its own badge origin and its own alpha instead of reading
//! them from the uniform block.
//!
//! [`draw`]: Badge::draw
//!
//! # Where it goes in the frame
//!
//! After the chain, over the finished glass, with `LoadOp::Load` -- the same
//! place and for the same reason as [`crate::column`]: chassis chrome stays
//! out of the CRT chain, and a badge run through the curvature would bend
//! with the tube it is announcing the size of. The badge is centred over the
//! screen well and layered above the whole chain rather than inside it, so it stays
//! flat while the glass beneath it curves.
//!
//! Unlike the column this composites straight onto the swapchain view instead
//! of rendering offscreen and blitting, because it is not a librashader chain:
//! nothing here begins a pass with `LoadOp::Clear`, so there is no frame to
//! protect from.
//!
//! # The badge's shape
//!
//! A black rectangle at **twice** the text's size (twice the width, twice the
//! height), `radius: 5`, white text centred in it, and the whole item --
//! rectangle and text together -- carrying one opacity.
//!
//! The typeface is deliberate: this appliance has no platform UI font to draw
//! with, and would not want one if it did. The badge draws its text from the
//! terminal's own glyph atlas, so it is struck in the same phosphor face as
//! the screen it sits over. The layout rule (`radius: 5`, the doubling) governs
//! the plate; the ruler underneath the text is the atlas's cell.
//!
//! # Two rulers, and which is which
//!
//! Quad geometry is in **unscaled raster pixels** with the integer
//! magnification in a uniform, which is `term::render`'s rule and is why the
//! badge's glyphs land on the same pixel lattice as the grid's. The rounded
//! corner is the exception and is measured in **physical** pixels, because
//! `radius: 5` is a device-independent length: a logical 5 px has to stay a
//! logical 5 px when the font's integer scale changes, so it cannot ride the
//! same multiplier the glyphs do.

use bytemuck::{Pod, Zeroable};
use term::GlyphAtlas;

/// The badge's corner radius, in logical pixels.
const CORNER_RADIUS: f32 = 5.0;

/// One quad, in the same shape `term::render::Instance` uses, because the
/// badge's glyphs are the grid's glyphs and the lattice has to agree.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable, PartialEq)]
struct Quad {
    /// The badge's own top-left in **physical** pixels: which slot of the stack
    /// this quad belongs to. Per-quad rather than a uniform so one buffer, one
    /// pass and one submission can carry the whole stack (see the module doc).
    origin: [i32; 2],
    /// Top-left in unscaled raster pixels, relative to the badge's origin.
    dst: [i32; 2],
    /// Size in unscaled raster pixels.
    size: [i32; 2],
    /// Top-left of the glyph in the atlas, in texels. [`ROUNDED`] marks the
    /// badge's own rounded backing instead.
    src: [i32; 2],
    /// The quad's colour with its badge's opacity already in the alpha:
    /// fading one item means plate and glyphs carry the same number.
    color: [f32; 4],
}

/// One badge in the stack: what it says, and how strongly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Entry<'a> {
    pub text: &'a str,
    pub opacity: f32,
}

/// A negative atlas origin means "no texture read". `term::render` spends
/// `[-1, -1]` on a square fill; the badge's backing is the rounded one, so it
/// gets its own marker rather than a second meaning for the same value.
const ROUNDED: [i32; 2] = [-2, -2];

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct Uniforms {
    viewport: [f32; 2],
    scale: i32,
    /// The corner radius in physical pixels (see the module doc on rulers).
    radius: f32,
}

const SHADER: &str = r#"
struct Uniforms {
    viewport: vec2<f32>,
    scale: i32,
    radius: f32,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var atlas: texture_2d<f32>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    // Flat for `term::render`'s reason: these must reach the fragment shader
    // as the integers the CPU wrote, not as interpolated approximations.
    @location(0) @interpolate(flat) origin: vec2<i32>,
    @location(1) @interpolate(flat) dst: vec2<i32>,
    @location(2) @interpolate(flat) src: vec2<i32>,
    @location(3) @interpolate(flat) size: vec2<i32>,
    @location(4) @interpolate(flat) color: vec4<f32>,
};

@vertex
fn vs(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) origin: vec2<i32>,
    @location(1) dst: vec2<i32>,
    @location(2) size: vec2<i32>,
    @location(3) src: vec2<i32>,
    @location(4) color: vec4<f32>,
) -> VsOut {
    var corners = array<vec2<i32>, 6>(
        vec2<i32>(0, 0), vec2<i32>(1, 0), vec2<i32>(0, 1),
        vec2<i32>(1, 0), vec2<i32>(1, 1), vec2<i32>(0, 1),
    );
    let corner = corners[vertex_index];
    let px = origin + (dst + corner * size) * u.scale;
    let ndc = vec2<f32>(
        f32(px.x) / u.viewport.x * 2.0 - 1.0,
        1.0 - f32(px.y) / u.viewport.y * 2.0,
    );

    var out: VsOut;
    out.pos = vec4<f32>(ndc, 0.0, 1.0);
    out.origin = origin;
    out.dst = dst;
    out.src = src;
    out.size = size;
    out.color = color;
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let pixel = floor(in.pos.xy) + vec2<f32>(0.5, 0.5);

    if (in.src.x == -2) {
        // The badge's backing. The rounded-box distance is evaluated in
        // physical pixels, because the radius is a logical length and does
        // not scale with the font's magnification.
        let origin = vec2<f32>(
            f32(in.origin.x + in.dst.x * u.scale),
            f32(in.origin.y + in.dst.y * u.scale),
        );
        let extent = vec2<f32>(f32(in.size.x * u.scale), f32(in.size.y * u.scale));
        let half = extent * 0.5;
        // Never a radius larger than the box can hold: a badge narrower than
        // 2r would otherwise invert its own corners.
        let r = min(u.radius, min(half.x, half.y));
        let q = abs(pixel - origin - half) - (half - vec2<f32>(r, r));
        let dist = length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - r;
        // One pixel of coverage across the edge: the badge is chrome and is
        // not on the glyph lattice, so its edge is antialiased rather than
        // snapped.
        let coverage = clamp(0.5 - dist, 0.0, 1.0);
        let a = in.color.a * coverage;
        return vec4<f32>(in.color.rgb * a, a);
    }

    if (in.src.x < 0) {
        let a = in.color.a;
        return vec4<f32>(in.color.rgb * a, a);
    }

    // `term::shader`'s glyph read, unchanged: recover the integer pixel, undo
    // the origin and the magnification, and load that texel with no sampler
    // anywhere in the path.
    let cell = vec2<i32>(floor(in.pos.xy)) - in.origin;
    let texel = in.src + (cell / u.scale - in.dst);
    let coverage = textureLoad(atlas, texel, 0).r;
    let a = in.color.a * coverage;
    return vec4<f32>(in.color.rgb * a, a);
}
"#;

/// The badge's mount: one pipeline, one uniform buffer, one instance buffer
/// that grows with the longest text it has been asked to draw.
pub struct Badge {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    uniforms: wgpu::Buffer,
    instances: wgpu::Buffer,
    /// How many quads the instance buffer has room for.
    capacity: usize,
}

/// Where the badge went, in physical pixels: origin and size. Returned so a
/// test can assert on the rectangle the mount chose rather than re-deriving
/// it, and `None` when there is nothing to draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BadgeRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Badge {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("badge stack"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("badge stack"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        // Unfilterable, and there is no sampler: the glyph is
                        // read with `textureLoad`, as the grid's is.
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("badge stack"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("badge stack"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Quad>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Sint32x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Sint32x2,
                            offset: 8,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Sint32x2,
                            offset: 16,
                            shader_location: 2,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Sint32x2,
                            offset: 24,
                            shader_location: 3,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 32,
                            shader_location: 4,
                        },
                    ],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    // Premultiplied, so the badge's own opacity composites over
                    // the glass instead of replacing it.
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let capacity = 16;
        Self {
            pipeline,
            layout,
            uniforms: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("badge stack uniforms"),
                size: std::mem::size_of::<Uniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            instances: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("badge stack instances"),
                size: (capacity * std::mem::size_of::<Quad>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            capacity,
        }
    }

    /// Record the badge stack over whatever is already on `output`.
    ///
    /// `well` is the screen well in physical pixels -- the rectangle the chain
    /// drew into, which is the window less the bank column. The badge centres
    /// in exactly that well, not in the window, so a wide bank does not push
    /// it off-centre over the glass.
    ///
    /// `entries` is the stack, top first: entry 0 takes the badge's own
    /// centred position, and each later one sits a badge-and-a-line below it.
    /// The answer is index-aligned with it -- where each badge landed, or
    /// `None` for an entry that drew nothing (faded out, or empty text) -- so
    /// a caller can assert on the rectangle the mount chose rather than
    /// re-deriving it.
    ///
    /// `scale` is the atlas's integer magnification and `scale_factor` the
    /// window's device pixel ratio.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        target: (u32, u32),
        well: (i32, i32, u32, u32),
        atlas: &GlyphAtlas,
        scale: u32,
        scale_factor: f64,
        entries: &[Entry],
    ) -> Vec<Option<BadgeRect>> {
        let mut rects: Vec<Option<BadgeRect>> = vec![None; entries.len()];
        let scale = scale.max(1);
        let cell = atlas.cell;
        if target.0 == 0 || target.1 == 0 || cell.width == 0 || cell.height == 0 {
            return rects;
        }

        let (well_x, well_y, well_w, well_h) = well;
        // The step down the stack: one badge (two text heights) plus a text
        // height of air, so two plates read as two and not as one tall one.
        // Every badge is the same height -- the atlas cell decides it -- so the
        // step does not depend on which entry is being placed.
        let text_h = cell.height as i32;
        let step = 3 * text_h * scale as i32;

        let mut quads: Vec<Quad> = Vec::new();
        for (slot, entry) in entries.iter().enumerate() {
            if entry.opacity <= 0.0 || entry.text.is_empty() {
                continue;
            }
            // The badge is twice the text's bounding box, and the text sits
            // in the middle of it.
            let chars: Vec<char> = entry.text.chars().collect();
            let text_w = (chars.len() as u32 * cell.width) as i32;
            let badge_w = text_w * 2;
            let badge_h = text_h * 2;

            // Centred in the well, on whole physical pixels: half a pixel of
            // offset would put every glyph between two of them, which is the
            // same rule `draw_frame` centres the grid by.
            let origin_x = well_x + (well_w as i32 - badge_w * scale as i32) / 2;
            let origin_y =
                well_y + (well_h as i32 - badge_h * scale as i32) / 2 + slot as i32 * step;
            let origin = [origin_x, origin_y];
            let alpha = entry.opacity.clamp(0.0, 1.0);

            quads.push(Quad {
                origin,
                dst: [0, 0],
                size: [badge_w, badge_h],
                src: ROUNDED,
                // `color: "black"`, faded by this badge's own envelope.
                color: [0.0, 0.0, 0.0, alpha],
            });

            // The text's own top-left inside the badge: centred, so half the
            // doubling on each side.
            let text_x = text_w / 2;
            let text_y = text_h / 2;
            for (i, c) in chars.iter().enumerate() {
                let Some(glyph) = atlas.slot(*c) else {
                    continue;
                };
                if glyph.width == 0 || glyph.height == 0 {
                    continue;
                }
                quads.push(Quad {
                    origin,
                    dst: [
                        text_x + i as i32 * cell.width as i32 + glyph.left,
                        text_y + cell.baseline - glyph.top,
                    ],
                    size: [glyph.width as i32, glyph.height as i32],
                    src: [glyph.atlas_x as i32, glyph.atlas_y as i32],
                    // `color: "white"`.
                    color: [1.0, 1.0, 1.0, alpha],
                });
            }

            rects[slot] = Some(BadgeRect {
                x: origin_x,
                y: origin_y,
                width: (badge_w * scale as i32).max(0) as u32,
                height: (badge_h * scale as i32).max(0) as u32,
            });
        }

        if quads.is_empty() {
            return rects;
        }

        if quads.len() > self.capacity {
            self.capacity = quads.len().next_power_of_two();
            self.instances = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("badge stack instances"),
                size: (self.capacity * std::mem::size_of::<Quad>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&quads));

        queue.write_buffer(
            &self.uniforms,
            0,
            bytemuck::bytes_of(&Uniforms {
                viewport: [target.0 as f32, target.1 as f32],
                scale: scale as i32,
                radius: CORNER_RADIUS * scale_factor.max(f64::EPSILON) as f32,
            }),
        );

        // Rebuilt per draw rather than cached against the atlas it binds. A
        // font change swaps the atlas underneath this mount and wgpu 30 has no
        // cheap identity to compare one against (`global_id` is gone), so the
        // choice is a stale binding or a bind group per draw. The badges are
        // drawn only while one of them is actually up, so the second costs
        // nothing worth keeping a correctness risk for.
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("badge stack"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas.view),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("badge stack"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    // Load, not Clear: the glass and the casting are already
                    // on this image and the badge goes over them.
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, Some(&bind_group), &[]);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        pass.draw(0..6, 0..quads.len() as u32);
        drop(pass);

        rects
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The uniform block's layout is what the shader reads by offset, so a
    /// field added without a matching WGSL change has to fail here rather
    /// than silently shifting the radius into the scale.
    #[test]
    fn the_uniform_block_is_the_size_the_shader_declares() {
        assert_eq!(std::mem::size_of::<Uniforms>(), 16);
        assert_eq!(std::mem::size_of::<Quad>(), 48);
    }
}
