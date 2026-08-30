//! This crate's own gradient-and-text painting, for the furniture that is
//! not a shader.
//!
//! [`crate::furniture`]'s doc drew the line this module crosses: the bank
//! furniture is two kinds of thing, and only one of them is a shader. The
//! plates, rails and channel displays are procedural metal passes and live
//! over there. The numerals, the window mouldings around each strip, the
//! pager, the selector carriage and the screw heads are rounded rectangles
//! with gradients, text items, and one canvas. This is the description of
//! them.
//!
//! It is a small, purpose-built painting vocabulary rather than a general
//! vector library: what these six pieces of furniture actually need is a
//! short list, and it is the whole of what is here:
//!
//! * a rounded rectangle, filled with a solid colour, a linear gradient
//!   (vertical by default, horizontal where a moulding calls for it) or, in
//!   the screw head's canvas, a two-circle radial gradient, at an item
//!   opacity, optionally with a border, optionally rotated about a point,
//!   optionally clipped to a parent's rounded rectangle;
//! * an arc stroked at a line width between two angles (the screw's rim);
//! * a filled path (the pagers' arrows);
//! * a line of text, aligned in a box ([`term::fonts::text`]).
//!
//! # Which pixels, and why a description rather than an image
//!
//! Every measure here is in the furniture's own coordinates (**logical**
//! pixels), and the crate draws nothing and owns no device. What crosses to
//! the host is a [`Painting`]: the description. The host draws each [`Op`]
//! as one instance of `app::chrome`'s pass, in the order they are listed, at
//! the window's own ratio, so a 2 px moulding lip is two logical pixels
//! across four device ones on a 2x display and the antialiasing lands on the
//! device grid. The shape arithmetic is `shaders/wgsl/vector.wgsl`, which is
//! the same list of ops in the same order under a signed distance field.
//!
//! # The one raster left
//!
//! Text is struck on the CPU, because a glyph outline is not a shape this
//! vocabulary can name: [`text_raster`] runs the swash pipeline
//! ([`term::fonts::text`]) at the device size and hands back a coverage mask
//! for the host to pack into its atlas. The mask is keyed on the run rather
//! than on the painting it appears in, so a piece that moves keeps its
//! numerals rather than striking them again.
//!
//! Coverage arrives as three subpixel channels where the machine's
//! fontconfig asks for them, and is folded to one number, the largest of the
//! three: a pixel is covered wherever any of its stripes is. That is the
//! alpha a line composites on.

use crate::color::Rgba;
use crate::furniture::Raster;
use crate::layout::Rect;

pub use term::fonts::text::Align;

/// One stop of a gradient.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stop {
    pub position: f64,
    pub color: Rgba,
}

impl Stop {
    pub const fn new(position: f64, color: Rgba) -> Self {
        Self { position, color }
    }
}

/// What fills a shape.
#[derive(Clone, Debug, PartialEq)]
pub enum Fill {
    Solid(Rgba),
    /// A linear gradient: stops from the item's top edge to its bottom, or
    /// from its left to its right where the fill is horizontal.
    Linear {
        horizontal: bool,
        stops: Vec<Stop>,
    },
    /// A two-circle radial gradient, which the screw head's dome needs
    /// because its focus is offset toward the light.
    Radial {
        from: (f64, f64, f64),
        to: (f64, f64, f64),
        stops: Vec<Stop>,
    },
}

/// A filled rectangle, optionally rotated.
#[derive(Clone, Debug, PartialEq)]
pub struct RectOp {
    pub rect: Rect,
    pub radius: f64,
    pub fill: Fill,
    pub opacity: f32,
    /// Width and colour, drawn inside the rectangle's own bounds.
    pub border: Option<(f64, Rgba)>,
    /// Radians, about the given pivot.
    pub rotation: Option<(f64, (f64, f64))>,
    /// A parent's clip rectangle, as `(rect, radius)`.
    pub clip: Option<(Rect, f64)>,
}

impl RectOp {
    /// A solid fill at full opacity, no border, no rotation, no clip: the
    /// common case.
    pub fn solid(rect: Rect, radius: f64, color: Rgba) -> Self {
        Self {
            rect,
            radius,
            fill: Fill::Solid(color),
            opacity: 1.0,
            border: None,
            rotation: None,
            clip: None,
        }
    }

    pub fn gradient(rect: Rect, radius: f64, stops: Vec<Stop>) -> Self {
        Self {
            fill: Fill::Linear {
                horizontal: false,
                stops,
            },
            ..Self::solid(rect, radius, Rgba::new(0.0, 0.0, 0.0, 1.0))
        }
    }

    pub fn horizontal_gradient(rect: Rect, radius: f64, stops: Vec<Stop>) -> Self {
        Self {
            fill: Fill::Linear {
                horizontal: true,
                stops,
            },
            ..Self::solid(rect, radius, Rgba::new(0.0, 0.0, 0.0, 1.0))
        }
    }

    pub fn at_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    pub fn clipped_to(mut self, rect: Rect, radius: f64) -> Self {
        self.clip = Some((rect, radius));
        self
    }

    pub fn rotated(mut self, radians: f64, pivot: (f64, f64)) -> Self {
        self.rotation = Some((radians, pivot));
        self
    }
}

/// A stroked arc, butt caps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArcOp {
    pub center: (f64, f64),
    pub radius: f64,
    pub line_width: f64,
    /// Radians, canvas convention (y down, angles increasing clockwise on
    /// screen).
    pub start: f64,
    pub end: f64,
    pub color: Rgba,
}

/// Which face a line of text is set in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    /// A catalogue key, e.g. `"IOSEVKA"`. The catalogue is the bundled entry
    /// (`term::fonts`'s `IOSEVKA`, `IosevkaTermNerdFontMono-Regular.ttf`)
    /// that owns the name, not the system-font path: a machine with Iosevka
    /// installed is excluded from the system list precisely because the
    /// bundle already occupies the family.
    Catalogue(&'static str),
    /// The application's own default face: see
    /// [`term::fonts::system::default_sans`]. Which face that is depends on
    /// whether the profile lets the cabinet letter itself off the machine;
    /// a bundled profile gets the bundled numeral face here too.
    Sans,
    /// The generic serif face, which the switchboard's counter rolls name
    /// and nothing else in the three shells does.
    /// [`term::fonts::system::default_serif`] resolves it here.
    Serif,
}

/// One line of text, in a box.
#[derive(Clone, Debug, PartialEq)]
pub struct TextOp {
    pub face: Face,
    /// The item's position: `x`, `y`, and the `width` the alignment measures
    /// against. The height is the line's own.
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub align: Align,
    pub pixel_size: f64,
    pub letter_spacing: f64,
    pub bold: bool,
    pub text: String,
    pub color: Rgba,
    pub opacity: f32,
}

/// A filled path, closed: `Context2D.beginPath` + `lineTo`s + `fill`.
///
/// Only the two shells' pager arrows need one, and both are convex; the fill
/// is nonzero-winding all the same, which is the canvas default.
#[derive(Clone, Debug, PartialEq)]
pub struct PolygonOp {
    pub points: Vec<(f64, f64)>,
    pub color: Rgba,
    pub opacity: f32,
}

/// One drawing operation.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    Rect(RectOp),
    Arc(ArcOp),
    Text(TextOp),
    Polygon(PolygonOp),
}

/// A piece of painted furniture, described rather than drawn.
///
/// The ops are in the order to paint them.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Painting {
    pub ops: Vec<Op>,
}

impl Painting {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn rect(&mut self, op: RectOp) -> &mut Self {
        self.ops.push(Op::Rect(op));
        self
    }

    pub fn arc(&mut self, op: ArcOp) -> &mut Self {
        self.ops.push(Op::Arc(op));
        self
    }

    pub fn text(&mut self, op: TextOp) -> &mut Self {
        self.ops.push(Op::Text(op));
        self
    }

    pub fn polygon(&mut self, op: PolygonOp) -> &mut Self {
        self.ops.push(Op::Polygon(op));
        self
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// The same painting moved, for a caller placing it in a rectangle bigger
    /// than the item it was written against.
    ///
    /// One caller: a shell whose row furniture paints past the row band gets a
    /// piece rectangle grown at the top, and the painting slides down into it
    /// (`crate::shells::row_overhang`). A no-op at `(0, 0)`, which is the
    /// annunciator's case and therefore the measured one.
    pub fn translated(mut self, dx: f64, dy: f64) -> Self {
        if dx == 0.0 && dy == 0.0 {
            return self;
        }
        let shift_rect = |r: &mut Rect| {
            r.x += dx;
            r.y += dy;
        };
        for op in &mut self.ops {
            match op {
                Op::Rect(r) => {
                    shift_rect(&mut r.rect);
                    if let Some((clip, _)) = r.clip.as_mut() {
                        shift_rect(clip);
                    }
                    if let Some((_, pivot)) = r.rotation.as_mut() {
                        pivot.0 += dx;
                        pivot.1 += dy;
                    }
                    if let Fill::Radial { from, to, .. } = &mut r.fill {
                        from.0 += dx;
                        from.1 += dy;
                        to.0 += dx;
                        to.1 += dy;
                    }
                }
                Op::Arc(a) => {
                    a.center.0 += dx;
                    a.center.1 += dy;
                }
                Op::Text(t) => {
                    t.x += dx;
                    t.y += dy;
                }
                Op::Polygon(g) => {
                    for p in &mut g.points {
                        p.0 += dx;
                        p.1 += dy;
                    }
                }
            }
        }
        self
    }
}

/// The bytes a face's key resolves to, or `None` where the machine cannot
/// supply it.
fn face_data(face: Face) -> Option<&'static [u8]> {
    match face {
        // Bundled by name: the catalogue key a `Face` carries is a bundled
        // entry's (see [`Face::Catalogue`]), so nothing painted on the
        // cabinet reaches for the machine's fonts here.
        Face::Catalogue(name) => {
            term::fonts::font_by_name(name, term::fonts::FontSource::Bundled).map(|e| e.data())
        }
        Face::Sans => term::fonts::system::default_sans(),
        Face::Serif => term::fonts::system::default_serif(),
    }
}

/// Whether the machine can set a line in this face at all.
///
/// The crate has no logger, so it cannot say so itself; the mount asks this
/// once at startup and complains there. A `false` here means every [`TextOp`]
/// in that face paints nothing.
pub fn face_available(face: Face) -> bool {
    face_data(face).is_some_and(|d| !d.is_empty())
}

/// One line of text struck to a coverage mask at `scale` device pixels to
/// the logical pixel the op is measured in, with the mask's top-left corner
/// in the painting's own device pixels.
///
/// The mask is [`Raster`]'s `[a, a, a, a]` convention, one texel per device
/// pixel, so the host can pack it into the same atlas the display kits' own
/// rasters go in and read the coverage off any channel.
///
/// `None` where the machine cannot supply the face, which is a plate that
/// was never printed rather than a label in whatever else is installed: a
/// label in the wrong face reads as a parity defect nobody can place. The
/// crate carries no logger (see its `Cargo.toml`); the caller that does is
/// [`face_available`], which the mount checks once.
pub fn text_raster(op: &TextOp, scale: f64) -> Option<(Raster, i32, i32)> {
    if op.text.is_empty() {
        return None;
    }
    let data = face_data(op.face)?;
    let scale = if scale > 0.0 { scale } else { 1.0 };
    let pixel_size = (op.pixel_size * scale).round().max(1.0) as u32;
    // The layout runs in device pixels: the glyphs are rasterised at the size
    // they will occupy, and the alignment is measured against the box at the
    // same scale, so the line's right edge lands on the box's right edge on
    // any display.
    let spec = term::fonts::text::TextSpec {
        data,
        pixel_size,
        text: &op.text,
        letter_spacing: op.letter_spacing * scale,
        bold: op.bold,
    };
    // The one place the machine's own configuration reaches the picture.
    //
    // This crate asks fontconfig whether the screen has subpixel stripes
    // before it decides how to antialias a line of text
    // (`term::fonts::subpixel` has the whole of it). So this raster is not
    // the same raster on two machines: a host with `rgba=rgb` set gets a
    // stripe-sampled mask, a host with nothing set gets a grey one. A parity
    // measurement taken on one machine says nothing about another -- that is
    // inherent to reading the host's own font configuration, not a defect.
    //
    // Read once per process: the answer cannot change under a running
    // application, since fontconfig is loaded at startup and a live edit
    // would need a restart to be seen.
    let raster = term::fonts::text::text_image(&spec, term::fonts::subpixel::host_layout())?;
    let natural = term::fonts::text::natural_width(&spec);
    let x = (op.x * scale + term::fonts::text::align_offset(op.align, op.width * scale, natural))
        .round() as i32;
    let y = (op.y * scale).round() as i32;

    let count = (raster.width * raster.height) as usize;
    let mut rgba = Vec::with_capacity(count * 4);
    for i in 0..count {
        let c = raster.coverage.channels(i);
        let a = c[0].max(c[1]).max(c[2]);
        rgba.extend_from_slice(&[a, a, a, a]);
    }
    Some((
        Raster {
            width: raster.width,
            height: raster.height,
            rgba: rgba.into(),
        },
        x,
        y,
    ))
}
