//! The wgpu 30 surface behind the window.
//!
//! This module owns getting a configured, correctly-sized surface onto the
//! screen and keeping it that way across resizes and DPR changes. What is
//! drawn on it is [`crate::window::TerminalSurface`]'s, which is why the
//! frame is three calls here rather than one: [`Gpu::acquire`] gets a
//! swapchain image and an encoder, the caller records whatever it likes
//! into them, and [`Gpu::present`] submits and hands the image back. It is
//! split that way because the recording half is now the glyph grid and
//! the filter chain living one module up, and neither of them belongs
//! behind a `render()` that owns the surface too.
//!
//! wgpu is pinned to exactly 30.0.0 because librashader 0.12 pins it
//! there. This module is written against that version's API, which drifted
//! from 29 in several places.
//!
//! This module also carries [`frame_stats`]'s timing instrument: [`Frame`]
//! carries this frame's query set (see that module's doc for why `Frame` rather than
//! `Gpu` is the natural owner), and [`Gpu`] carries the rolling aggregate
//! that survives across frames.

use std::sync::Arc;

use winit::window::Window;

use crate::frame_stats;

pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    pub adapter_name: String,
    pub backend: String,
    /// The live-preview throughput instrument. Present on every `Gpu`,
    /// enabled or not (see `frame_stats::Instrument`'s doc): a run that never
    /// passes `--frame-stats` pays nothing for it on the frame path.
    stats: frame_stats::Instrument,
    /// A reconfigure the swapchain is owed but cannot be given yet: the
    /// surface handed out an image that is drawable but stale, and wgpu
    /// forbids configuring a surface while an image of it is outstanding.
    /// [`Gpu::acquire`] pays it at the top of the next frame, when nothing
    /// is held.
    stale_swapchain: bool,
}

impl Gpu {
    /// `frame_stats_enabled` is `--frame-stats`; see [`frame_stats`].
    pub fn new(window: Arc<Window>, frame_stats_enabled: bool) -> Result<Self, String> {
        pollster::block_on(Self::new_async(window, frame_stats_enabled))
    }

    async fn new_async(window: Arc<Window>, frame_stats_enabled: bool) -> Result<Self, String> {
        // With a window there *is* a display handle, and on Wayland the
        // instance needs it before any surface can be created. `_from_env`
        // keeps WGPU_BACKEND working, which is how a headless run picks
        // Vulkan/lavapipe.
        let instance = wgpu::Instance::new(
            wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(window.clone())),
        );

        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| format!("no wgpu surface: {e}"))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                // Passing the surface makes the adapter choice one that
                // can actually present to it, rather than one we discover
                // is incompatible at configure time.
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(|e| format!("no wgpu adapter: {e}"))?;

        let info = adapter.get_info();
        let (device, queue) = adapter
            // The CRT chain runs on this device, and both of the features it
            // wants have to be asked for here or they are gone for the life of
            // the device. `gpu::required_features` is the authority on
            // which those are and why; it filters them to what the adapter
            // actually offers, so this is not a device that fails to be created
            // on a machine that lacks them.
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("robco device"),
                required_features: gpu::required_features(&adapter)
                    | frame_stats::required_features(&adapter),
                ..Default::default()
            })
            .await
            .map_err(|e| format!("no wgpu device: {e}"))?;

        let stats = frame_stats::Instrument::new(&device, &queue, frame_stats_enabled);

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        // The sRGB question, settled by measurement rather than left open:
        // a *non*-sRGB swapchain, so a
        // value the chain writes is the byte the screen gets.
        //
        // The last pass of the chain writes a display value, already in the
        // space the screen expects, straight to the surface: writing that
        // same value through an sRGB *view* would encode it a second time,
        // and the cost is not subtle. With the shipped Default Amber
        // profile, the unlit glass reads (73, 61, 46) out of an sRGB
        // swapchain against (17, 12, 7) out of this one, measured on the
        // same frame of the same run. The whole picture is washed out, and
        // the still-floor reference (a masked RMSE of 0.00107) is a
        // nearly black glass, which the first of those cannot be.
        //
        // This also puts the presented path in the same space as every pixel
        // number already recorded here, which were all measured on
        // `gpu::TARGET_FORMAT` (`Rgba8Unorm`, no transfer function), so
        // existing RMSE comparisons inherit rather than re-establish them. The
        // alternative -- keeping the sRGB swapchain and moving the whole
        // chain to linear light -- is a different design than the one built
        // here, and would have to start at the shaders.
        //
        // `Chain::frame` is told the format either way (see `Gpu::format`),
        // because librashader builds the last pass's pipeline against it.
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);
        log::info!("swapchain format {format:?}");

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            // `Auto` is the only value guaranteed for every format in
            // `caps.formats`; anything else has to be checked against
            // that format's `color_spaces` set first. A future revision may
            // want a wider space for the CRT chain, and that is where the check
            // belongs.
            color_space: wgpu::SurfaceColorSpace::Auto,
            // Never zero: a minimised window reports 0x0 and configuring
            // a zero-sized surface is a validation error.
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: caps.present_modes[0],
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        Ok(Self {
            device,
            queue,
            surface,
            config,
            adapter_name: info.name,
            backend: format!("{:?}", info.backend),
            stats,
            stale_swapchain: false,
        })
    }

    /// Whether `--frame-stats` is on for this run.
    pub fn frame_stats_enabled(&self) -> bool {
        self.stats.enabled()
    }

    /// Whether this adapter can actually be GPU-timed: `--frame-stats` is on
    /// *and* the device carries both timestamp features. `false` is a
    /// documented, non-panicking fallback (see `frame_stats`'s module doc),
    /// not an error.
    pub fn frame_stats_available(&self) -> bool {
        self.stats.gpu_available()
    }

    /// The rolling window's p50/p99s: `--frame-stats`'s periodic log line
    /// reads this, and so does a test that wants the numbers without
    /// scraping the log.
    pub fn frame_stats(&self) -> frame_stats::Stats {
        self.stats.stats()
    }

    /// Reconfigure for a new physical size.
    ///
    /// Driven by `Resized`, which winit also emits after a
    /// `ScaleFactorChanged`, so both paths land here.
    pub fn resize(&mut self, width: u32, height: u32) {
        let (width, height) = (width.max(1), height.max(1));
        if (width, height) == (self.config.width, self.config.height) {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        // This configure is the one a pending stale swapchain was waiting
        // for, so the next frame does not repeat it.
        self.stale_swapchain = false;
    }

    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// The swapchain's pixel format.
    ///
    /// Load bearing rather than informational: `crt::Chain::frame` has to be
    /// told the format of the view it renders into, because librashader builds
    /// the last pass's pipeline against it. It is also the answer to the sRGB
    /// question -- see [`Gpu::new_async`], where the format is chosen.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// Get a swapchain image to draw into, or `None` if this frame cannot be
    /// drawn at all.
    ///
    /// The recording between this and [`Gpu::present`] is the caller's: on the
    /// only real path it is the glyph grid into an offscreen target and then
    /// the CRT chain from that target into `frame.view`.
    pub fn acquire(&mut self) -> Option<Frame> {
        // The debt a `Suboptimal` frame left behind. Here rather than there
        // because there the image was still in hand, and a surface with an
        // image outstanding is one wgpu refuses to configure.
        if std::mem::take(&mut self.stale_swapchain) {
            self.surface.configure(&self.device, &self.config);
        }
        // wgpu 30 returns an enum here, not a `Result`: acquiring a frame
        // has more outcomes than success and failure, and two of them
        // (`Suboptimal`, `Occluded`) are not errors at all.
        use wgpu::CurrentSurfaceTexture as Acquired;
        let surface = match self.surface.get_current_texture() {
            Acquired::Success(f) => f,
            // Usable this frame, but the surface no longer matches the
            // window. Draw it, and note the reconfigure the next frame owes
            // so the image after this one is right.
            Acquired::Suboptimal(f) => {
                self.stale_swapchain = true;
                f
            }
            // A surface goes stale across a resize or a monitor change;
            // reconfiguring and skipping the frame is the normal cure.
            Acquired::Outdated | Acquired::Lost => {
                self.surface.configure(&self.device, &self.config);
                return None;
            }
            // Minimised or covered: there is nothing to present to.
            Acquired::Occluded => return None,
            Acquired::Timeout => {
                log::debug!("timed out acquiring a frame");
                return None;
            }
            Acquired::Validation => {
                log::error!("validation error acquiring a frame");
                return None;
            }
        };

        let view = surface
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("robco frame"),
            });
        // `Frame` is the query-set owner (see `frame_stats`'s module doc):
        // every mark this frame writes, and the eventual resolve, land in
        // the encoder it already holds.
        let timing = self.stats.new_frame_timing(&self.device);
        Some(Frame {
            surface,
            view,
            encoder,
            timing,
        })
    }

    /// Submit the frame's commands and put the image on the screen.
    pub fn present(&mut self, mut frame: Frame) {
        // The resolve has to be in this encoder before it is `finish`ed --
        // there is no reaching back in after `present` submits.
        if let Some(timing) = &frame.timing {
            timing.resolve(&mut frame.encoder);
        }
        let submission = self.queue.submit(Some(frame.encoder.finish()));
        // wgpu 30 moved `present` from the texture onto the queue.
        self.queue.present(frame.surface);
        let timing = frame.timing.as_ref().map(|t| (t, submission));
        self.stats.after_present(&self.device, timing);
    }
}

/// One acquired swapchain image, its view, and the encoder the frame is
/// recorded into. Held together because they are only ever used together, and
/// because the view has to outlive the render passes that write to it.
pub struct Frame {
    surface: wgpu::SurfaceTexture,
    pub view: wgpu::TextureView,
    pub encoder: wgpu::CommandEncoder,
    /// This frame's query set, or `None` when it will not be GPU-timed
    /// (`--frame-stats` off, or the adapter lacks the feature pair). Owned
    /// here rather than on `Gpu`; see `frame_stats`'s module doc.
    timing: Option<frame_stats::FrameTiming>,
}

impl Frame {
    /// Write one of `draw_frame`'s eight timing marks, if this frame is
    /// being GPU-timed. A no-op otherwise, so every call site can mark
    /// unconditionally rather than checking availability itself.
    pub fn mark(&mut self, point: frame_stats::Mark) {
        if let Some(timing) = &self.timing {
            timing.mark(&mut self.encoder, point);
        }
    }

    /// Give up on timing this frame: no marks were written (or only some
    /// were), so resolving the query set at present-time would read an
    /// index the GPU never wrote. The no-glass fallback path (`clear`) is
    /// the one caller that needs this, since it never reaches the marks at
    /// all.
    pub fn discard_timing(&mut self) {
        self.timing = None;
    }

    /// The whole recording, for a window with nothing to draw yet: clear to the
    /// phosphor-ish background so a run is visibly alive and the surface
    /// configuration is visibly right. This is what every frame did before
    /// real content rendering existed, and it is still what a window whose
    /// GPU-side glass failed to build falls back to.
    pub fn clear(&mut self) {
        let _pass = self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.01,
                        g: 0.02,
                        b: 0.01,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }
}
