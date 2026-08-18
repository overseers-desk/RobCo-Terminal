//! Glyph atlas: cosmic-text does the shaping and the rasterising, we own the
//! bytes between swash and the GPU. Upload takes a borrowed device/queue
//! rather than owning a `Gpu`, and the packer reports how full it is.
//!
//! That ownership is the entire reason this module exists instead of a call to
//! glyphon. swash hands back an 8-bit coverage mask (`Format::Alpha`), which is
//! antialiasing by construction: a stem that lands half over a pixel comes back
//! as 128, not as on or off. cosmic-text's `CacheKeyFlags::PIXEL_FONT` does not
//! change this: it only rounds the subpixel offset before rasterising
//! (cosmic-text-0.19.0/src/swash.rs:48). For the low-resolution half of the
//! catalogue somebody has to threshold, and the only place it can happen
//! without leaving a trace is on the way into the atlas.
//!
//! Which half is being drawn is the whole of the mode question. One switch
//! decides it: the 16 pixel faces are drawn with antialiasing off and the 8
//! scalable ones with it on. So the
//! atlas has two modes ([`Rasterization`]), the flag picks between them
//! ([`Rasterization::for_face`]), and the texture is R8 coverage either way --
//! the binary mode is coverage that happens to hold only 0 and 255. The shader
//! reads that byte as alpha and never asks which mode produced it, so the
//! antialiasing-on path needed no second pipeline; see `shader.wgsl`.
//!
//! It is also why the rasterising is done here rather than through
//! `cosmic_text::SwashCache`. cosmic-text shapes; it does not get to choose the
//! picture. Its cache renders from `[ColorOutline, ColorBitmap, Outline]` with
//! no monochrome `Source::Bitmap` in the list, so a face with an embedded
//! bitmap strike at the ppem it was drawn at -- which is every low-resolution
//! entry in the catalogue -- comes back as its *outline* instead, and the
//! threshold above then eats the stems. [`crate::fonts::raster`] is the one
//! rule, shared with the LED raster.
//!
//! Two further things are deliberate here:
//!
//! * Cache keys are built with a hard zero subpixel position, so a given
//!   character has exactly one bitmap regardless of where on the line it sits.
//!   Subpixel-positioned variants would defeat the whole exercise.
//! * Glyphs are placed on an integer cell grid computed from the face's own
//!   advance, not at the fractional pen positions cosmic-text would hand us.
//!   A terminal is a grid; pretending otherwise reintroduces the fractional
//!   offsets we just spent the previous point removing.

use std::collections::HashMap;

use cosmic_text::{
    fontdb, Attrs, Buffer, CacheKeyFlags, Family, FontSystem, Metrics, Shaping, SwashContent,
};
use swash::scale::ScaleContext;

use crate::fonts::raster;
use crate::fonts::sizing::ResolvedFont;
use crate::fonts::FontEntry;

/// The face a font that will not load is replaced by: the catalogue's first
/// bundled entry. Bundled, so it is `include_bytes!` of a file in this
/// repository and cannot itself be the thing that went missing.
const FALLBACK_FACE: &str = "TERMINESS_SCALED";

/// How the 8-bit coverage mask becomes atlas content.
///
/// The two arms are selected by one switch -- whether the face is
/// low-resolution -- and nothing else selects between them:
/// [`Rasterization::for_face`] is the whole rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rasterization {
    /// Coverage >= threshold is ink, everything else is paper. The atlas holds
    /// only 0 and 255 afterwards, which is what "antialiasing off" has to mean
    /// if it is to mean anything checkable. This is the low-resolution half of
    /// the catalogue, and the pixel-exactness properties are stated about it.
    Binary { threshold: u8 },
    /// Coverage passed through untouched, to be read as alpha downstream: the
    /// antialiasing-on path, which is the scalable half of the catalogue.
    ///
    /// It is also, unchanged, the control the pixel properties were always
    /// measured against: it uses the identical pipeline, so any intermediate
    /// values in a render of a `Binary` atlas would have to have come from the
    /// pipeline rather than from the rasteriser. That the shipped scalable path
    /// and that control are the same code is the point -- the control is
    /// measuring the thing that ships.
    Coverage,
}

impl Rasterization {
    /// The rasterisation mode a resolved face is entitled to, and the one place
    /// that decision is made.
    ///
    /// [`ResolvedFont::antialias`] is `!entry.low_resolution`, computed in
    /// `fonts::sizing::resolve`. Reading it here rather than restating the
    /// flag keeps one answer to "may this face be antialiased?" in the tree:
    /// the sizing seam computes it, the atlas obeys it.
    pub fn for_face(resolved: &ResolvedFont) -> Self {
        if resolved.antialias {
            Rasterization::Coverage
        } else {
            Rasterization::Binary {
                threshold: crate::DEFAULT_THRESHOLD,
            }
        }
    }

    fn apply(self, v: u8) -> u8 {
        match self {
            Rasterization::Binary { threshold } => {
                if v >= threshold {
                    255
                } else {
                    0
                }
            }
            Rasterization::Coverage => v,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GlyphSlot {
    /// Position in the atlas texture, in texels.
    pub atlas_x: u32,
    pub atlas_y: u32,
    /// Size of the bitmap, in texels.
    pub width: u32,
    pub height: u32,
    /// Offset from the pen position to the bitmap's top-left, in *unscaled*
    /// raster pixels. swash's `placement.left` / `placement.top`.
    pub left: i32,
    pub top: i32,
}

/// Integer cell metrics, in unscaled raster pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellMetrics {
    pub width: u32,
    pub height: u32,
    pub baseline: i32,
}

pub struct GlyphAtlas {
    /// Held only to keep the texture alive behind `view`.
    #[allow(dead_code)]
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
    pub cell: CellMetrics,
    pub rasterization: Rasterization,
    /// The size everything in here was rasterised at. Recorded so a DPR change
    /// can assert it did *not* move.
    pub raster_pixel_size: u32,
    slots: HashMap<char, GlyphSlot>,
    /// Every distinct byte value written into the atlas. The evidence for
    /// property 1 is taken here, before the GPU is involved at all.
    pub value_histogram: [u64; 256],
}

impl GlyphAtlas {
    pub fn slot(&self, c: char) -> Option<&GlyphSlot> {
        self.slots.get(&c)
    }

    pub fn distinct_values(&self) -> Vec<u8> {
        (0..=255u8)
            .filter(|v| self.value_histogram[*v as usize] > 0)
            .collect()
    }

    pub fn intermediate_value_count(&self) -> u64 {
        (1..255u8).map(|v| self.value_histogram[v as usize]).sum()
    }

    pub fn total_value_count(&self) -> u64 {
        self.value_histogram.iter().sum()
    }
}

/// Holds the shaping side of cosmic-text for one catalogue face, bundled or
/// system.
pub struct FontContext {
    pub font_system: FontSystem,
    /// swash's own scaler state. It is here rather than a `SwashCache` because
    /// this crate picks the rasterising sources itself; see the module doc and
    /// [`crate::fonts::raster`].
    scale_context: ScaleContext,
    pub font_id: fontdb::ID,
    pub family: String,
}

impl FontContext {
    pub fn new(spec: &FontEntry) -> Self {
        let mut db = fontdb::Database::new();
        db.load_font_data(spec.data().to_vec());
        // A bundled face cannot fail here -- it is `include_bytes!` of a file
        // in this repository. A system face can: `FontEntry::data` reads it
        // from the machine at selection time, and a font uninstalled since the
        // menu was built comes back empty.
        //
        // That is an ordinary thing for a machine to do, and both sides of the
        // seam already say so: `FontEntry::data`'s doc calls the empty slice
        // something "the atlas reports as a face it cannot build rather than
        // as a wrong picture", and `system::face_data`'s says "the caller
        // turns it into a fallback". Aborting was neither. So the bundled
        // default face is loaded instead and the substitution is named in the
        // log -- the user gets the wrong font, which they can see and fix,
        // rather than no terminal at all.
        if db.faces().next().is_none() {
            log::warn!(
                "the face {:?} produced no font ({} bytes read); falling back \
                 to {FALLBACK_FACE}. A system font removed since the catalogue \
                 was built does this.",
                spec.name,
                spec.data().len()
            );
            if let Some(fallback) = crate::fonts::font_by_name(FALLBACK_FACE) {
                db.load_font_data(fallback.data().to_vec());
            }
        }
        let face = db
            .faces()
            .next()
            .unwrap_or_else(|| {
                // Not a machine's doing: the bundled fallback is compiled into
                // this binary, so reaching here means the build itself is
                // wrong and there is nothing to degrade to.
                panic!(
                    "the face {:?} produced no font and neither did the \
                     bundled {FALLBACK_FACE}",
                    spec.name
                )
            })
            .clone();
        let font_id = face.id;
        let family = face
            .families
            .first()
            .map(|(name, _)| name.clone())
            .unwrap_or_default();
        // Locale is fixed rather than read from the environment so two runs on
        // two machines shape the same text the same way.
        let font_system = FontSystem::new_with_locale_and_db("en-US".to_string(), db);
        Self {
            font_system,
            scale_context: ScaleContext::new(),
            font_id,
            family,
        }
    }

    /// Shape a string and return one glyph id plus advance per glyph, in order.
    /// Deliberately a flat list: a terminal owns the cell assignment.
    fn shape_to_glyph_ids(&mut self, text: &str, pixel_size: f32) -> Vec<(u16, f32)> {
        let metrics = Metrics::new(pixel_size, pixel_size);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        let family = self.family.clone();
        let attrs = Attrs::new()
            .family(Family::Name(&family))
            // Tell cosmic-text this is a pixel face. On its own this only
            // rounds the subpixel offset, but it is the correct signal to send
            // and it keeps our cache keys honest.
            .cache_key_flags(CacheKeyFlags::PIXEL_FONT);
        buffer.set_text(text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.font_system, false);

        let mut out = Vec::new();
        for run in buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                out.push((glyph.glyph_id, glyph.w));
            }
        }
        out
    }

    /// Cell metrics for a face at a given raster size. The advance comes from
    /// the face, then is rounded to an integer. A terminal cell that is 6.4
    /// pixels wide has no pixel-exact rendering to offer.
    pub fn cell_metrics(&mut self, resolved: &ResolvedFont) -> CellMetrics {
        let px = resolved.raster_pixel_size as f32;
        let advance = self
            .shape_to_glyph_ids("M", px)
            .first()
            .map(|(_, w)| *w)
            .unwrap_or(px * 0.5);
        // Cell width is the face's own advance, rounded, and nothing else.
        //
        // An aspect-ratio squeeze factor deliberately does not appear here.
        // That factor -- `0.5 * glyph_height / glyph_width` -- squeezes the
        // rendered terminal horizontally at display time. Folding it into the
        // advance packs cells closer than the glyphs are wide, which for Pet
        // Me at 8px means a 4-pixel cell holding an 8-pixel glyph, and
        // neighbouring characters overwrite each other. See
        // `ResolvedFont::font_width` for where that factor has to be applied
        // instead.
        let width = (advance.round() as i64).max(1) as u32;
        let height = (resolved.raster_pixel_size as i32 + resolved.line_spacing).max(1) as u32;
        // Baseline from the face's own ascent, snapped to a whole pixel.
        let baseline = self
            .font_system
            .get_font(self.font_id, fontdb::Weight::NORMAL)
            .map(|f| {
                let m = f.as_swash().metrics(&[]);
                let upem = if m.units_per_em == 0 {
                    1000.0
                } else {
                    m.units_per_em as f32
                };
                (m.ascent / upem * px).round() as i32
            })
            .unwrap_or((px * 0.8).round() as i32);
        CellMetrics {
            width,
            height,
            baseline,
        }
    }

    /// Rasterise every character in `charset` at the resolved size, threshold
    /// it, and upload one atlas.
    pub fn build_atlas(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resolved: &ResolvedFont,
        charset: &str,
        rasterization: Rasterization,
    ) -> GlyphAtlas {
        let px = resolved.raster_pixel_size as f32;
        let cell = self.cell_metrics(resolved);

        let mut chars: Vec<char> = charset.chars().collect();
        chars.sort_unstable();
        chars.dedup();

        struct Raster {
            c: char,
            w: u32,
            h: u32,
            left: i32,
            top: i32,
            data: Vec<u8>,
        }
        // Shaping first, rasterising second, because they want the same `self`:
        // shaping needs the whole `FontSystem`, and the scaler holds a
        // `FontRef` borrowed out of one face for the length of the loop.
        let mut glyphs = Vec::with_capacity(chars.len());
        for c in chars {
            let mut buf = [0u8; 4];
            if let Some((glyph_id, _)) = self
                .shape_to_glyph_ids(c.encode_utf8(&mut buf), px)
                .first()
                .copied()
            {
                glyphs.push((c, glyph_id));
            }
        }

        let mut rasters = Vec::new();
        // `None` only if the face this context was built from has gone, which
        // it cannot: the database holds it and nothing removes it. An empty
        // atlas is still a legal one, so this is a `if let` rather than a panic.
        if let Some(font) = self
            .font_system
            .get_font(self.font_id, fontdb::Weight::NORMAL)
        {
            // `hint(true)` is what cosmic-text asked for and what the LED
            // raster asks for: it changes nothing on the strike path and is the
            // right answer on the outline one.
            let mut scaler = self
                .scale_context
                .builder(font.as_swash())
                .size(px)
                .hint(true)
                .build();

            for (c, glyph_id) in glyphs {
                let Some(image) = raster::glyph_mask(&mut scaler, glyph_id) else {
                    continue;
                };
                let (w, h) = (image.placement.width, image.placement.height);
                if w == 0 || h == 0 {
                    // Space and friends: no ink, no atlas entry needed.
                    continue;
                }
                // Alpha masks only. A colour bitmap in a terminal face would
                // mean the pixel-font assumption is wrong, so say so rather
                // than silently rendering something plausible. `raster`'s
                // source list cannot produce one; this is the guard that keeps
                // the list from quietly growing one.
                assert!(
                    matches!(image.content, SwashContent::Mask),
                    "glyph {c:?} rasterised as {:?}, not an alpha mask; \
                     a terminal face with colour glyphs is outside what either \
                     rasterisation mode can carry",
                    image.content
                );
                let data = image
                    .data
                    .iter()
                    .map(|v| rasterization.apply(*v))
                    .collect::<Vec<u8>>();
                rasters.push(Raster {
                    c,
                    w,
                    h,
                    left: image.placement.left,
                    top: image.placement.top,
                    data,
                });
            }
        }

        // Shelf packing. Padding between glyphs would be pointless: nothing
        // ever filters across a glyph boundary, because nothing ever filters.
        let atlas_w: u32 = 512;
        let mut slots = HashMap::new();
        let (mut pen_x, mut pen_y, mut shelf_h) = (0u32, 0u32, 0u32);
        for r in &rasters {
            if pen_x + r.w > atlas_w {
                pen_x = 0;
                pen_y += shelf_h;
                shelf_h = 0;
            }
            slots.insert(
                r.c,
                GlyphSlot {
                    atlas_x: pen_x,
                    atlas_y: pen_y,
                    width: r.w,
                    height: r.h,
                    left: r.left,
                    top: r.top,
                },
            );
            pen_x += r.w;
            shelf_h = shelf_h.max(r.h);
        }
        let atlas_h = (pen_y + shelf_h).max(1);

        let mut pixels = vec![0u8; (atlas_w * atlas_h) as usize];
        for r in &rasters {
            let slot = slots[&r.c];
            for row in 0..r.h {
                let src = (row * r.w) as usize;
                let dst = ((slot.atlas_y + row) * atlas_w + slot.atlas_x) as usize;
                pixels[dst..dst + r.w as usize].copy_from_slice(&r.data[src..src + r.w as usize]);
            }
        }

        let mut value_histogram = [0u64; 256];
        for r in &rasters {
            for v in &r.data {
                value_histogram[*v as usize] += 1;
            }
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph atlas"),
            size: wgpu::Extent3d {
                width: atlas_w,
                height: atlas_h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(atlas_w),
                rows_per_image: Some(atlas_h),
            },
            wgpu::Extent3d {
                width: atlas_w,
                height: atlas_h,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        GlyphAtlas {
            texture,
            view,
            width: atlas_w,
            height: atlas_h,
            cell,
            rasterization,
            raster_pixel_size: resolved.raster_pixel_size,
            slots,
            value_histogram,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A face the machine has lost since the catalogue was built.
    ///
    /// `fonts/mod.rs` promises this degrades ("an unreadable file comes back
    /// empty, which the atlas reports as a face it cannot build rather than
    /// as a wrong picture"), and `fonts/system.rs` says the caller "turns it
    /// into a fallback". The atlas used to abort the process instead, so
    /// uninstalling a font that happened to be the selected one meant the
    /// terminal would no longer start at all -- and it could not be started
    /// to change the setting back.
    #[test]
    fn a_face_with_no_data_builds_the_context_on_the_bundled_fallback() {
        let entry = crate::fonts::missing_system_face(
            "GoneAway",
            std::path::PathBuf::from("/nonexistent/robco/gone-away.ttf"),
        );
        assert!(entry.data().is_empty(), "the test's premise: no bytes");

        // The assertion is first that this returns at all.
        let context = FontContext::new(&entry);

        // And that what it stood up is the bundled default, not an empty
        // shell that will produce blank glyphs forever.
        let fallback = crate::fonts::font_by_name(FALLBACK_FACE).expect("the bundled fallback");
        assert_eq!(
            context.family, fallback.family,
            "the fallback face is not the one the atlas fell back to"
        );
    }
}
