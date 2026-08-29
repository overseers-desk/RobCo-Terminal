//! The live-preview throughput instrument, exercised on a real device.
//!
//! `app::gpu::Gpu` needs a winit window (it creates a surface from one), and
//! what this proves does not: that the query-set plumbing --
//! mark/resolve/submit/readback/aggregate -- produces plausible numbers on
//! real hardware, and that the fallback behavior ("absent
//! = instrument reports unavailable, never panics") holds on a real device,
//! not only in the pure-logic unit tests `frame_stats`'s own module carries.
//! So this builds its own bare device, the same way `tests/suite/bank_chrome.rs`
//! and `gpu::harness` do: no surface, no window, `GpuLock`-serialised
//! against every other GPU test in the workspace.

use app::frame_stats::{self, Instrument, Mark};
use gpu::harness::GpuLock;

/// A bare device with no surface and no window.
struct Bare {
    device: wgpu::Device,
    queue: wgpu::Queue,
    _lock: GpuLock,
}

impl Bare {
    /// `with_timestamps` decides whether `frame_stats::required_features`
    /// is actually requested, even on a machine whose adapter offers it.
    /// `false` is how the done-test's fallback path is forced for real, on
    /// a real device: the instrument sees a `device.features()` that lacks
    /// the pair, exactly what it would see on hardware that never had it.
    fn new(with_timestamps: bool) -> Option<Self> {
        let lock = GpuLock::acquire().ok()?;
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
            ..Default::default()
        }))
        .ok()?;
        let required_features = if with_timestamps {
            frame_stats::required_features(&adapter)
        } else {
            wgpu::Features::empty()
        };
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("frame stats test device"),
            required_features,
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        }))
        .ok()?;
        Some(Self {
            device,
            queue,
            _lock: lock,
        })
    }
}

/// Twenty dependent clears of the same texture: cheap to set up (no shader,
/// no pipeline) and, because each pass writes the same resource as the last,
/// not collapsible into nothing by the driver the way twenty *independent*
/// clears might be. Enough real GPU work that a straddling timestamp pair
/// has something to measure.
fn burn_some_gpu_time(device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("frame stats test target"),
        size: wgpu::Extent3d {
            width: 1024,
            height: 1024,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    for i in 0..20 {
        let shade = f64::from(i) / 20.0;
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("frame stats test clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: shade,
                        g: shade,
                        b: shade,
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

#[test]
fn on_an_adapter_with_the_feature_pair_the_instrument_reports_plausible_timings() {
    let Some(bare) = Bare::new(true) else {
        eprintln!("skipping: no wgpu adapter on this machine");
        return;
    };
    let mut instrument = Instrument::new(&bare.device, &bare.queue, true);
    if !instrument.gpu_available() {
        eprintln!(
            "skipping: this adapter offers no TIMESTAMP_QUERY + \
             TIMESTAMP_QUERY_INSIDE_ENCODERS"
        );
        return;
    }

    // Eight frames, each timed the same way `draw_frame` times a real one:
    // frame-start, three sections each bracketed, frame-end, resolved in
    // the same encoder, submitted, then read back through the instrument.
    for _ in 0..8 {
        let timing = instrument
            .new_frame_timing(&bare.device)
            .expect("gpu_available() said yes");
        let mut encoder = bare
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        timing.mark(&mut encoder, Mark::FrameStart);
        timing.mark(&mut encoder, Mark::GridStart);
        burn_some_gpu_time(&bare.device, &mut encoder);
        timing.mark(&mut encoder, Mark::GridEnd);
        timing.mark(&mut encoder, Mark::ChainStart);
        burn_some_gpu_time(&bare.device, &mut encoder);
        timing.mark(&mut encoder, Mark::ChainEnd);
        timing.mark(&mut encoder, Mark::ChromeStart);
        burn_some_gpu_time(&bare.device, &mut encoder);
        timing.mark(&mut encoder, Mark::ChromeEnd);
        timing.mark(&mut encoder, Mark::FrameEnd);
        timing.resolve(&mut encoder);
        let submission = bare.queue.submit(Some(encoder.finish()));
        instrument.after_present(&bare.device, Some((&timing, submission)));
    }

    let stats = instrument.stats();
    assert_eq!(stats.gpu_samples, 8);
    for series in [
        stats.grid_ms,
        stats.chain_ms,
        stats.chrome_ms,
        stats.frame_ms,
    ] {
        let p = series.expect("eight gpu-timed frames were recorded");
        assert!(p.p50.is_finite() && p.p50 >= 0.0, "p50 {p:?}");
        assert!(p.p99.is_finite() && p.p99 >= p.p50 - 1e-9, "p99 {p:?}");
    }
    // Sixty dependent clears of a megapixel target across the three
    // sections is real work; on any GPU this machine plausibly ships,
    // the whole frame does not round-trip through a submit and a blocking
    // readback in exactly zero measured nanoseconds.
    assert!(stats.frame_ms.unwrap().p99 > 0.0, "{stats:?}");
}

#[test]
fn forcing_the_feature_pair_off_the_device_reports_unavailable_without_panicking() {
    let Some(bare) = Bare::new(false) else {
        eprintln!("skipping: no wgpu adapter on this machine");
        return;
    };
    let mut instrument = Instrument::new(&bare.device, &bare.queue, true);

    // The done-test's fallback: absent support reports unavailable, and
    // every call on the instrument stays a plain no-op rather than a panic
    // reaching for a query set the device was never given.
    assert!(!instrument.gpu_available());
    assert!(instrument.new_frame_timing(&bare.device).is_none());

    let stats_before = instrument.stats();
    assert!(stats_before.grid_ms.is_none());
    assert!(stats_before.chain_ms.is_none());
    assert!(stats_before.chrome_ms.is_none());
    assert!(stats_before.frame_ms.is_none());
    assert_eq!(stats_before.gpu_samples, 0);

    // Present cadence needs no GPU feature at all, so it still counts even
    // here -- two calls, one interval between them.
    instrument.after_present(&bare.device, None);
    instrument.after_present(&bare.device, None);
    let stats_after = instrument.stats();
    assert_eq!(stats_after.cadence_samples, 1);
    assert!(stats_after.present_interval_ms.is_some());
    assert_eq!(stats_after.gpu_samples, 0);
}
