//! A minimal chain that mounts the accumulator on its own.
//!
//! This is the measurement rig and the worked example of the mount, not
//! something the application runs: the real graph (`crt-render`) mounts the same
//! pass inside its one preset with the CRT passes after it. Everything this type
//! does per frame, that graph has to do too, and `MOUNT.md` lists it.

use std::path::Path;
use std::time::Duration;

use librashader::presets::ShaderFeatures;
use librashader::runtime::wgpu::{FilterChain, FilterChainOptions, WgpuOutputView};
use librashader::runtime::{FilterChainParameters, Size, Viewport};

use crate::headless::{frame_pixels, Cell, Gpu, GpuError, OUTPUT_FORMAT};
use crate::BurnInPass;

type Result<T> = std::result::Result<T, GpuError>;

pub struct BurnInChain<'g> {
    gpu: &'g Gpu,
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
    pub fn load(gpu: &'g Gpu, preset: &Path, burn_in: f64, w: u32, h: u32) -> Result<Self> {
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
