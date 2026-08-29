//! The test picture for the pass-graph tests, and one chain frame over it.
//!
//! Every claim in these tests is made about bytes read back from a texture, on
//! a device with no window and no surface, so the suite runs under Xvfb (and
//! would run with no display at all). The picture the chain is fed is drawn
//! into a real `gpu::Target`, because that texture, and its
//! `TEXTURE_BINDING` usage, is the seam this suite exercises.
//!
//! The device itself is not made here. It comes from `gpu::harness`, the
//! rebuild's one headless GPU harness; this module used to stand up a third
//! copy of that and now owns only what is particular to these tests, which is
//! the picture and the frame.

// Each integration test compiles this module into its own binary, so anything
// one test file does not call reads as dead code there. The allow is about that
// and nothing else.
#![allow(dead_code)]

use std::path::PathBuf;

use gpu::harness::Locked as Gpu;
use gpu::{Image, Target, TARGET_FORMAT};
use wgpu::util::DeviceExt as _;

/// A deterministic stand-in for the terminal grid: black with one lit bar
/// across the middle quarter. The bar has hard horizontal edges, which is what
/// makes a vertical squeeze measurable, and it is off-centre in neither axis,
/// so a squeeze about the centre moves both edges by the same amount.
///
/// The bar is a middling grey rather than white on purpose. A white bar is
/// already at the rail, so any test of a *brightening* uniform (the brightness
/// setting, the degauss flood) would clamp and read as no change at all; the
/// first draft of these tests did exactly that and passed the wrong way.
pub const BAR_LEVEL: f32 = 0.30;
fn picture_wgsl() -> String {
    PICTURE_WGSL.replace("BAR_LEVEL", &format!("{BAR_LEVEL:?}"))
}

const PICTURE_WGSL: &str = r#"
@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    // One oversized triangle covering the clip rectangle.
    var p = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    return vec4<f32>(p[i], 0.0, 1.0);
}

// `full` is 0 for the bar, 1 for a field covering the whole target, and 2 for
// a black frame -- the picture the terminal shows after the text goes away,
// which is the only thing a burn-in ghost is visible against.
struct Size { width: f32, height: f32, full: f32, pad: f32 };
@group(0) @binding(0) var<uniform> size: Size;

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let y = pos.y / size.height;
    if (size.full < 1.5 && (size.full > 0.5 || (y >= 0.375 && y < 0.625))) {
        return vec4<f32>(BAR_LEVEL, BAR_LEVEL, BAR_LEVEL, 1.0);
    }
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}
"#;

pub struct Harness {
    pub gpu: Gpu,
    pub input: Target,
    pub output: Target,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    field_bind_group: wgpu::BindGroup,
    dark_bind_group: wgpu::BindGroup,
    pub dir: PathBuf,
}

impl Harness {
    pub fn new(name: &str, width: u32, height: u32) -> Result<Self, String> {
        let gpu = Gpu::new().map_err(|e| e.to_string())?;
        let input = Target::new(&gpu.device, width, height, TARGET_FORMAT);
        let output = Target::new(&gpu.device, width, height, TARGET_FORMAT);

        let module = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("test picture"),
                source: wgpu::ShaderSource::Wgsl(picture_wgsl().into()),
            });

        let uniform = |label, full| {
            gpu.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents: uniform_bytes(width as f32, height as f32, full).as_slice(),
                    usage: wgpu::BufferUsages::UNIFORM,
                })
        };
        let bar_buffer = uniform("test picture size, bar", 0.0);
        let field_buffer = uniform("test picture size, field", 1.0);
        let dark_buffer = uniform("test picture size, dark", 2.0);

        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("test picture layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let make_bind_group = |label, buffer: &wgpu::Buffer| {
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            })
        };
        let bind_group = make_bind_group("test picture, bar", &bar_buffer);
        let field_bind_group = make_bind_group("test picture, field", &field_buffer);
        let dark_bind_group = make_bind_group("test picture, dark", &dark_buffer);

        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("test picture pipeline layout"),
                // wgpu 30: bind group layout entries are Option-wrapped.
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });

        let pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("test picture"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: TARGET_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                // wgpu 30: `multiview` became `multiview_mask`.
                multiview_mask: None,
                cache: None,
            });

        let dir = std::env::temp_dir().join(format!("robco-crt-render-{name}"));
        let _ = std::fs::remove_dir_all(&dir);

        Ok(Self {
            gpu,
            input,
            output,
            pipeline,
            bind_group,
            field_bind_group,
            dark_bind_group,
            dir,
        })
    }

    /// The bar picture: black with one lit band across the middle quarter.
    pub fn draw_picture(&self) {
        self.draw(&self.bind_group);
    }

    /// A lit field covering the whole target.
    ///
    /// This is what a squeeze can be measured on. A centred bar cannot show
    /// one: 3% of the way to an edge that sits an eighth of the image from
    /// centre is under half a pixel, so the bar's edges do not move at all. A
    /// full field puts the edges where the squeeze is largest, and the bands it
    /// opens at the top and bottom are the measurement.
    pub fn draw_field(&self) {
        self.draw(&self.field_bind_group);
    }

    /// A black frame: the terminal after the text it was showing has gone.
    /// What a burn-in ghost is measured against, since it is the only thing
    /// left in the picture.
    pub fn draw_dark(&self) {
        self.draw(&self.dark_bind_group);
    }

    fn draw(&self, bind_group: &wgpu::BindGroup) {
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("test picture"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("test picture"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.input.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                // wgpu 30 drift, one the waves' lists had not reached yet:
                // `RenderPassDescriptor` carries `multiview_mask` too, not just
                // `RenderPipelineDescriptor`.
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        let index = self.gpu.queue.submit([encoder.finish()]);
        self.gpu
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(index),
                timeout: None,
            })
            .expect("poll after drawing the test picture");
    }

    /// Run one chain frame and read the result back.
    pub fn render(&self, chain: &mut crt::Chain, time: crt::FrameTime) -> Image {
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("chain frame"),
            });
        chain
            .frame(
                &self.input.texture,
                &self.output.view,
                (self.output.width, self.output.height),
                TARGET_FORMAT,
                &mut encoder,
                time,
            )
            .expect("chain frame");
        let index = self.gpu.queue.submit([encoder.finish()]);
        self.gpu
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(index),
                timeout: None,
            })
            .expect("poll after a chain frame");
        self.output.read_rgba(&self.gpu.device, &self.gpu.queue)
    }
}

/// The picture shader's uniform block: width, height, the field flag, and one
/// float of padding to reach the 16-byte minimum uniform binding size.
fn uniform_bytes(width: f32, height: f32, full: f32) -> Vec<u8> {
    let mut v = Vec::with_capacity(16);
    for f in [width, height, full, 0.0f32] {
        v.extend_from_slice(&f.to_ne_bytes());
    }
    v
}

/// Mean luma over the whole image, on 0..1. The coarsest measure that still
/// separates "the chain drew the picture" from "the chain drew nothing".
pub fn mean_luma(img: &Image) -> f64 {
    let sum: u64 = img
        .pixels
        .chunks_exact(4)
        .map(|px| px[0].max(px[1]).max(px[2]) as u64)
        .sum();
    sum as f64 / (img.pixels.len() / 4) as f64 / 255.0
}

/// Rows whose centre pixel is lit, which is where a vertical squeeze shows.
pub fn lit_rows(img: &Image) -> Vec<u32> {
    (0..img.height)
        .filter(|y| {
            let px = img.pixel(img.width / 2, *y);
            px[0].max(px[1]).max(px[2]) > 8
        })
        .collect()
}
