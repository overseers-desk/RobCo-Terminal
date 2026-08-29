//! Everything drawn after the chain: the casting, and in time the furniture
//! and the badges that stand on it.
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
use chassis::params::{ChassisMetalParams, CHASSIS_RECORD_FLOATS};

/// Which body draws an instance. The fragment stage switches on it, and the
/// value is the one the WGSL glue below declares.
const KIND_CHASSIS: u32 = 0;

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
@group(0) @binding(1) var<storage, read> chassis: array<ChassisParams>;

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
        return chassis_metal(input.uv, chassis[input.index]);
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
    chassis: wgpu::Buffer,
    /// How many records the chassis buffer has room for.
    chassis_capacity: usize,
    instances: wgpu::Buffer,
    /// How many instances the instance buffer has room for.
    capacity: usize,
    bind_group: wgpu::BindGroup,
}

impl Chrome {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let source = format!(
            "{}{}{CHROME_WGSL}",
            chassis::shaders::COMMON_WGSL,
            chassis::shaders::CHASSIS_METAL_WGSL,
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
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
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

        let capacity = 8;
        let chassis_capacity = 1;
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("chassis chrome uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let chassis = make_records(device, chassis_capacity);
        let instances = make_instances(device, capacity);
        let bind_group = make_bind(device, &layout, &uniforms, &chassis);
        Self {
            pipeline,
            layout,
            uniforms,
            chassis,
            chassis_capacity,
            instances,
            capacity,
            bind_group,
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
    ) {
        if column.0 == 0 || column.1 == 0 || target.0 == 0 || target.1 == 0 {
            return;
        }

        let ruler = well_ruler(well, scale_factor);
        let records = vec![casting.record([ruler.0 as f32, ruler.1 as f32])];
        let instances = vec![Instance {
            rect: [0.0, 0.0, column.0 as f32, column.1 as f32],
            kind: KIND_CHASSIS,
            index: 0,
            _pad: [0, 0],
        }];

        let mut rebind = false;
        if records.len() > self.chassis_capacity {
            self.chassis_capacity = records.len().next_power_of_two();
            self.chassis = make_records(device, self.chassis_capacity);
            rebind = true;
        }
        if instances.len() > self.capacity {
            self.capacity = instances.len().next_power_of_two();
            self.instances = make_instances(device, self.capacity);
        }
        if rebind {
            self.bind_group = make_bind(device, &self.layout, &self.uniforms, &self.chassis);
        }

        queue.write_buffer(&self.chassis, 0, bytemuck::cast_slice(&records));
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
}

fn make_records(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("chassis chrome casting records"),
        size: (capacity * CHASSIS_RECORD_FLOATS * 4) as u64,
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

fn make_bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniforms: &wgpu::Buffer,
    chassis: &wgpu::Buffer,
) -> wgpu::BindGroup {
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
                resource: chassis.as_entire_binding(),
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
