//! A device with no display, held under a machine-wide lock, for the tests.
//!
//! There is no surface and no window in here: a chain renders into a texture we
//! own and we copy that texture back to look at it, which is what lets the
//! tests assert on real GPU output under Xvfb, or with no X at all. It handles
//! arbitrary sizes and arbitrary frame content, since the burn-in mask tests
//! need two cells behaving differently in one frame.
//!
//! Every [`Locked`] holds a machine-wide lock for its whole life, so GPU-backed
//! tests serialise across processes as well as within one. [`GpuLock`] says why
//! that is not optional.
//!
//! The whole module sits behind the `harness` feature, which no shipped binary
//! turns on: a test rig that ships is a test rig that can be reached from the
//! application, and this one blocks on a lock.

use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use crate::offscreen::decode_rgba_f32;
use crate::{color_texture, read_back, Gpu};

/// Readback format for a measured chain's final output. 32-bit float keeps a
/// ghost measurable well below the 1/255 floor an 8-bit target would impose,
/// and the alpha channel arrives as the value the shader wrote rather than a
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
/// It is held for the life of the [`Locked`] device, not just across
/// `request_device`, because the hazard is two live devices, not two device
/// *creations*. That makes GPU-backed tests slow under load rather than
/// concurrent, which is the trade: a wedged run costs more than a serialised
/// one.
///
/// A failure to take it is an error, deliberately, rather than a warning and a
/// race: a caller that treats "no device" as "skip this test" would otherwise
/// skip in silence exactly when the machine is busiest.
///
/// It is public because [`Locked`] is not the only device in the tree. The
/// plain [`Gpu`] is a shipping type that creates a device for a window and
/// takes no lock by design, but the tests that drive it headless are GPU tests
/// like any other and race the same way; they hold one of these for the life of
/// their device. Acquire it *before* the device and drop it *after*, which for
/// a tuple binding means putting the device first, since fields drop in
/// declaration order.
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

/// The offscreen device the tests measure on, under the machine-wide lock.
///
/// It derefs to the plain [`Gpu`], so `device`, `queue`, `adapter_name` and
/// `backend` read the same as they do on the shipping type. What it adds is the
/// lock, the float32 target this rig renders into, and the frame content a
/// shader test feeds in.
pub struct Locked {
    gpu: Gpu,
    /// Last field on purpose: struct fields drop in declaration order, so the
    /// device and queue are gone before the lock is released and the next
    /// process in the queue never overlaps this one's device.
    _lock: GpuLock,
}

impl std::ops::Deref for Locked {
    type Target = Gpu;

    fn deref(&self) -> &Gpu {
        &self.gpu
    }
}

impl Locked {
    pub fn new() -> Result<Self> {
        // Before the adapter, not after: two processes that both got as far as
        // an adapter have already paid for the collision.
        let lock = GpuLock::acquire()?;

        // `FLOAT32_BLENDABLE` on top of the chain's set, and deliberately not
        // part of it: this rig renders into [`OUTPUT_FORMAT`], which is
        // `Rgba32Float`, and wgpu refuses a blending pipeline on a float32
        // target without it. A shipped window has no such target, its swapchain
        // is `Bgra8Unorm` and blendable everywhere, so asking for this where the
        // chain runs would be asking for a capability the chain does not use.
        let gpu =
            Gpu::with_extra_features(wgpu::Features::FLOAT32_BLENDABLE).map_err(GpuError::new)?;

        Ok(Self { gpu, _lock: lock })
    }

    /// Whether a blending pipeline can be built against [`OUTPUT_FORMAT`] on
    /// this machine. A test that composites has to say so rather than assert
    /// through a validation panic.
    pub fn blends_float32(&self) -> bool {
        self.gpu
            .device
            .features()
            .contains(wgpu::Features::FLOAT32_BLENDABLE)
    }

    /// Whether this device can carry an fp32 accumulator through a filtering
    /// sampler. Without it that is a validation error, not a slower path.
    pub fn float32_filterable(&self) -> bool {
        crate::supports_fp32_accumulator(&self.gpu.device)
    }

    /// A chain's input. librashader copies this texture into its history ring,
    /// so it needs `COPY_SRC` on top of the obvious sampling and upload usages.
    pub fn make_input(&self, w: u32, h: u32) -> wgpu::Texture {
        self.gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("chain input"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: crate::TARGET_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
    }

    pub fn make_output(&self, w: u32, h: u32) -> wgpu::Texture {
        color_texture(&self.gpu.device, "chain output", w, h, OUTPUT_FORMAT)
    }

    /// Upload one RGBA8 frame.
    pub fn upload(&self, tex: &wgpu::Texture, w: u32, h: u32, pixels: &[u8]) {
        self.gpu.queue.write_texture(
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

    /// Copy an [`OUTPUT_FORMAT`] texture back and return it as RGBA f32 pixels,
    /// row padding removed.
    pub fn read_output(&self, tex: &wgpu::Texture, w: u32, h: u32) -> Result<Vec<[f32; 4]>> {
        let bytes = read_back(&self.gpu.device, &self.gpu.queue, tex, w, h, 16);
        Ok(decode_rgba_f32(&bytes))
    }
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
