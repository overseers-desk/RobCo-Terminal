//! The librashader measurement rig, on the shared headless device.
//!
//! The device, the machine-wide lock, the offscreen readback and the frame
//! content all come from `gpu::harness` and are re-exported here so this
//! crate's callers name one module. What is this crate's own is the mount:
//! loading a preset and rendering a single pass through it.

use std::path::Path;

use librashader::presets::ShaderFeatures;
use librashader::runtime::wgpu::{FilterChain, FilterChainOptions, WgpuOutputView};
use librashader::runtime::{FilterChainParameters, Size, Viewport};

pub use gpu::harness::{
    frame_pixels, lock_path, px_index, Cell, GpuError, GpuLock, Locked as Gpu, OUTPUT_FORMAT,
};

/// Load a preset, set scalar parameters by name, render one frame from
/// `input_pixels` (RGBA8, `w*h*4` bytes; pass all-zero for procedural-only
/// shaders that ignore `Source`), and read the output back as RGBA f32.
///
/// The measurement rig for a single pass: `crt-render`'s per-pass tests mount
/// one shader this way and compare the readback against a CPU oracle. Input and
/// output share one resolution; use [`render_single_pass_io`] when a pass's
/// input texture (a glyph raster, say) is a different size than the framebuffer
/// it renders into.
///
/// It panics rather than returning a `Result` because every caller is a test
/// whose next line would be `.expect` anyway, and the panic message names the
/// preset that failed.
pub fn render_single_pass(
    gpu: &Gpu,
    preset: &Path,
    params: &[(&str, f32)],
    w: u32,
    h: u32,
    input_pixels: &[u8],
) -> Vec<[f32; 4]> {
    render_single_pass_io(gpu, preset, params, w, h, w, h, input_pixels)
}

/// Same as [`render_single_pass`], with the input texture's resolution
/// (`in_w`/`in_h`) independent of the output framebuffer's (`out_w`/`out_h`).
#[allow(clippy::too_many_arguments)]
pub fn render_single_pass_io(
    gpu: &Gpu,
    preset: &Path,
    params: &[(&str, f32)],
    in_w: u32,
    in_h: u32,
    out_w: u32,
    out_h: u32,
    input_pixels: &[u8],
) -> Vec<[f32; 4]> {
    let opts = FilterChainOptions {
        force_no_mipmaps: false,
        enable_cache: false,
        adapter_info: None,
    };
    let mut chain = FilterChain::load_from_path(
        preset,
        ShaderFeatures::NONE,
        &gpu.device,
        &gpu.queue,
        Some(&opts),
    )
    .unwrap_or_else(|e| panic!("loading {}: {e}", preset.display()));

    for (name, value) in params {
        chain.parameters().set_parameter_value(name, *value);
    }

    let input = gpu.make_input(in_w, in_h);
    gpu.upload(&input, in_w, in_h, input_pixels);
    let output = gpu.make_output(out_w, out_h);
    let view = output.create_view(&wgpu::TextureViewDescriptor::default());

    let mut cmd = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("pass"),
        });
    let size = Size::new(out_w, out_h);
    let viewport = Viewport {
        x: 0.0,
        y: 0.0,
        mvp: None,
        output: WgpuOutputView::new_from_raw(&view, size, OUTPUT_FORMAT),
        size,
    };
    chain
        .frame(&input, &viewport, &mut cmd, 0, None)
        .unwrap_or_else(|e| panic!("rendering {}: {e}", preset.display()));
    let idx = gpu.queue.submit([cmd.finish()]);
    gpu.device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(idx),
            timeout: None,
        })
        .expect("device poll failed");

    gpu.read_output(&output, out_w, out_h)
        .unwrap_or_else(|e| panic!("reading back {}: {e}", preset.display()))
}

