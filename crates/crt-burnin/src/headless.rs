//! A device with no display and a way to read pixels back off it.
//!
//! There is no surface and no window anywhere in here: the chain renders into a
//! texture we own and we copy that texture back to look at it, which is what
//! lets the tests assert on real GPU output under Xvfb (or with no X at all).
//! It handles arbitrary sizes and arbitrary frame content, since the mask
//! tests need two cells behaving differently in one frame.
//!
//! This is the only headless GPU harness in the tree. Three near-copies of it
//! (this one, `crt::testutil`, and `crt-render`'s `tests/support`) once
//! existed -- one measurement rig described three ways; they are folded in
//! here because this crate is the lowest one in the graph that owns a
//! device, and `crt::testutil` re-exports what it needs rather than keeping a
//! second copy. So the union of what the three could do lives here: a padded
//! readback that works at any width, frames with several independently lit
//! cells, and the single-pass mount ([`render_single_pass`]) the per-pass tests
//! measure a shader against.
//!
//! Every [`Gpu`] holds a machine-wide lock for its whole life, so GPU-backed
//! tests serialise across processes as well as within one. [`GpuLock`] says why
//! that is not optional.

use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

use librashader::presets::ShaderFeatures;
use librashader::runtime::wgpu::{FilterChain, FilterChainOptions, WgpuOutputView};
use librashader::runtime::{FilterChainParameters, Size, Viewport};

/// Readback format for the chain's final output. 32-bit float keeps the ghost
/// measurable well below the 1/255 floor an 8-bit target would impose, and the
/// alpha channel arrives as the mask value the shader wrote rather than a
/// quantised one.
pub const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;

#[derive(Debug)]
pub struct GpuError(String);

impl fmt::Display for GpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for GpuError {}

impl GpuError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self(msg.into())
    }
}

type Result<T> = std::result::Result<T, GpuError>;

/// The features a librashader chain wants, filtered to those this adapter has.
///
/// `crt::device::required_features` is the authority on this set and says why
/// each feature is on it. This crate cannot call it (`crt-render` depends on
/// this one, not the other way round), so the set is restated here and
/// `crt-render`'s `device_features` test asserts the two agree on one adapter.
pub fn required_features(adapter: &wgpu::Adapter) -> wgpu::Features {
    adapter.features() & (wgpu::Features::PIPELINE_CACHE | wgpu::Features::FLOAT32_FILTERABLE)
}

/// The path of the machine-wide GPU lock, `robco-gpu-tests.lock` in the
/// process's temporary directory.
///
/// One caveat comes with `temp_dir()`: it honours `TMPDIR`, so two runs with
/// different `TMPDIR` settings lock different files and do not see each other.
/// Every harness in this tree leaves `TMPDIR` alone, which is what makes the
/// path machine-wide in practice.
pub fn lock_path() -> PathBuf {
    std::env::temp_dir().join("robco-gpu-tests.lock")
}

/// A machine-wide exclusive lock on "somebody is holding a GPU device".
///
/// Three concurrent chain-heavy devices segfault inside the software ICD;
/// harder to see, three concurrent workspace runs from separate worktrees
/// wedge in `pass_graph` for 55 minutes at 0% CPU.
/// `--test-threads=1` cannot reach that, because the racing devices are in
/// different *processes*. This is the lock that can: an exclusive `flock` on a
/// file under [`lock_path`], taken before the adapter request and released when
/// the fd closes.
///
/// It is held for the life of the [`Gpu`], not just across `request_device`,
/// because the hazard is two live devices, not two device *creations*. That
/// makes GPU-backed tests slow under load rather than concurrent, which is the
/// trade: a wedged run costs more than a serialised one.
///
/// A failure to take it is an error, deliberately, rather than a warning and a
/// race: a caller that treats "no device" as "skip this test" would otherwise
/// skip in silence exactly when the machine is busiest.
///
/// It is public because [`Gpu`] is not the only device in the tree. `term`'s
/// own `term::gpu::Gpu` is a shipping type that creates a device for a window
/// and takes no lock by design, but the tests that drive it headless are GPU
/// tests like any other and race the same way; they hold one of these for the
/// life of their device. Acquire it *before* the device and drop it *after*,
/// which for a tuple binding means putting the device first, since fields drop
/// in declaration order.
pub struct GpuLock(std::fs::File);

impl GpuLock {
    /// Block until this process may hold a GPU device.
    pub fn acquire() -> Result<Self> {
        let path = lock_path();
        let file = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&path)
            .map_err(|e| GpuError::new(format!("cannot open GPU lock {}: {e}", path.display())))?;
        // Blocking, not `try_lock`: waiting for the other run is the point.
        file.lock()
            .map_err(|e| GpuError::new(format!("cannot lock {}: {e}", path.display())))?;
        Ok(Self(file))
    }
}

impl Drop for GpuLock {
    fn drop(&mut self) {
        // Closing the fd would release it anyway; saying so costs one line and
        // makes "released on drop" a property of this file rather than of the
        // platform's fd semantics.
        let _ = self.0.unlock();
    }
}

pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_name: String,
    pub backend: String,
    /// Whether the device has wgpu's `PIPELINE_CACHE`. librashader's
    /// `enable_cache` is only safe to set when this is true: it calls
    /// `create_pipeline_cache` without checking, and on a device without the
    /// feature that is a validation panic on a rayon worker, not a `Result`.
    pub pipeline_cache: bool,
    /// Whether the device has `FLOAT32_FILTERABLE`. librashader builds one bind
    /// group layout for every sampled texture and it asks for a filterable
    /// float sample type, so an `Rgba32Float` framebuffer anywhere in the chain
    /// is a validation error without this feature, whatever the pass's own
    /// filter setting says.
    pub float32_filterable: bool,
    /// The rig's own capability, not the chain's: whether this device can blend
    /// into [`OUTPUT_FORMAT`]. See [`Gpu::blends_float32`].
    pub float32_blendable: bool,
    /// Last field on purpose: struct fields drop in declaration order, so the
    /// device and queue are gone before the lock is released and the next
    /// process in the queue never overlaps this one's device.
    _lock: GpuLock,
}

impl Gpu {
    pub fn new() -> Result<Self> {
        // Before the adapter, not after: two processes that both got as far as
        // an adapter have already paid for the collision.
        let lock = GpuLock::acquire()?;

        // wgpu 30 dropped `Default` on `InstanceDescriptor` and makes you say
        // whether you have a display handle. We do not, which is the point.
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
            ..Default::default()
        }))
        .map_err(|e| GpuError::new(format!("no wgpu adapter: {e}")))?;

        let info = adapter.get_info();
        let pipeline_cache = adapter.features().contains(wgpu::Features::PIPELINE_CACHE);
        let float32_filterable = adapter
            .features()
            .contains(wgpu::Features::FLOAT32_FILTERABLE);

        // The rig's own feature, on top of the chain's, and deliberately not in
        // `required_features`: this harness renders into [`OUTPUT_FORMAT`],
        // which is `Rgba32Float`, and wgpu refuses a blending pipeline on a
        // float32 target without it. A shipped window has no such target -- its
        // swapchain is `Bgra8Unorm`, blendable everywhere -- so asking for this
        // where the chain runs would be asking for a capability the chain does
        // not use. `crt-render`'s `device_features` test holds the three
        // restatements of *the chain's* set to one answer, and this is not part
        // of it.
        let float32_blendable = adapter
            .features()
            .contains(wgpu::Features::FLOAT32_BLENDABLE);
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("crt-burnin headless"),
            required_features: required_features(&adapter)
                | (adapter.features() & wgpu::Features::FLOAT32_BLENDABLE),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        }))
        .map_err(|e| GpuError::new(format!("no wgpu device: {e}")))?;

        Ok(Self {
            device,
            queue,
            adapter_name: info.name.clone(),
            backend: format!("{:?}", info.backend),
            pipeline_cache,
            float32_filterable,
            float32_blendable,
            _lock: lock,
        })
    }

    /// Whether a blending pipeline can be built against [`OUTPUT_FORMAT`] on
    /// this machine. A test that composites has to say so rather than assert
    /// through a validation panic.
    pub fn blends_float32(&self) -> bool {
        self.float32_blendable
    }

    /// The chain's input. librashader copies this texture into its history ring,
    /// so it needs `COPY_SRC` on top of the obvious sampling and upload usages.
    pub fn make_input(&self, w: u32, h: u32) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("chain input"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
    }

    pub fn make_output(&self, w: u32, h: u32) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("chain output"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OUTPUT_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    }

    /// Upload one RGBA8 frame.
    pub fn upload(&self, tex: &wgpu::Texture, w: u32, h: u32, pixels: &[u8]) {
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
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

    /// Copy the output texture back and return it as RGBA f32 pixels, row
    /// padding removed.
    pub fn read_output(&self, tex: &wgpu::Texture, w: u32, h: u32) -> Result<Vec<[f32; 4]>> {
        let unpadded = w * 16;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded = unpadded.div_ceil(align) * align;
        let size = (padded * h) as u64;

        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut cmd = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("readback"),
            });
        cmd.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        let idx = self.queue.submit([cmd.finish()]);

        buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(idx),
                timeout: None,
            })
            .map_err(|e| GpuError::new(format!("poll: {e}")))?;

        let out = {
            let view = buffer
                .slice(..)
                .get_mapped_range()
                .map_err(|e| GpuError::new(format!("map: {e}")))?;
            let mut px = Vec::with_capacity((w * h) as usize);
            for row in 0..h {
                let start = (row * padded) as usize;
                let floats = cast_f32(&view[start..start + unpadded as usize]);
                px.extend(floats.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]));
            }
            px
        };
        buffer.unmap();
        Ok(out)
    }
}

/// Reinterpret a byte slice as f32. Rows start at multiples of 256, so every
/// slice handed here is 4-byte aligned by construction; the assert holds us to
/// it if the padding rule ever changes.
fn cast_f32(bytes: &[u8]) -> &[f32] {
    assert_eq!(bytes.as_ptr() as usize % std::mem::align_of::<f32>(), 0);
    unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const f32, bytes.len() / 4) }
}

/// A rectangle of lit pixels, in texel coordinates.
#[derive(Debug, Clone, Copy)]
pub struct Cell {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
}

impl Cell {
    pub const fn new(x0: u32, y0: u32, x1: u32, y1: u32) -> Self {
        Self { x0, y0, x1, y1 }
    }

    pub fn contains(&self, x: u32, y: u32) -> bool {
        (self.x0..self.x1).contains(&x) && (self.y0..self.y1).contains(&y)
    }

    /// The middle of the cell, as an index into a `w`-wide readback.
    pub fn centre_index(&self, w: u32) -> usize {
        let cx = (self.x0 + self.x1) / 2;
        let cy = (self.y0 + self.y1) / 2;
        (cy * w + cx) as usize
    }
}

/// Pixel index for `(x, y)` in a `w`-wide readback, row major.
pub fn px_index(w: u32, x: u32, y: u32) -> usize {
    (y * w + x) as usize
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

/// An RGBA8 frame: white inside any of `lit`, black everywhere else.
pub fn frame_pixels(w: u32, h: u32, lit: &[Cell]) -> Vec<u8> {
    let mut px = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let v = if lit.iter().any(|c| c.contains(x, y)) {
                255
            } else {
                0
            };
            px[i] = v;
            px[i + 1] = v;
            px[i + 2] = v;
            px[i + 3] = 255;
        }
    }
    px
}
