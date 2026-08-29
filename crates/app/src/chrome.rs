//! Everything drawn after the chain: the casting, the furniture that stands
//! on it, and in time the badges over the glass.
//!
//! One pipeline, one instance buffer, one draw, one render pass with
//! `LoadOp::Load` scissored to the bank column, compositing straight onto the
//! swapchain view. The picture the chain finished is already on that image and
//! this goes over it; nothing here begins a pass with `Clear`, so there is no
//! offscreen texture and no copy back.
//!
//! # What each instance is
//!
//! A rectangle in **physical** pixels of the swapchain, a `kind` saying which
//! shader body draws it, and an index into that kind's parameter buffer. The
//! vertex stage places the rectangle and hands the fragment stage a `uv`
//! running 0..1 across it, so a body is written against its own piece rather
//! than against the window.
//!
//! Per-piece parameters ride in a storage buffer indexed by that index, which
//! is what makes one pass enough: nothing is written per draw, so nothing can
//! be overwritten by a later write in the same submission. A chain's uniform
//! buffer could not do that, and the mount that used one had to compile a
//! chain per distinct parameter set to work around it.
//!
//! One pass is also what keeps the bank's own order. The plan alternates
//! shaded pieces and painted ones -- a row's moulding is struck over the
//! plate and under nothing, a strip's lamps sit inside the moulding -- so a
//! mount that drew all the shaded pieces and then all the painted ones would
//! put a row's chrome over its own lit window. Every piece is an instance
//! here, in the plan's own order, and the draw is painter's order.
//!
//! # The vector half
//!
//! A painted piece carries a [`chassis::paint::Painting`], a description
//! rather than an image, and is struck on the CPU into the raster atlas at
//! the destination's own size in physical pixels. The description is kept
//! beside the raster, so a bank standing still strikes nothing: the plan is
//! rebuilt every frame from the channel model and on all but the frames where
//! something actually changed the new description equals the old one.
//!
//! # Which size is which
//!
//! Two sizes go in and they are deliberately different. The **rectangle** is
//! the bank column, in physical pixels, because it is a scissor and a quad.
//! The casting's `viewport_size` is the **screen well**, in logical pixels,
//! because the casting continues the bezel's metal field leftwards and is
//! drawn in the bezel's coordinates. The bezel reaches that same logical ruler
//! by dividing its own `OutputSize` by `windowScaling * DPR`
//! (`frame_metal.slang`'s `main`, and `crt::params::Params::build`);
//! [`well_ruler`] is this side of it.
//!
//! The two used to travel as one number each way round: a view declared at one
//! size while the quad covered another, in units that were physical
//! everywhere else in the mount. Here the well is a named field of the
//! parameter record and the rectangle is an instance attribute, so neither can
//! be read as the other.

use bytemuck::{Pod, Zeroable};
use chassis::furniture::{Pass, Piece, Raster};
use chassis::params::{
    led_record, plate_record, tape_record, ChassisMetalParams, CHASSIS_RECORD_FLOATS,
    PIECE_RECORD_FLOATS,
};

/// Which body draws an instance. The fragment stage switches on it, and the
/// values are the ones the WGSL glue below declares.
const KIND_CHASSIS: u32 = 0;
const KIND_PLATE: u32 = 1;
const KIND_LED: u32 = 2;
const KIND_TAPE: u32 = 3;
const KIND_RASTER: u32 = 4;

/// One texel of transparent gutter between two rasters in the atlas, so a
/// piece sampling at exactly the far edge of its own rectangle reads nothing
/// rather than its neighbour's first lamp.
const GUTTER: u32 = 1;

/// A painted piece's whole parameter record: the `vec4` its picture's place
/// in the atlas takes.
const PICTURE_RECORD_FLOATS: usize = 4;

/// One rectangle to draw.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct Instance {
    /// `x`, `y`, `width`, `height` in physical pixels of the target.
    rect: [f32; 4],
    kind: u32,
    /// Index into this kind's parameter buffer.
    index: u32,
    _pad: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct Uniforms {
    /// The target's own size in physical pixels, for the NDC divide.
    viewport: [f32; 2],
    _pad: [f32; 2],
}

/// The glue around the shader bodies `chassis::shaders` compiles in: the
/// bindings, the vertex stage, and the switch on `kind`.
///
/// The bodies declare no bindings of their own, which is what lets the same
/// text serve this pass and the single-quad measurement rig.
const CHROME_WGSL: &str = r#"
@group(0) @binding(0) var<uniform> chrome: Uniforms;
@group(0) @binding(1) var<storage, read> castings: array<ChassisParams>;
@group(0) @binding(2) var<storage, read> plates: array<PlateParams>;
@group(0) @binding(3) var<storage, read> leds: array<LedParams>;
@group(0) @binding(4) var<storage, read> tapes: array<TapeParams>;
// A painted piece's whole record: where its picture sits in the atlas.
@group(0) @binding(5) var<storage, read> pictures: array<vec4<f32>>;
@group(0) @binding(6) var atlas: texture_2d<f32>;
@group(0) @binding(7) var atlas_sampler: sampler;

// The raster read the display bodies ask the host for. Every strip's raster
// lives in one atlas, so a body's own 0..1 coordinates are mapped through the
// rectangle its record carries.
//
// `textureSampleLevel` rather than `textureSample`: the bodies read their
// raster inside an early-out branch, and a plain sample may only be taken in
// uniform control flow.
fn chrome_sample(uv: vec2<f32>, rect: vec4<f32>) -> vec4<f32> {
    let p = rect.xy + clamp(uv, vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0)) * rect.zw;
    return textureSampleLevel(atlas, atlas_sampler, p, 0.0);
}

struct Uniforms {
    viewport: vec2<f32>,
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    // Flat: these reach the fragment stage as the integers the CPU wrote.
    @location(1) @interpolate(flat) kind: u32,
    @location(2) @interpolate(flat) index: u32,
};

@vertex
fn vs(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) rect: vec4<f32>,
    @location(1) kind: u32,
    @location(2) index: u32,
) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );
    let corner = corners[vertex_index];
    let px = rect.xy + corner * rect.zw;

    var out: VsOut;
    out.uv = corner;
    out.pos = vec4<f32>(
        px.x / chrome.viewport.x * 2.0 - 1.0,
        1.0 - px.y / chrome.viewport.y * 2.0,
        0.0,
        1.0,
    );
    out.kind = kind;
    out.index = index;
    return out;
}

@fragment
fn fs(input: VsOut) -> @location(0) vec4<f32> {
    if (input.kind == 0u) {
        return chassis_metal(input.uv, castings[input.index]);
    }
    if (input.kind == 1u) {
        return plate_metal(input.uv, plates[input.index]);
    }
    if (input.kind == 2u) {
        return led_matrix(input.uv, leds[input.index]);
    }
    if (input.kind == 3u) {
        return tape_label(input.uv, tapes[input.index]);
    }
    if (input.kind == 4u) {
        // The picture is already premultiplied, at one texel per pixel of the
        // rectangle it is drawn into.
        return chrome_sample(input.uv, pictures[input.index]);
    }
    return vec4<f32>(0.0, 0.0, 0.0, 0.0);
}
"#;

/// The mount: one pipeline, the instance buffer, and one parameter buffer per
/// kind, each grown to what a frame asked for.
pub struct Chrome {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    uniforms: wgpu::Buffer,
    /// One parameter buffer per kind, each grown to what a frame asked for:
    /// the casting's, then the plate's, the lamp grid's and the tape's.
    castings: wgpu::Buffer,
    plates: wgpu::Buffer,
    leds: wgpu::Buffer,
    tapes: wgpu::Buffer,
    pictures: wgpu::Buffer,
    /// How many records each of the five has room for.
    room: [usize; 5],
    instances: wgpu::Buffer,
    /// How many instances the instance buffer has room for.
    capacity: usize,
    /// Every display piece's raster, in one texture, so one draw covers the
    /// whole bank. Rebuilt when the shape of the plan changes.
    atlas: wgpu::Texture,
    atlas_size: (u32, u32),
    sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
    /// One struck picture per painted piece, held across frames.
    struck: Vec<Option<Struck>>,
}

/// A painted piece's picture and the description it was struck from.
struct Struck {
    size: (u32, u32),
    painting: chassis::paint::Painting,
    raster: Raster,
}

impl Chrome {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let source = format!(
            "{}{}{}{}{}{CHROME_WGSL}",
            chassis::shaders::COMMON_WGSL,
            chassis::shaders::CHASSIS_METAL_WGSL,
            chassis::shaders::PLATE_METAL_WGSL,
            chassis::shaders::LED_MATRIX_WGSL,
            chassis::shaders::TAPE_LABEL_WGSL,
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("chassis chrome"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("chassis chrome"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                storage_entry(1),
                storage_entry(2),
                storage_entry(3),
                storage_entry(4),
                storage_entry(5),
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("chassis chrome"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("chassis chrome"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Uint32,
                            offset: 16,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Uint32,
                            offset: 20,
                            shader_location: 2,
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
                    // Premultiplied source-over, so a piece with a soft edge
                    // lands on what is already there rather than replacing
                    // it. The casting is opaque and composites as a copy.
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

        let capacity = 32;
        let room = [1usize, 4, 24, 24, 32];
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("chassis chrome uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let castings = make_records(device, "castings", room[0] * CHASSIS_RECORD_FLOATS);
        let plates = make_records(device, "plates", room[1] * PIECE_RECORD_FLOATS);
        let leds = make_records(device, "lamp grids", room[2] * PIECE_RECORD_FLOATS);
        let tapes = make_records(device, "tapes", room[3] * PIECE_RECORD_FLOATS);
        let pictures = make_records(device, "pictures", room[4] * PICTURE_RECORD_FLOATS);
        let instances = make_instances(device, capacity);
        let atlas_size = (1, 1);
        let atlas = make_atlas(device, atlas_size);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("chassis chrome atlas"),
            // Nearest, and never anything else: a lamp grid reads exactly one
            // texel per cell and a filtered sample would light lamps between
            // two of them; the tape's dilation reads a nearest sample by hand
            // and the kit's oracle test is written against that.
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let bind_group = make_bind(
            device, &layout, &uniforms, &castings, &plates, &leds, &tapes, &pictures, &atlas,
            &sampler,
        );
        Self {
            pipeline,
            layout,
            uniforms,
            castings,
            plates,
            leds,
            tapes,
            pictures,
            room,
            instances,
            capacity,
            atlas,
            atlas_size,
            sampler,
            bind_group,
            struck: Vec::new(),
        }
    }

    /// Record the chrome over whatever is already on `output`.
    ///
    /// `target` is the swapchain image's size in physical pixels; `column` is
    /// the bank's rectangle at its left edge, also physical; `well` is the
    /// screen well's physical size, which [`well_ruler`] puts on the logical
    /// ruler the casting's field is measured in; `scale_factor` is the
    /// window's device pixel ratio, for that conversion.
    ///
    /// `pieces` is `chassis::Cabinet::furniture`, whose rectangles are
    /// logical and are scaled by the same ratio on the way in.
    ///
    /// A column of no width draws nothing, which is what a hidden chassis is:
    /// no bank rather than a bank of zero pixels.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        target: (u32, u32),
        column: (u32, u32),
        well: (u32, u32),
        scale_factor: f64,
        casting: &ChassisMetalParams,
        pieces: &[Piece],
    ) {
        if column.0 == 0 || column.1 == 0 || target.0 == 0 || target.1 == 0 {
            return;
        }

        // The pictures first: a painted piece is struck on the CPU into the
        // same atlas the display rasters go in, and only when its description
        // or its rectangle changed.
        self.strike(pieces, scale_factor, column);

        // Then the packing, because a piece's record carries where its raster
        // landed and the records are built after it.
        let plan = pack(pieces, &self.struck);
        let regrown = self.fit_atlas(device, plan.size);
        for (i, place) in &plan.places {
            if let Some(raster) = raster_of(pieces, &self.struck, *i) {
                upload(queue, &self.atlas, raster, *place);
            }
        }
        let atlas_size = self.atlas_size;
        let struck = &self.struck;
        let rect_of = |i: usize| -> [f32; 4] {
            let (w, h) = (atlas_size.0 as f32, atlas_size.1 as f32);
            match plan.places.iter().position(|(p, _)| *p == i) {
                Some(k) => {
                    let (x, y) = plan.places[k].1;
                    let r = raster_of(pieces, struck, i).expect("a placed raster");
                    [
                        x as f32 / w,
                        y as f32 / h,
                        r.width.max(1) as f32 / w,
                        r.height.max(1) as f32 / h,
                    ]
                }
                None => [0.0, 0.0, 1.0, 1.0],
            }
        };

        let ruler = well_ruler(well, scale_factor);
        let castings = vec![casting.record([ruler.0 as f32, ruler.1 as f32])];
        let mut plates: Vec<[f32; PIECE_RECORD_FLOATS]> = Vec::new();
        let mut leds: Vec<[f32; PIECE_RECORD_FLOATS]> = Vec::new();
        let mut tapes: Vec<[f32; PIECE_RECORD_FLOATS]> = Vec::new();
        let mut pictures: Vec<[f32; PICTURE_RECORD_FLOATS]> = Vec::new();

        // The casting is the bank's floor, so it goes first and everything
        // else keeps the plan's own order: one draw, painter's order, no sort.
        let mut instances = vec![Instance {
            rect: [0.0, 0.0, column.0 as f32, column.1 as f32],
            kind: KIND_CHASSIS,
            index: 0,
            _pad: [0, 0],
        }];
        for (i, piece) in pieces.iter().enumerate() {
            let Some(dest) = scale_rect(piece.rect, scale_factor, column) else {
                continue;
            };
            let (kind, index) = match piece.pass {
                Pass::Plate => {
                    plates.push(plate_record(&piece.params));
                    (KIND_PLATE, plates.len() - 1)
                }
                Pass::LedMatrix => {
                    leds.push(led_record(&piece.params, rect_of(i)));
                    (KIND_LED, leds.len() - 1)
                }
                Pass::TapeLabel => {
                    tapes.push(tape_record(&piece.params, rect_of(i)));
                    (KIND_TAPE, tapes.len() - 1)
                }
                Pass::Painted => {
                    if struck[i].is_none() {
                        continue;
                    }
                    pictures.push(rect_of(i));
                    (KIND_RASTER, pictures.len() - 1)
                }
            };
            instances.push(Instance {
                rect: [dest.0 as f32, dest.1 as f32, dest.2 as f32, dest.3 as f32],
                kind,
                index: index as u32,
                _pad: [0, 0],
            });
        }

        let mut rebind = self.fit(device, 0, castings.len(), CHASSIS_RECORD_FLOATS);
        rebind |= self.fit(device, 1, plates.len(), PIECE_RECORD_FLOATS);
        rebind |= self.fit(device, 2, leds.len(), PIECE_RECORD_FLOATS);
        rebind |= self.fit(device, 3, tapes.len(), PIECE_RECORD_FLOATS);
        rebind |= self.fit(device, 4, pictures.len(), PICTURE_RECORD_FLOATS);
        if instances.len() > self.capacity {
            self.capacity = instances.len().next_power_of_two();
            self.instances = make_instances(device, self.capacity);
        }
        if rebind || regrown {
            self.rebind(device);
        }

        write_records(queue, &self.castings, &castings);
        write_records(queue, &self.plates, &plates);
        write_records(queue, &self.leds, &leds);
        write_records(queue, &self.tapes, &tapes);
        write_records(queue, &self.pictures, &pictures);
        queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&instances));
        queue.write_buffer(
            &self.uniforms,
            0,
            bytemuck::bytes_of(&Uniforms {
                viewport: [target.0 as f32, target.1 as f32],
                _pad: [0.0, 0.0],
            }),
        );

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("chassis chrome"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    // Load, not Clear: the glass is already on this image.
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        // The bank clips its children, and this is that clip: a piece is
        // allowed to hang off the column and one routinely does, so the
        // scissor is the column's rectangle clamped to the target.
        let w = column.0.min(target.0);
        let h = column.1.min(target.1);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, Some(&self.bind_group), &[]);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        pass.set_scissor_rect(0, 0, w, h);
        pass.draw(0..6, 0..instances.len() as u32);
    }

    /// Grow one kind's parameter buffer to hold `wanted` records; `true` when
    /// it was replaced and the bind group has to be rebuilt.
    fn fit(&mut self, device: &wgpu::Device, kind: usize, wanted: usize, floats: usize) -> bool {
        if wanted <= self.room[kind] {
            return false;
        }
        self.room[kind] = wanted.next_power_of_two();
        let size = self.room[kind] * floats;
        let (buffer, label) = match kind {
            0 => (&mut self.castings, "castings"),
            1 => (&mut self.plates, "plates"),
            2 => (&mut self.leds, "lamp grids"),
            3 => (&mut self.tapes, "tapes"),
            _ => (&mut self.pictures, "pictures"),
        };
        *buffer = make_records(device, label, size);
        true
    }

    /// Strike every painted piece whose description or rectangle changed, and
    /// forget the pictures of pieces the plan no longer carries.
    fn strike(&mut self, pieces: &[Piece], scale_factor: f64, column: (u32, u32)) {
        self.struck.resize_with(pieces.len(), || None);
        self.struck.truncate(pieces.len());
        for (i, piece) in pieces.iter().enumerate() {
            let Some(painting) = piece.paint.as_ref() else {
                self.struck[i] = None;
                continue;
            };
            let Some(dest) = scale_rect(piece.rect, scale_factor, column) else {
                self.struck[i] = None;
                continue;
            };
            let size = (dest.2.max(1), dest.3.max(1));
            let kept = self.struck[i]
                .as_ref()
                .is_some_and(|s| s.size == size && &s.painting == painting);
            if kept {
                continue;
            }
            let picture = chassis::paint::rasterize(painting, size, scale_factor);
            self.struck[i] = Some(Struck {
                size,
                painting: painting.clone(),
                raster: Raster {
                    width: picture.width,
                    height: picture.height,
                    rgba: picture.rgba,
                },
            });
        }
    }

    /// Grow the atlas to hold `size` texels. Only ever grows: the bank's plan
    /// is the same list of rectangles frame after frame, so after the first
    /// one this allocates nothing.
    fn fit_atlas(&mut self, device: &wgpu::Device, size: (u32, u32)) -> bool {
        let wanted = (
            size.0.max(self.atlas_size.0).max(1),
            size.1.max(self.atlas_size.1).max(1),
        );
        if wanted == self.atlas_size {
            return false;
        }
        self.atlas_size = wanted;
        self.atlas = make_atlas(device, wanted);
        true
    }

    fn rebind(&mut self, device: &wgpu::Device) {
        self.bind_group = make_bind(
            device,
            &self.layout,
            &self.uniforms,
            &self.castings,
            &self.plates,
            &self.leds,
            &self.tapes,
            &self.pictures,
            &self.atlas,
            &self.sampler,
        );
    }
}

/// Where each display piece's raster goes in the atlas.
///
/// One column, each raster under the last with a gutter row between: the
/// rasters are a lamp grid or a glyph mask, tens of texels on a side, so a
/// shelf packer would buy nothing a bank of twenty strips can measure.
struct Packing {
    size: (u32, u32),
    /// `(piece index, top-left in texels)`, in the plan's own order.
    places: Vec<(usize, (u32, u32))>,
}

/// A piece's raster: the display's own, or the picture a painted piece was
/// struck into.
fn raster_of<'a>(
    pieces: &'a [Piece],
    struck: &'a [Option<Struck>],
    i: usize,
) -> Option<&'a Raster> {
    match pieces[i].source.as_ref() {
        Some(raster) => Some(raster),
        None => struck.get(i).and_then(|s| s.as_ref()).map(|s| &s.raster),
    }
}

fn pack(pieces: &[Piece], struck: &[Option<Struck>]) -> Packing {
    let mut places = Vec::new();
    let mut width = 1u32;
    let mut y = 0u32;
    for i in 0..pieces.len() {
        let Some(raster) = raster_of(pieces, struck, i) else {
            continue;
        };
        let (w, h) = (raster.width.max(1), raster.height.max(1));
        places.push((i, (0, y)));
        width = width.max(w + GUTTER);
        y += h + GUTTER;
    }
    Packing {
        size: (width, y.max(1)),
        places,
    }
}

fn write_records<const N: usize>(queue: &wgpu::Queue, buffer: &wgpu::Buffer, records: &[[f32; N]]) {
    if records.is_empty() {
        return;
    }
    queue.write_buffer(buffer, 0, bytemuck::cast_slice(records));
}

fn upload(queue: &wgpu::Queue, atlas: &wgpu::Texture, raster: &Raster, at: (u32, u32)) {
    let (w, h) = (raster.width.max(1), raster.height.max(1));
    if raster.rgba.len() < (w * h * 4) as usize {
        return;
    }
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: atlas,
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: at.0,
                y: at.1,
                z: 0,
            },
            aspect: wgpu::TextureAspect::All,
        },
        &raster.rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(w * 4),
            rows_per_image: Some(h),
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
}

fn make_atlas(device: &wgpu::Device, size: (u32, u32)) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("chassis chrome atlas"),
        size: wgpu::Extent3d {
            width: size.0.max(1),
            height: size.1.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn storage_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn make_records(device: &wgpu::Device, label: &str, floats: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(&format!("chassis chrome {label}")),
        size: (floats.max(1) * 4) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn make_instances(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("chassis chrome instances"),
        size: (capacity * std::mem::size_of::<Instance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

#[allow(clippy::too_many_arguments)]
fn make_bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniforms: &wgpu::Buffer,
    castings: &wgpu::Buffer,
    plates: &wgpu::Buffer,
    leds: &wgpu::Buffer,
    tapes: &wgpu::Buffer,
    pictures: &wgpu::Buffer,
    atlas: &wgpu::Texture,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    let view = atlas.create_view(&wgpu::TextureViewDescriptor::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("chassis chrome"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: castings.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: plates.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: leds.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: tapes.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: pictures.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

/// The screen well in the logical pixels the casting measures its field in,
/// from the physical size the swapchain is in.
///
/// The bezel this casting continues reaches the same ruler by dividing its
/// `OutputSize` by `windowScaling * device_pixel_ratio`
/// (`crt::params::Params::build`); the chrome has no window scaling to undo,
/// because it draws straight onto the swapchain and is never scaled by a
/// preset, so the whole conversion is the ratio. Floored at one pixel: a
/// zero-sized well divides through the shader's field mapping.
fn well_ruler(well: (u32, u32), scale_factor: f64) -> (u32, u32) {
    let ratio = if scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };
    (
        ((well.0 as f64 / ratio).round() as u32).max(1),
        ((well.1 as f64 / ratio).round() as u32).max(1),
    )
}

/// A furniture piece's rectangle, from `chassis`'s logical pixels to the
/// target's physical ones.
///
/// Returns `(x, y, width, height)` with `x`/`y` signed, because a piece is
/// allowed to hang off the column and one routinely does: an LED window's
/// spill margin reaches a strip's height and a half past its own left edge,
/// which is well outside the bank on the annunciator. The scissor cuts it at
/// the bank's bounds; what this returns is the *whole* rectangle, so the part
/// that is on the column lands in the right place.
///
/// `None` for a piece with no area, or one entirely off the column.
pub(crate) fn scale_rect(
    rect: chassis::Rect,
    scale_factor: f64,
    column: (u32, u32),
) -> Option<(i32, i32, u32, u32)> {
    let ratio = if scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };
    // The edges are rounded rather than the origin and size, so two rows a
    // whole pitch apart stay a whole pitch apart at any ratio.
    let x0 = (rect.x * ratio).round();
    let y0 = (rect.y * ratio).round();
    let x1 = ((rect.x + rect.width) * ratio).round();
    let y1 = ((rect.y + rect.height) * ratio).round();
    let (w, h) = ((x1 - x0) as i64, (y1 - y0) as i64);
    if w <= 0 || h <= 0 {
        return None;
    }
    if x1 <= 0.0 || y1 <= 0.0 || x0 >= f64::from(column.0) || y0 >= f64::from(column.1) {
        return None;
    }
    Some((x0 as i32, y0 as i32, w as u32, h as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::time::{Duration, Instant};

    use config::Config;
    use crt::{DegaussState, Geometry, Pacing, Params};

    /// The buffer layouts the WGSL declares, which the shader reads by
    /// offset: a field added without a matching change there has to fail here
    /// rather than silently shifting one number into another.
    #[test]
    fn the_blocks_are_the_size_the_shader_declares() {
        assert_eq!(std::mem::size_of::<Uniforms>(), 16);
        assert_eq!(std::mem::size_of::<Instance>(), 32);
        assert_eq!(CHASSIS_RECORD_FLOATS * 4, 64);
    }

    /// The frame pass's own ruler, computed the way the shader computes it:
    /// `OutputSize / windowScaling`, where `OutputSize` is the render target
    /// (physical pixels) scaled by the preset's `windowScaling` and the
    /// uniform is whatever `Params::build` pushed under that name.
    fn frame_ruler(well_physical: (u32, u32), window_scaling: f64, dpr: f32) -> (f64, f64) {
        let mut cfg = Config::default();
        cfg.general.window_scaling = window_scaling;
        let geom = Geometry {
            output_width: well_physical.0 as f32 / dpr,
            output_height: well_physical.1 as f32 / dpr,
            device_pixel_ratio: dpr,
            ..Geometry::default()
        };
        let mut pacing = Pacing::new(Instant::now());
        let time = pacing.tick_by(Duration::from_millis(16));
        let params = Params::build(&cfg, &geom, time, DegaussState::IDLE);
        let divisor = f64::from(params.get("windowScaling").expect("the frame's ruler"));
        // The framebuffer: `scale_type = original`, `scale = windowScaling`.
        let output = (
            well_physical.0 as f64 * window_scaling,
            well_physical.1 as f64 * window_scaling,
        );
        (output.0 / divisor, output.1 / divisor)
    }

    #[test]
    fn the_well_the_casting_is_given_is_the_ruler_the_bezel_divides_to() {
        // The bezel and the bank casting are one field across the seam, so the
        // size the chrome hands `chassis_metal` has to be the size
        // `frame_metal` arrives at from the other side: at every ratio, and
        // whatever the window scaling does to the frame pass's framebuffer.
        for &(well, scaling, dpr) in &[
            ((840u32, 768u32), 1.0f64, 1.0f32),
            ((840, 768), 1.0, 2.0),
            ((840, 768), 2.0, 2.0),
            ((1680, 1536), 0.5, 2.0),
        ] {
            let (fx, fy) = frame_ruler(well, scaling, dpr);
            let (cx, cy) = well_ruler(well, f64::from(dpr));
            assert!(
                (fx - f64::from(cx)).abs() < 1.0 && (fy - f64::from(cy)).abs() < 1.0,
                "chrome hands over {cx}x{cy}, bezel measures {fx}x{fy} \
                 (well {well:?}, windowScaling {scaling}, dpr {dpr})"
            );
        }
    }

    #[test]
    fn the_ruler_halves_the_well_on_a_two_times_display() {
        assert_eq!(well_ruler((840, 768), 1.0), (840, 768));
        assert_eq!(well_ruler((1680, 1536), 2.0), (840, 768));
        // ...and never hands the shader a zero to divide its field by.
        assert_eq!(well_ruler((1, 1), 4.0), (1, 1));
        assert_eq!(well_ruler((0, 0), 1.0), (1, 1));
        // A scale factor a window never reports is not a divide by zero.
        assert_eq!(well_ruler((840, 768), 0.0), (840, 768));
    }

    #[test]
    fn a_dpr_of_two_halves_the_frames_ruler_and_the_window_scaling_drops_out() {
        // The 1024x768 logical well of the default window, on a 2x display:
        // the shader must land on the logical size at both scalings.
        for scaling in [1.0, 2.0, 0.5] {
            let (x, y) = frame_ruler((2048, 1536), scaling, 2.0);
            assert!(
                (x - 1024.0).abs() < 1e-6 && (y - 768.0).abs() < 1e-6,
                "{x}x{y}"
            );
        }
        // At DPR 1 the same well is 1024x768 physical and lands in the same
        // place, which is what says the DPR is the only thing that changed.
        let (x, y) = frame_ruler((1024, 768), 1.0, 1.0);
        assert!((x - 1024.0).abs() < 1e-6 && (y - 768.0).abs() < 1e-6);
    }

    #[test]
    fn a_rectangle_scales_by_its_edges_and_a_piece_off_the_column_is_dropped() {
        let column = (200u32, 400u32);
        let r = chassis::Rect::new(10.0, 20.0, 30.0, 40.0);
        assert_eq!(scale_rect(r, 1.0, column), Some((10, 20, 30, 40)));
        assert_eq!(scale_rect(r, 2.0, column), Some((20, 40, 60, 80)));
        // A piece hanging off the left keeps its whole rectangle, negative
        // origin and all: the scissor is what cuts it.
        let spill = chassis::Rect::new(-15.0, 20.0, 30.0, 40.0);
        assert_eq!(scale_rect(spill, 1.0, column), Some((-15, 20, 30, 40)));
        // ...and one entirely off it is not drawn at all.
        assert_eq!(
            scale_rect(chassis::Rect::new(-40.0, 20.0, 30.0, 40.0), 1.0, column),
            None
        );
        assert_eq!(
            scale_rect(chassis::Rect::new(10.0, 20.0, 0.0, 40.0), 1.0, column),
            None
        );
    }
}
