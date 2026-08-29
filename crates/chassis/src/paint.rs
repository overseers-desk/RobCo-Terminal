//! This crate's own gradient-and-text painting, for the furniture that is
//! not a shader.
//!
//! [`crate::furniture`]'s doc drew the line this module crosses: the bank
//! furniture is two kinds of thing, and only one of them is a shader. The
//! plates, rails and channel displays are procedural metal passes and live
//! over there. The numerals, the window mouldings around each strip, the
//! pager, the selector carriage and the screw heads are rounded rectangles
//! with gradients, text items, and one canvas. This is the small painting
//! stack they need.
//!
//! It is a small, purpose-built painting stack rather than a general vector
//! library: what these six pieces of furniture actually need is a short
//! list, and it is the whole of what is here:
//!
//! * a rounded rectangle, filled with a solid colour, a linear gradient
//!   (vertical by default, horizontal where a moulding calls for it) or, in
//!   the screw head's canvas, a two-circle radial gradient, at an item
//!   opacity, optionally with a border, optionally rotated about a point,
//!   optionally clipped to a parent's rounded rectangle;
//! * an arc stroked at a line width between two angles (the screw's rim);
//! * a line of text, aligned in a box ([`term::fonts::text`]).
//!
//! # Which pixels, and why a description rather than an image
//!
//! Every measure here is in the furniture's own coordinates (**logical**
//! pixels), and the crate draws nothing and owns no device. What crosses to
//! the host is a [`Painting`]: the description. The host rasterises it at
//! the window's own ratio ([`rasterize`]), so a 2 px moulding lip is two
//! logical pixels drawn across four device ones on a 2x display and the
//! antialiasing lands on the device grid. Handing over an image rasterised
//! at logical size and letting the blit scale it would give a scaled-up
//! picture of the wrong thing.
//!
//! A [`Painting`] is compared for equality rather than hashed, and that is
//! load-bearing at the mount: the plan is rebuilt every frame and is almost
//! always the same plan, so the host keeps the raster it made last time and
//! re-rasterises only what changed. Twelve rows of numerals are not worth
//! rendering sixty times a second.
//!
//! # Coverage
//!
//! Antialiasing is a signed distance field, not a supersample: the exact
//! distance to a rounded rectangle's boundary, taken to coverage as
//! `clamp(0.5 - d, 0, 1)` on the **device** grid. For the axis-aligned edges
//! that most of this furniture is made of that is exact, and for the rounded
//! corners it is the standard smooth-rectangle SDF computation. The output
//! is premultiplied RGBA8, which is what the mount's blit blends
//! (`app::chrome`'s premultiplied blend).

use crate::color::Rgba;
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

/// A premultiplied RGBA8 image, ready for the mount's blit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Painted {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Rasterise a painting into `size` device pixels, with `scale` device pixels
/// to the logical pixel the painting is measured in.
pub fn rasterize(painting: &Painting, size: (u32, u32), scale: f64) -> Painted {
    let (w, h) = (size.0.max(1), size.1.max(1));
    let scale = if scale > 0.0 { scale } else { 1.0 };
    // Premultiplied, straight into the accumulator: every composite below is
    // source-over on premultiplied colour, which is what the blit expects to
    // receive and what makes a stack of half-transparent lips add up
    // correctly.
    let mut buf = vec![[0f32; 4]; (w as usize) * (h as usize)];
    for op in &painting.ops {
        match op {
            Op::Rect(r) => paint_rect(&mut buf, w, h, scale, r),
            Op::Arc(a) => paint_arc(&mut buf, w, h, scale, a),
            Op::Text(t) => paint_text(&mut buf, w, h, scale, t),
            Op::Polygon(g) => paint_polygon(&mut buf, w, h, scale, g),
        }
    }
    let mut rgba = Vec::with_capacity(buf.len() * 4);
    for px in &buf {
        for c in px {
            rgba.push((c.clamp(0.0, 1.0) * 255.0).round() as u8);
        }
    }
    Painted {
        width: w,
        height: h,
        rgba,
    }
}

/// Source-over of a premultiplied sample onto the accumulator.
fn over(dst: &mut [f32; 4], color: Rgba, alpha: f32) {
    if alpha <= 0.0 {
        return;
    }
    let a = alpha.clamp(0.0, 1.0);
    let src = [color.r * a, color.g * a, color.b * a, a];
    let inv = 1.0 - a;
    for i in 0..4 {
        dst[i] = src[i] + dst[i] * inv;
    }
}

/// Source-over of a premultiplied sample whose coverage differs per colour
/// channel: component-alpha compositing, which is the whole of what subpixel
/// text is once the coverage has been sampled.
///
/// Each channel is blended against its own coverage. The pixel's alpha takes
/// the largest of the three, because the pixel is covered wherever any of its
/// stripes is: a numeral painted onto a transparent plate has to arrive at the
/// mount with an alpha that says "there is ink here", and picking any one
/// channel to speak for the three would make a red-fringed edge more or less
/// present than a blue-fringed one. Passing three equal coverages through this
/// is exactly [`over`], which is what the grey path gets.
fn over_stripes(dst: &mut [f32; 4], color: Rgba, alpha: [f32; 3]) {
    let a = alpha.map(|c| c.clamp(0.0, 1.0));
    let cover = a[0].max(a[1]).max(a[2]);
    if cover <= 0.0 {
        return;
    }
    let src = [color.r * a[0], color.g * a[1], color.b * a[2]];
    for i in 0..3 {
        dst[i] = src[i] + dst[i] * (1.0 - a[i]);
    }
    dst[3] = cover + dst[3] * (1.0 - cover);
}

/// The exact distance from a point to a rounded rectangle's boundary,
/// negative inside.
fn rrect_distance(px: f64, py: f64, rect: &Rect, radius: f64) -> f64 {
    let hx = rect.width / 2.0;
    let hy = rect.height / 2.0;
    let r = radius.min(hx).min(hy).max(0.0);
    let qx = (px - (rect.x + hx)).abs() - (hx - r);
    let qy = (py - (rect.y + hy)).abs() - (hy - r);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - r
}

fn coverage(distance: f64) -> f32 {
    (0.5 - distance).clamp(0.0, 1.0) as f32
}

fn scaled(rect: &Rect, scale: f64) -> Rect {
    Rect::new(
        rect.x * scale,
        rect.y * scale,
        rect.width * scale,
        rect.height * scale,
    )
}

/// The colour a fill gives at a device-space point, in the shape's own
/// device-space rectangle.
fn sample(fill: &Fill, rect: &Rect, px: f64, py: f64) -> Rgba {
    match fill {
        Fill::Solid(c) => *c,
        Fill::Linear { horizontal, stops } => {
            let t = if *horizontal {
                if rect.width > 0.0 {
                    (px - rect.x) / rect.width
                } else {
                    0.0
                }
            } else if rect.height > 0.0 {
                (py - rect.y) / rect.height
            } else {
                0.0
            };
            stop_color(stops, t)
        }
        Fill::Radial { from, to, stops } => stop_color(stops, radial_t(*from, *to, px, py)),
    }
}

/// The canvas two-circle radial gradient's parameter at a point: the largest
/// `t` whose interpolated circle passes through it, padded outside `0..1`.
fn radial_t(from: (f64, f64, f64), to: (f64, f64, f64), px: f64, py: f64) -> f64 {
    let (cdx, cdy, dr) = (to.0 - from.0, to.1 - from.1, to.2 - from.2);
    let (pdx, pdy) = (px - from.0, py - from.1);
    let a = cdx * cdx + cdy * cdy - dr * dr;
    let b = pdx * cdx + pdy * cdy + from.2 * dr;
    let c = pdx * pdx + pdy * pdy - from.2 * from.2;
    if a.abs() < 1e-12 {
        if b.abs() < 1e-12 {
            return 0.0;
        }
        return c / (2.0 * b);
    }
    let disc = b * b - a * c;
    if disc < 0.0 {
        return 0.0;
    }
    // Both roots, and the greater of the two that names a circle with a real
    // radius. Which of `(b ± sqrt) / a` that is depends on the sign of `a`,
    // and `a` is negative for every concentric gradient (the two centres
    // coincide, so `a` is `-dr²`), which is all but one of the screw head's.
    let root = disc.sqrt();
    let valid = |t: f64| from.2 + t * dr >= 0.0;
    let (hi, lo) = {
        let (p, q) = ((b + root) / a, (b - root) / a);
        if p >= q {
            (p, q)
        } else {
            (q, p)
        }
    };
    if valid(hi) {
        hi
    } else if valid(lo) {
        lo
    } else {
        0.0
    }
}

/// A gradient's colour at `t`, with a pad spread at both ends and straight
/// interpolation between neighbouring stops.
fn stop_color(stops: &[Stop], t: f64) -> Rgba {
    if stops.is_empty() {
        return Rgba::new(0.0, 0.0, 0.0, 0.0);
    }
    if t <= stops[0].position {
        return stops[0].color;
    }
    let last = &stops[stops.len() - 1];
    if t >= last.position {
        return last.color;
    }
    for pair in stops.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if t >= a.position && t <= b.position {
            let span = b.position - a.position;
            let f = if span > 0.0 {
                ((t - a.position) / span) as f32
            } else {
                0.0
            };
            return Rgba::new(
                a.color.r + (b.color.r - a.color.r) * f,
                a.color.g + (b.color.g - a.color.g) * f,
                a.color.b + (b.color.b - a.color.b) * f,
                a.color.a + (b.color.a - a.color.a) * f,
            );
        }
    }
    last.color
}

/// The device-space box a shape can touch, clamped to the image.
fn bounds(rect: &Rect, pad: f64, w: u32, h: u32) -> (u32, u32, u32, u32) {
    let x0 = ((rect.x - pad).floor().max(0.0)) as u32;
    let y0 = ((rect.y - pad).floor().max(0.0)) as u32;
    let x1 = (((rect.x + rect.width + pad).ceil()).max(0.0) as u32).min(w);
    let y1 = (((rect.y + rect.height + pad).ceil()).max(0.0) as u32).min(h);
    (x0, y0, x1.max(x0), y1.max(y0))
}

fn paint_rect(buf: &mut [[f32; 4]], w: u32, h: u32, scale: f64, op: &RectOp) {
    if op.opacity <= 0.0 || op.rect.width <= 0.0 || op.rect.height <= 0.0 {
        return;
    }
    let rect = scaled(&op.rect, scale);
    let radius = op.radius * scale;
    let clip = op
        .clip
        .as_ref()
        .map(|(r, rad)| (scaled(r, scale), rad * scale));
    let border = op.border.map(|(bw, c)| (bw * scale, c));
    let rotation = op
        .rotation
        .map(|(a, (px, py))| (a, (px * scale, py * scale)));
    // A radial gradient's two circles are authored in the same logical pixels
    // as the rectangle, but `sample` compares them against `px`/`py`, which
    // walk device pixels. Every other measure above is scaled here for that
    // reason and this one was not, so at any scale but 1.0 the gradient was
    // evaluated at the wrong distance from the wrong centre: at DPR 2 the
    // circles stayed put in the top-left quarter while the sample point
    // covered the whole rectangle, running off the end of `to`'s radius.
    //
    // Scaled once here rather than inside `sample`, which runs per pixel. The
    // other fills need nothing: a solid has no geometry, and a linear one is
    // measured against `rect`, which is already scaled.
    let scaled_radial = match &op.fill {
        Fill::Radial { from, to, stops } => Some(Fill::Radial {
            from: (from.0 * scale, from.1 * scale, from.2 * scale),
            to: (to.0 * scale, to.1 * scale, to.2 * scale),
            stops: stops.clone(),
        }),
        Fill::Solid(_) | Fill::Linear { .. } => None,
    };
    let fill = scaled_radial.as_ref().unwrap_or(&op.fill);

    // A rotated rectangle sweeps a wider box; its half-diagonal covers it.
    let pad = 1.0
        + rotation
            .map(|_| (rect.width * rect.width + rect.height * rect.height).sqrt())
            .unwrap_or(0.0);
    let touched = match &clip {
        // Clipped: nothing outside the clip can be painted, so the smaller of
        // the two boxes is the one to walk.
        Some((c, _)) => {
            let a = bounds(&rect, pad, w, h);
            let b = bounds(c, 1.0, w, h);
            (a.0.max(b.0), a.1.max(b.1), a.2.min(b.2), a.3.min(b.3))
        }
        None => bounds(&rect, pad, w, h),
    };
    let (x0, y0, x1, y1) = touched;

    for y in y0..y1 {
        for x in x0..x1 {
            let (mut px, mut py) = (x as f64 + 0.5, y as f64 + 0.5);
            // The parent's clip, evaluated in the parent's own (unrotated)
            // frame.
            let clip_cov = match &clip {
                Some((r, rad)) => coverage(rrect_distance(px, py, r, *rad)),
                None => 1.0,
            };
            if clip_cov <= 0.0 {
                continue;
            }
            if let Some((angle, (cx, cy))) = rotation {
                let (s, c) = (-angle).sin_cos();
                let (dx, dy) = (px - cx, py - cy);
                px = cx + dx * c - dy * s;
                py = cy + dx * s + dy * c;
            }
            let d = rrect_distance(px, py, &rect, radius);
            let cov = coverage(d) * clip_cov;
            if cov <= 0.0 {
                continue;
            }
            let slot = &mut buf[(y * w + x) as usize];
            // The fill first, over the whole rectangle: the border paints
            // *inside* the bounds and on top, so the fill runs underneath it.
            let colour = sample(fill, &rect, px, py);
            over(slot, colour, cov * op.opacity * colour.a);
            if let Some((bw, bc)) = border {
                if bw > 0.0 {
                    let inner = Rect::new(
                        rect.x + bw,
                        rect.y + bw,
                        (rect.width - 2.0 * bw).max(0.0),
                        (rect.height - 2.0 * bw).max(0.0),
                    );
                    let ring = (cov
                        - coverage(rrect_distance(px, py, &inner, radius - bw)) * clip_cov)
                        .max(0.0);
                    over(slot, bc, ring * op.opacity * bc.a);
                }
            }
        }
    }
}

fn paint_arc(buf: &mut [[f32; 4]], w: u32, h: u32, scale: f64, op: &ArcOp) {
    let (cx, cy) = (op.center.0 * scale, op.center.1 * scale);
    let radius = op.radius * scale;
    let half = (op.line_width * scale) / 2.0;
    let reach = radius + half + 1.0;
    let box_ = Rect::new(cx - reach, cy - reach, 2.0 * reach, 2.0 * reach);
    let (x0, y0, x1, y1) = bounds(&box_, 0.0, w, h);
    for y in y0..y1 {
        for x in x0..x1 {
            let (dx, dy) = (x as f64 + 0.5 - cx, y as f64 + 0.5 - cy);
            let dist = (dx * dx + dy * dy).sqrt();
            let cov = coverage((dist - radius).abs() - half);
            if cov <= 0.0 {
                continue;
            }
            // Butt caps: the sweep is a hard gate, as a stroke's ends are.
            let mut angle = dy.atan2(dx);
            let sweep = op.end - op.start;
            let mut from_start = angle - op.start;
            let two_pi = std::f64::consts::TAU;
            from_start = from_start.rem_euclid(two_pi);
            if sweep < 0.0 {
                angle = -angle;
                from_start = two_pi - from_start;
            }
            if from_start > sweep.abs() {
                continue;
            }
            over(&mut buf[(y * w + x) as usize], op.color, cov * op.color.a);
        }
    }
}

/// A filled path. Straight edges have no closed-form distance the way a
/// rounded rectangle does, so this one is supersampled: a 4x4 grid inside each
/// touched pixel, nonzero winding at each sample. Sixteen point-in-polygon
/// tests over a fifty-pixel arrow is nothing, and it happens once per change.
fn paint_polygon(buf: &mut [[f32; 4]], w: u32, h: u32, scale: f64, op: &PolygonOp) {
    if op.points.len() < 3 || op.opacity <= 0.0 {
        return;
    }
    let pts: Vec<(f64, f64)> = op
        .points
        .iter()
        .map(|(x, y)| (x * scale, y * scale))
        .collect();
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (x, y) in &pts {
        x0 = x0.min(*x);
        y0 = y0.min(*y);
        x1 = x1.max(*x);
        y1 = y1.max(*y);
    }
    let bx0 = (x0.floor().max(0.0)) as u32;
    let by0 = (y0.floor().max(0.0)) as u32;
    let bx1 = ((x1.ceil().max(0.0)) as u32).min(w);
    let by1 = ((y1.ceil().max(0.0)) as u32).min(h);

    const N: i32 = 4;
    for y in by0..by1 {
        for x in bx0..bx1 {
            let mut hits = 0;
            for sy in 0..N {
                for sx in 0..N {
                    let px = x as f64 + (sx as f64 + 0.5) / N as f64;
                    let py = y as f64 + (sy as f64 + 0.5) / N as f64;
                    if winding(&pts, px, py) != 0 {
                        hits += 1;
                    }
                }
            }
            if hits == 0 {
                continue;
            }
            let cov = hits as f32 / (N * N) as f32;
            over(
                &mut buf[(y * w + x) as usize],
                op.color,
                cov * op.opacity * op.color.a,
            );
        }
    }
}

/// The nonzero winding number of a closed path around a point.
fn winding(pts: &[(f64, f64)], px: f64, py: f64) -> i32 {
    let mut n = 0;
    for i in 0..pts.len() {
        let (ax, ay) = pts[i];
        let (bx, by) = pts[(i + 1) % pts.len()];
        if ay <= py {
            if by > py && (bx - ax) * (py - ay) - (px - ax) * (by - ay) > 0.0 {
                n += 1;
            }
        } else if by <= py && (bx - ax) * (py - ay) - (px - ax) * (by - ay) < 0.0 {
            n -= 1;
        }
    }
    n
}

fn paint_text(buf: &mut [[f32; 4]], w: u32, h: u32, scale: f64, op: &TextOp) {
    if op.opacity <= 0.0 || op.text.is_empty() {
        return;
    }
    let Some(data) = face_data(op.face) else {
        // No face, no label. Painting the words in whatever else the machine
        // happens to have is a worse answer than leaving the plate bare: a
        // missing label reads as a plate that was never printed, where a
        // label in the wrong face reads as a parity defect nobody can place.
        // The crate carries no logger (see its `Cargo.toml`); the caller that
        // does is [`face_available`], which the mount checks once.
        return;
    };
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
    // (`term::fonts::subpixel` has the whole of it). So this painting is not
    // the same painting on two machines: a host with `rgba=rgb` set gets
    // colour-filtered numerals, a host with nothing set gets grey ones. A
    // parity measurement taken on one machine says nothing about another --
    // that is inherent to reading the host's own font configuration, not a
    // defect.
    //
    // Read once per process, so it stays out of the painting-cache key: the
    // answer cannot change under a running application -- fontconfig is
    // loaded at startup and a live edit would need a restart to be seen.
    let Some(raster) = term::fonts::text::text_image(&spec, term::fonts::subpixel::host_layout())
    else {
        return;
    };
    let natural = term::fonts::text::natural_width(&spec);
    let x0 = (op.x * scale + term::fonts::text::align_offset(op.align, op.width * scale, natural))
        .round() as i32;
    let y0 = (op.y * scale).round() as i32;

    for ry in 0..raster.height as i32 {
        let y = y0 + ry;
        if y < 0 || y >= h as i32 {
            continue;
        }
        for rx in 0..raster.width as i32 {
            let x = x0 + rx;
            if x < 0 || x >= w as i32 {
                continue;
            }
            let a = raster
                .coverage
                .channels((ry as u32 * raster.width + rx as u32) as usize);
            if a == [0, 0, 0] {
                continue;
            }
            // Per channel, and in exactly the order the one-channel version
            // multiplied in: three equal coverages have to come out of here
            // bit for bit what a grey raster used to, since that is what a
            // host with no subpixel geometry still gets.
            let alpha = a.map(|c| (c as f32 / 255.0) * op.opacity * op.color.a);
            over_stripes(
                &mut buf[(y as u32 * w + x as u32) as usize],
                op.color,
                alpha,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::hex_literal_to_color;

    fn pixel(p: &Painted, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * p.width + x) * 4) as usize;
        [p.rgba[i], p.rgba[i + 1], p.rgba[i + 2], p.rgba[i + 3]]
    }

    /// A host with no subpixel geometry configured gets the same pixels it
    /// got before there was a subpixel path at all, and this is the join
    /// where that could quietly stop being true: three equal coverages
    /// through [`over_stripes`] must be [`over`] to the bit, not to four
    /// decimal places.
    #[test]
    fn three_equal_coverages_composite_exactly_as_one_did() {
        let color = Rgba::new(0.83, 0.71, 0.42, 0.9);
        for &a in &[0.0f32, 0.003, 0.25, 1.0 / 3.0, 0.517, 0.999, 1.0] {
            let mut one = [0.2f32, 0.11, 0.07, 0.31];
            let mut three = one;
            over(&mut one, color, a);
            over_stripes(&mut three, color, [a; 3]);
            assert_eq!(one, three, "a={a}");
        }
    }

    #[test]
    fn an_unpainted_pixel_is_transparent_so_the_casting_shows_through() {
        // The whole reason the output is premultiplied with a real alpha: a
        // piece of furniture is a shape on the plate, not a tile over it.
        let mut p = Painting::new();
        p.rect(RectOp::solid(
            Rect::new(2.0, 2.0, 4.0, 4.0),
            0.0,
            Rgba::new(1.0, 1.0, 1.0, 1.0),
        ));
        let out = rasterize(&p, (10, 10), 1.0);
        assert_eq!(pixel(&out, 0, 0), [0, 0, 0, 0]);
        assert_eq!(pixel(&out, 4, 4), [255, 255, 255, 255]);
    }

    #[test]
    fn a_vertical_gradient_runs_top_to_bottom_across_the_items_own_height() {
        // The default gradient orientation, and the one every moulding in
        // the three shells uses: position 0 is the item's top edge.
        let black = Rgba::new(0.0, 0.0, 0.0, 1.0);
        let white = Rgba::new(1.0, 1.0, 1.0, 1.0);
        let mut p = Painting::new();
        p.rect(RectOp::gradient(
            Rect::new(0.0, 0.0, 4.0, 100.0),
            0.0,
            vec![Stop::new(0.0, black), Stop::new(1.0, white)],
        ));
        let out = rasterize(&p, (4, 100), 1.0);
        assert_eq!(pixel(&out, 2, 0)[0], 1); // half a pixel in from the top
        assert_eq!(pixel(&out, 2, 99)[0], 254);
        // Monotone all the way down, which is what says the ramp is the
        // item's height and not the image's.
        for y in 1..100 {
            assert!(pixel(&out, 2, y)[0] >= pixel(&out, 2, y - 1)[0]);
        }
    }

    #[test]
    fn a_horizontal_gradient_runs_left_to_right_by_orientation() {
        let a = Rgba::new(0.0, 0.0, 0.0, 1.0);
        let b = Rgba::new(1.0, 1.0, 1.0, 1.0);
        let mut p = Painting::new();
        p.rect(RectOp::horizontal_gradient(
            Rect::new(0.0, 0.0, 100.0, 4.0),
            0.0,
            vec![Stop::new(0.0, a), Stop::new(1.0, b)],
        ));
        let out = rasterize(&p, (100, 4), 1.0);
        assert_eq!(pixel(&out, 0, 2)[0], 1);
        assert_eq!(pixel(&out, 99, 2)[0], 254);
        assert_eq!(pixel(&out, 50, 0)[0], pixel(&out, 50, 3)[0]);
    }

    #[test]
    fn a_radius_rounds_the_corner_and_antialiases_it() {
        let white = Rgba::new(1.0, 1.0, 1.0, 1.0);
        let mut p = Painting::new();
        p.rect(RectOp::solid(Rect::new(0.0, 0.0, 20.0, 20.0), 8.0, white));
        let out = rasterize(&p, (20, 20), 1.0);
        // The corner is cut away...
        assert_eq!(pixel(&out, 0, 0)[3], 0);
        // ...the middle is solid...
        assert_eq!(pixel(&out, 10, 10)[3], 255);
        // ...and the arc is a ramp rather than a staircase.
        let edge: Vec<u8> = (0..8).map(|i| pixel(&out, i, 7 - i)[3]).collect();
        assert!(
            edge.iter().any(|&a| a > 0 && a < 255),
            "no partial coverage on the corner arc: {edge:?}"
        );
    }

    #[test]
    fn the_scale_factor_moves_the_geometry_and_not_the_uniforms() {
        // The same painting at 2x is the same picture on twice the grid: a
        // 2px lip is four device pixels tall, and its colour is unchanged.
        let c = hex_literal_to_color("#7a6448");
        let mut p = Painting::new();
        p.rect(RectOp::solid(Rect::new(1.0, 1.0, 6.0, 2.0), 0.0, c));
        let one = rasterize(&p, (8, 4), 1.0);
        let two = rasterize(&p, (16, 8), 2.0);
        assert_eq!(pixel(&one, 3, 1), pixel(&two, 6, 2));
        assert_eq!(pixel(&one, 3, 2)[3], 255);
        assert_eq!(pixel(&two, 6, 5)[3], 255);
        assert_eq!(pixel(&two, 6, 6)[3], 0);
    }

    #[test]
    fn a_clip_cuts_a_child_at_its_parents_rounded_bounds() {
        let white = Rgba::new(1.0, 1.0, 1.0, 1.0);
        let mut p = Painting::new();
        p.rect(
            RectOp::solid(Rect::new(0.0, 0.0, 20.0, 20.0), 0.0, white)
                .clipped_to(Rect::new(5.0, 5.0, 10.0, 10.0), 0.0),
        );
        let out = rasterize(&p, (20, 20), 1.0);
        assert_eq!(pixel(&out, 10, 10)[3], 255);
        assert_eq!(pixel(&out, 2, 10)[3], 0);
        assert_eq!(pixel(&out, 10, 2)[3], 0);
    }

    #[test]
    fn text_lands_where_the_alignment_puts_it() {
        // Right-aligned in the numeral lane, so the ink ends at the lane's
        // right edge and the lane's left is bare.
        let ink = hex_literal_to_color("#9c8168");
        let mut p = Painting::new();
        p.text(TextOp {
            face: Face::Catalogue("IOSEVKA"),
            x: 0.0,
            y: 0.0,
            width: 46.0,
            align: Align::Right,
            pixel_size: 34.0,
            letter_spacing: -1.0,
            bold: false,
            text: "08".to_string(),
            color: ink,
            opacity: 1.0,
        });
        let out = rasterize(&p, (46, 40), 1.0);
        let column_ink = |x: u32| (0..40).map(|y| pixel(&out, x, y)[3] as u32).sum::<u32>();
        assert!(column_ink(2) == 0, "the lane's left margin is not bare");
        assert!(
            (36..46).map(column_ink).sum::<u32>() > 0,
            "no ink at the lane's right edge"
        );
    }

    #[test]
    fn a_missing_face_paints_nothing_rather_than_the_wrong_face() {
        let mut p = Painting::new();
        p.text(TextOp {
            face: Face::Catalogue("NO SUCH FACE"),
            x: 0.0,
            y: 0.0,
            width: 46.0,
            align: Align::Right,
            pixel_size: 34.0,
            letter_spacing: 0.0,
            bold: false,
            text: "08".to_string(),
            color: Rgba::new(1.0, 1.0, 1.0, 1.0),
            opacity: 1.0,
        });
        let out = rasterize(&p, (46, 40), 1.0);
        assert!(out.rgba.iter().all(|&b| b == 0));
    }

    #[test]
    fn a_stroked_arc_covers_its_sweep_and_stops_at_the_cap() {
        // The screw head's lit rim: a band on the light's side and nothing
        // opposite it.
        let mut p = Painting::new();
        p.arc(ArcOp {
            center: (20.0, 20.0),
            radius: 10.0,
            line_width: 4.0,
            start: -0.6,
            end: 0.6,
            color: Rgba::new(1.0, 1.0, 1.0, 1.0),
        });
        let out = rasterize(&p, (40, 40), 1.0);
        // Straight right of the centre is inside the sweep...
        assert!(pixel(&out, 30, 20)[3] > 200);
        // ...straight left is not, and neither is the disc's middle.
        assert_eq!(pixel(&out, 10, 20)[3], 0);
        assert_eq!(pixel(&out, 20, 20)[3], 0);
    }

    #[test]
    fn a_concentric_radial_gradient_runs_from_the_inner_circle_out() {
        let a = Rgba::new(0.0, 0.0, 0.0, 0.0);
        let b = Rgba::new(0.0, 0.0, 0.0, 1.0);
        let mut p = Painting::new();
        p.rect(RectOp {
            fill: Fill::Radial {
                from: (20.0, 20.0, 5.0),
                to: (20.0, 20.0, 20.0),
                stops: vec![Stop::new(0.0, a), Stop::new(1.0, b)],
            },
            ..RectOp::solid(Rect::new(0.0, 0.0, 40.0, 40.0), 20.0, b)
        });
        let out = rasterize(&p, (40, 40), 1.0);
        // Inside the inner circle the first stop pads out: fully transparent.
        assert_eq!(pixel(&out, 20, 20)[3], 0);
        // Halfway out, halfway along the ramp.
        let mid = pixel(&out, 20, 20 - 12)[3];
        assert!(mid > 80 && mid < 200, "{mid}");
    }

    /// The same gradient on a 2x display is the same gradient.
    ///
    /// `sample` walks device pixels, so a radial's two circles have to be
    /// scaled with the rest of the geometry. Unscaled they stayed in the
    /// top-left quarter of the rectangle while the sample point covered all
    /// of it, and every pixel past `to`'s radius padded out to the last stop:
    /// the screw head's dome came out flat on any HiDPI screen.
    #[test]
    fn a_radial_gradient_at_double_scale_samples_where_it_does_at_single() {
        let a = Rgba::new(0.0, 0.0, 0.0, 0.0);
        let b = Rgba::new(0.0, 0.0, 0.0, 1.0);
        let painting = || {
            let mut p = Painting::new();
            p.rect(RectOp {
                fill: Fill::Radial {
                    from: (20.0, 20.0, 5.0),
                    to: (20.0, 20.0, 20.0),
                    stops: vec![Stop::new(0.0, a), Stop::new(1.0, b)],
                },
                ..RectOp::solid(Rect::new(0.0, 0.0, 40.0, 40.0), 20.0, b)
            });
            p
        };
        let single = rasterize(&painting(), (40, 40), 1.0);
        let double = rasterize(&painting(), (80, 80), 2.0);

        // Corresponding points: a logical (x, y) is device (x, y) at 1x and
        // (2x, 2y) at 2x. The half-pixel offsets differ, so the two rasters
        // are not bit-identical and the comparison is by ramp value.
        for (x, y) in [(20, 20), (20, 8), (20, 14), (26, 20), (20, 32)] {
            let one = pixel(&single, x, y)[3] as i32;
            let two = pixel(&double, x * 2, y * 2)[3] as i32;
            assert!(
                (one - two).abs() <= 8,
                "logical ({x}, {y}): {one} at 1x but {two} at 2x"
            );
        }

        // Not vacuous: the points above really do span the ramp, so an
        // unscaled gradient (which pads to the last stop almost everywhere)
        // could not have matched them.
        let centre = pixel(&single, 20, 20)[3];
        let edge = pixel(&single, 20, 2)[3];
        assert_eq!(centre, 0, "the inner circle should be transparent");
        assert!(edge > 200, "the outer edge should be opaque: {edge}");
    }
}
