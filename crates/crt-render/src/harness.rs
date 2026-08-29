//! The librashader measurement rig: one pass, or the accumulator alone.
//!
//! Behind the `harness` feature, which only this crate's own dev-dependency on
//! itself and the test crates' dev-dependencies turn on. Nothing shipped mounts
//! a preset this way, and nothing shipped should link a rig that blocks on the
//! machine-wide device lock `gpu::harness` takes.
//!
//! Two mounts live here:
//!
//! - [`render_single_pass`], the per-pass measurement: load a preset, set its
//!   scalar parameters, render one frame, read the output back as RGBA f32 and
//!   compare it against a CPU oracle. The chassis's metals and displays, the
//!   bloom halves and the frame pass are all measured this way.
//! - [`BurnInChain`], a minimal chain that mounts the burn-in accumulator on
//!   its own. The worked example of [`crate::burn_in`]'s mount, not something
//!   the application runs: the shipping graph mounts the same pass inside its
//!   one preset with the CRT passes after it, and everything this type does per
//!   frame that graph has to do too.
//!
//! The device, the lock and the readback are `gpu::harness`'s. What is here is
//! the librashader half.

use std::path::Path;
use std::time::Duration;

use gpu::harness::{frame_pixels, Cell, GpuError, Locked, OUTPUT_FORMAT};
use librashader::presets::ShaderFeatures;
use librashader::runtime::wgpu::{FilterChain, FilterChainOptions, WgpuOutputView};
use librashader::runtime::{FilterChainParameters, Size, Viewport};

use crate::burn_in::BurnInPass;

type Result<T> = std::result::Result<T, GpuError>;

pub struct BurnInChain<'g> {
    gpu: &'g Locked,
    chain: FilterChain,
    pass: BurnInPass,
    input: wgpu::Texture,
    output: wgpu::Texture,
    view: wgpu::TextureView,
    w: u32,
    h: u32,
    frame: usize,
    /// Cold load time of the preset, kept so tests can report it alongside
    /// the recorded ramp-error numbers.
    pub load: Duration,
}

impl<'g> BurnInChain<'g> {
    pub fn load(gpu: &'g Locked, preset: &Path, burn_in: f64, w: u32, h: u32) -> Result<Self> {
        let opts = FilterChainOptions {
            force_no_mipmaps: false,
            // Cache off: librashader only checks the wgpu `PIPELINE_CACHE`
            // feature by trusting the caller, and a cold compile is the honest
            // number anyway.
            enable_cache: false,
            adapter_info: None,
        };
        let t = std::time::Instant::now();
        let chain = FilterChain::load_from_path(
            preset,
            ShaderFeatures::NONE,
            &gpu.device,
            &gpu.queue,
            Some(&opts),
        )
        .map_err(|e| GpuError::new(format!("loading {}: {e}", preset.display())))?;
        let load = t.elapsed();

        let input = gpu.make_input(w, h);
        let output = gpu.make_output(w, h);
        let view = output.create_view(&wgpu::TextureViewDescriptor::default());

        Ok(Self {
            gpu,
            chain,
            pass: BurnInPass::new(burn_in),
            input,
            output,
            view,
            w,
            h,
            frame: 0,
            load,
        })
    }

    pub fn pass(&mut self) -> &mut BurnInPass {
        &mut self.pass
    }

    /// The decay pushed on the last rendered frame.
    pub fn last_decay(&self) -> Option<f32> {
        self.pass.last_decay()
    }

    /// Render one frame whose lit cells are `lit`, at wall-clock time `now`, and
    /// return the whole output as RGBA f32.
    ///
    /// The order is the mount contract in miniature: push the decay for this
    /// frame's delta, then run the chain, then read.
    pub fn frame(&mut self, now: f64, lit: &[Cell]) -> Result<Vec<[f32; 4]>> {
        self.gpu.upload(
            &self.input,
            self.w,
            self.h,
            &frame_pixels(self.w, self.h, lit),
        );

        self.pass.push(self.chain.parameters(), now);

        let mut cmd = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("burn-in frame"),
            });

        let size = Size::new(self.w, self.h);
        let viewport = Viewport {
            x: 0.0,
            y: 0.0,
            mvp: None,
            output: WgpuOutputView::new_from_raw(&self.view, size, OUTPUT_FORMAT),
            size,
        };

        self.chain
            .frame(&self.input, &viewport, &mut cmd, self.frame, None)
            .map_err(|e| GpuError::new(format!("frame {}: {e}", self.frame)))?;

        let idx = self.gpu.queue.submit([cmd.finish()]);
        self.gpu
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(idx),
                timeout: None,
            })
            .map_err(|e| GpuError::new(format!("poll: {e}")))?;

        self.frame += 1;
        self.gpu.read_output(&self.output, self.w, self.h)
    }

    /// Read a named parameter back out of the chain, for tests that want to
    /// prove the write landed rather than infer it from pixels.
    pub fn parameter(&self, name: &str) -> Option<f32> {
        self.chain.parameters().parameter_value(name)
    }
}

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
    gpu: &Locked,
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
    gpu: &Locked,
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
