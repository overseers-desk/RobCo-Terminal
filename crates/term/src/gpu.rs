//! Offscreen wgpu context: a device, a colour target, and a pixel readback.
//!
//! This is not test-only scaffolding: the grid has to be drawn into an
//! offscreen texture *before* the CRT chain runs (that is the point the Rio
//! fork could not provide), so `Target` is the seam the pass graph hangs
//! off. The readback half is what the pixel-property tests measure.
//!
//! No window and no surface. Every claim this module makes is made about bytes
//! that came back from `Target::read_rgba`, so the readback path is deliberately
//! dull: a non-sRGB `Rgba8Unorm` target, so a shader value of 1.0 is the byte 255
//! and a shader value of 0.0 is the byte 0, with no transfer function in between
//! to smear the comparison.

use wgpu::util::DeviceExt as _;

pub const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// The wgpu features to ask for, filtered to those this adapter has.
///
/// Nothing in this crate uses either of them. They are here because the device
/// this module creates is the device the CRT chain runs on, and both features
/// have to be asked for at creation or they are unavailable for the life of the
/// device: `PIPELINE_CACHE`, without which librashader's `enable_cache` is a
/// validation panic on a rayon worker rather than an error, and
/// `FLOAT32_FILTERABLE`, without which an fp32 burn-in accumulator is a
/// validation error rather than a slower path.
///
/// `crt::device::required_features` is the authority on that set and says why
/// each feature is on it. This crate cannot call it: `crt-render` depends on
/// this one, so a dependency the other way is a cycle. The set is therefore
/// restated here, and `crt-render`'s `device_features` test asserts that this
/// function and `crt::device`'s agree on one adapter, so the copy cannot drift
/// in silence.
pub fn required_features(adapter: &wgpu::Adapter) -> wgpu::Features {
    adapter.features() & (wgpu::Features::PIPELINE_CACHE | wgpu::Features::FLOAT32_FILTERABLE)
}

pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_name: String,
    pub backend: String,
}

impl Gpu {
    pub fn new() -> Result<Self, String> {
        pollster::block_on(Self::new_async())
    }

    async fn new_async() -> Result<Self, String> {
        // No window, so no display handle. `_from_env` keeps WGPU_BACKEND
        // usable for reproducing a result on a specific backend.
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
                ..Default::default()
            })
            .await
            .map_err(|e| format!("no wgpu adapter: {e}"))?;
        let info = adapter.get_info();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("term offscreen device"),
                required_features: required_features(&adapter),
                ..Default::default()
            })
            .await
            .map_err(|e| format!("no wgpu device: {e}"))?;
        Ok(Self {
            device,
            queue,
            adapter_name: info.name,
            backend: format!("{:?}", info.backend),
        })
    }

    pub fn create_buffer_init(
        &self,
        label: &str,
        contents: &[u8],
        usage: wgpu::BufferUsages,
    ) -> wgpu::Buffer {
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage,
            })
    }
}

/// A colour attachment we can render into and then read back byte for byte.
pub struct Target {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}

impl Target {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("term offscreen target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TARGET_FORMAT,
            // `TEXTURE_BINDING` because this texture is the CRT filter
            // chain's input: a shader pass samples it, and a
            // texture without the binding usage cannot be sampled at all.
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            width,
            height,
        }
    }

    /// Copy the target into a mappable buffer and return tightly packed RGBA8.
    pub fn read_rgba(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Image {
        let unpadded = self.width * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded = unpadded.div_ceil(align) * align;

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("term readback buffer"),
            size: (padded * self.height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(Some(encoder.finish()));

        buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .expect("device poll for readback");

        let view = buffer
            .slice(..)
            .get_mapped_range()
            .expect("map readback buffer");
        let mut pixels = Vec::with_capacity((unpadded * self.height) as usize);
        for row in 0..self.height {
            let start = (row * padded) as usize;
            pixels.extend_from_slice(&view[start..start + unpadded as usize]);
        }
        drop(view);
        buffer.unmap();

        Image {
            width: self.width,
            height: self.height,
            pixels,
        }
    }
}

/// A readback: tightly packed RGBA8, row major, top row first.
#[derive(Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Image {
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [
            self.pixels[i],
            self.pixels[i + 1],
            self.pixels[i + 2],
            self.pixels[i + 3],
        ]
    }

    /// Count of channel values that are neither fully off nor fully on.
    /// This is the antialiasing measurement: an antialiased edge lands here.
    pub fn intermediate_channel_values(&self) -> usize {
        self.pixels
            .chunks_exact(4)
            .flat_map(|px| px[..3].iter())
            .filter(|v| **v != 0 && **v != 255)
            .count()
    }

    pub fn distinct_luma_values(&self) -> Vec<u8> {
        let mut seen = [false; 256];
        for px in self.pixels.chunks_exact(4) {
            seen[px[0] as usize] = true;
        }
        (0..=255u8).filter(|v| seen[*v as usize]).collect()
    }

    pub fn lit_pixels(&self) -> usize {
        self.pixels
            .chunks_exact(4)
            .filter(|px| px[0] != 0 || px[1] != 0 || px[2] != 0)
            .count()
    }

    /// Nearest-neighbour magnification by an integer factor. Used as the
    /// reference an integer-scaled render must equal exactly.
    pub fn upscale_nearest(&self, factor: u32) -> Image {
        let (w, h) = (self.width * factor, self.height * factor);
        let mut pixels = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                pixels.extend_from_slice(&self.pixel(x / factor, y / factor));
            }
        }
        Image {
            width: w,
            height: h,
            pixels,
        }
    }

    /// Number of differing pixels, plus the first offender for a failure report.
    pub fn diff(&self, other: &Image) -> Diff {
        if self.width != other.width || self.height != other.height {
            return Diff {
                differing: usize::MAX,
                first: Some((0, 0, [0; 4], [0; 4])),
                max_channel_delta: 255,
            };
        }
        let mut differing = 0usize;
        let mut first = None;
        let mut max_channel_delta = 0u8;
        for y in 0..self.height {
            for x in 0..self.width {
                let (a, b) = (self.pixel(x, y), other.pixel(x, y));
                if a != b {
                    differing += 1;
                    if first.is_none() {
                        first = Some((x, y, a, b));
                    }
                    for c in 0..4 {
                        max_channel_delta = max_channel_delta.max(a[c].abs_diff(b[c]));
                    }
                }
            }
        }
        Diff {
            differing,
            first,
            max_channel_delta,
        }
    }

    /// A readback the numbers cannot check: whether the text is legible.
    /// Every structural property this module checks would still hold if the
    /// atlas were mapping every character to the same glyph.
    pub fn ascii_preview(&self, cols: u32, rows: u32) -> String {
        let mut s = String::new();
        for y in 0..rows.min(self.height) {
            for x in 0..cols.min(self.width) {
                let px = self.pixel(x, y);
                let luma = px[0].max(px[1]).max(px[2]);
                s.push(match luma {
                    0 => ' ',
                    255 => '#',
                    _ => '+',
                });
            }
            s.push('\n');
        }
        s
    }

    /// Binary netpbm (P5, 8-bit grey) so the renders can be eyeballed without
    /// pulling an image encoder into the dependency list.
    pub fn write_pgm(&self, path: &std::path::Path) -> std::io::Result<()> {
        use std::io::Write as _;
        let mut out = Vec::with_capacity(self.pixels.len() / 4 + 32);
        write!(out, "P5\n{} {}\n255\n", self.width, self.height)?;
        for px in self.pixels.chunks_exact(4) {
            out.push(px[0].max(px[1]).max(px[2]));
        }
        std::fs::write(path, out)
    }
}

pub struct Diff {
    pub differing: usize,
    pub first: Option<(u32, u32, [u8; 4], [u8; 4])>,
    pub max_channel_delta: u8,
}

impl Diff {
    pub fn describe(&self) -> String {
        match self.first {
            None => "identical".to_string(),
            Some((x, y, a, b)) => format!(
                "{} differing pixels, max channel delta {}, first at ({x},{y}): {a:?} vs {b:?}",
                self.differing, self.max_channel_delta
            ),
        }
    }
}
