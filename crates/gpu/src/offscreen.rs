//! Offscreen wgpu context: a device, a colour target, and a pixel readback.
//!
//! No window and no surface. The grid is drawn into a [`Target`] before the CRT
//! chain runs, which is the point the Rio fork could not provide, and the
//! readback half is what the pixel-property tests measure.
//!
//! Every claim a test makes is made about bytes that came back from
//! [`Target::read_rgba`], so the readback path is deliberately dull: a non-sRGB
//! [`TARGET_FORMAT`], so a shader value of 1.0 is the byte 255 and a shader
//! value of 0.0 is the byte 0, with no transfer function in between to smear
//! the comparison.
//!
//! A target carries its format, so one device serves both the grid's
//! `Rgba8Unorm` and the measurement rig's `Rgba32Float`: [`Target::read_rgba`]
//! and [`Target::read_rgba_f32`] are the two readings of one padded copy path,
//! which differs only in bytes per pixel.

use wgpu::util::DeviceExt as _;

/// The grid's offscreen format. Non-sRGB on purpose: see the module docs.
pub const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_name: String,
    pub backend: String,
}

impl Gpu {
    /// An offscreen device with the features the chain needs.
    pub fn new() -> Result<Self, String> {
        Self::with_extra_features(wgpu::Features::empty())
    }

    /// An offscreen device with the chain's features plus `extra`, filtered to
    /// what the adapter offers.
    ///
    /// `extra` is for a caller whose own rendering needs a capability the chain
    /// does not, such as blending into a float32 target, which no shipped
    /// swapchain ever asks for.
    pub fn with_extra_features(extra: wgpu::Features) -> Result<Self, String> {
        pollster::block_on(Self::new_async(extra))
    }

    async fn new_async(extra: wgpu::Features) -> Result<Self, String> {
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
                label: Some("robco offscreen device"),
                required_features: crate::required_features(&adapter)
                    | (adapter.features() & extra),
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

/// A colour texture something renders into and a later pass samples.
///
/// `TEXTURE_BINDING` because this texture is the CRT filter chain's input: a
/// shader pass samples it, and a texture without the binding usage cannot be
/// sampled at all. `COPY_SRC` because [`read_back`] copies it.
pub fn color_texture(
    device: &wgpu::Device,
    label: &str,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}

/// Copy a texture into a mappable buffer and return its rows tightly packed.
///
/// One padded `copy_texture_to_buffer`, one `map_async`, one de-pad, for every
/// format anything here reads back; `bytes_per_pixel` is the whole of the
/// difference between an 8-bit target and a float32 one.
pub fn read_back(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
    bytes_per_pixel: u32,
) -> Vec<u8> {
    let unpadded = width * bytes_per_pixel;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded = unpadded.div_ceil(align) * align;

    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback buffer"),
        size: (padded * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("readback"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    let index = queue.submit(Some(encoder.finish()));

    buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(index),
            timeout: None,
        })
        .expect("device poll for readback");

    let view = buffer
        .slice(..)
        .get_mapped_range()
        .expect("map readback buffer");
    let mut bytes = Vec::with_capacity((unpadded * height) as usize);
    for row in 0..height {
        let start = (row * padded) as usize;
        bytes.extend_from_slice(&view[start..start + unpadded as usize]);
    }
    drop(view);
    buffer.unmap();
    bytes
}

/// A colour attachment we can render into and then read back byte for byte.
pub struct Target {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
}

impl Target {
    /// A target at `format`. [`TARGET_FORMAT`] for the terminal grid;
    /// `Rgba32Float` where a measurement needs finer steps than 1/255.
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Self {
        let texture = color_texture(device, "robco offscreen target", width, height, format);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            width,
            height,
            format,
        }
    }

    /// Read an 8-bit target back as tightly packed RGBA8.
    pub fn read_rgba(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Image {
        let pixels = read_back(device, queue, &self.texture, self.width, self.height, 4);
        Image {
            width: self.width,
            height: self.height,
            pixels,
        }
    }

    /// Read a float32 target back as RGBA f32, row padding removed.
    pub fn read_rgba_f32(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Vec<[f32; 4]> {
        let bytes = read_back(device, queue, &self.texture, self.width, self.height, 16);
        decode_rgba_f32(&bytes)
    }
}

/// Tightly packed `Rgba32Float` bytes as pixels.
///
/// Decoded four bytes at a time rather than reinterpreted: a `Vec<u8>` carries
/// no alignment guarantee an `&[f32]` cast could rely on.
pub(crate) fn decode_rgba_f32(bytes: &[u8]) -> Vec<[f32; 4]> {
    bytes
        .chunks_exact(16)
        .map(|px| {
            let f = |i: usize| f32::from_ne_bytes(px[i..i + 4].try_into().unwrap());
            [f(0), f(4), f(8), f(12)]
        })
        .collect()
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
    /// Every structural property the pixel tests check would still hold if the
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
