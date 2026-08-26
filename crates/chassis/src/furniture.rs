//! What stands on the casting: the shell's plate, and one channel display per
//! row.
//!
//! [`bank`] and [`layout`](crate::layout) divide the window and measure the
//! bank's footprint; this module is the next question, which is what goes
//! *inside* that footprint: one shell's plate and rows of `ChannelRow`s,
//! reduced to what a host with a device needs and nothing more: a rectangle,
//! the name of the pass to run over it, its uniforms by name, and, for the
//! two display kits, the glyph raster that pass samples.
//!
//! It draws nothing and owns no device, like the rest of the crate. The mount
//! is `app::column`.
//!
//! # What is here
//!
//! The bank furniture is two kinds of thing, and only one of them is a
//! shader. The plates, rails and channel displays are drawn by the three
//! procedural metals and the two display passes. The numerals (two stacked
//! text items in Iosevka at 34 px), the window mouldings around each strip
//! (gradient rectangles), the pager, the selector carriage, the screw heads
//! and the tape kit's well chrome are vector painting instead.
//!
//! Both kinds are here now, and they meet at [`Piece`]: a shaded piece names a
//! pass and carries its uniforms, a painted one carries a
//! [`crate::paint::Painting`] and names [`Pass::Painted`]. The mount treats
//! them the same way (scratch, then premultiplied blit), which is what
//! keeps the composition order fixed across the join, and it is the reason
//! the two kinds are one list rather than two.
//!
//! The one piece deliberately left out of the switchboard's row is the
//! lever's throw: the swing from rest to thrown, with its own easing
//! overshoot. The cap itself is painted: drop shadow, front face, chamfer
//! and lit top sliver over the well's left, alongside the riveted plate,
//! stamped numeral, well, glow and bevel. It is named here rather than left
//! to be discovered, because a swing is a mechanism, not a moulding.
//!
//! # The channel seam
//!
//! A strip's content is channel state, and the channel state machines live in
//! `app::{channels,bank}`. [`crate::BankStrips`] is the seam between them:
//! the whole of what the furniture asks the channel model, built by
//! `app::bank::BankPager::strips` from the real slots and read here. This
//! module reads channel state nowhere else, and holds none.
//!
//! This module shipped its own `ChannelView`/`ChannelSlot` pair while the
//! channel state machine was still being built elsewhere; the merge
//! collapsed the two shapes into [`crate::BankStrips`], which is the wider
//! of the two (it carries the page,
//! the indicator and where the selector rides as well as the row contents) and
//! the one the real model already produces.

use crate::bank::BankGeometry;
use crate::cabinet::Display;
use crate::color::{self, Rgba};
use crate::displays::{led, raster, tape};
use crate::frame::Param;
use crate::layout::Rect;
use crate::metrics::{LedMetrics, ShellMetrics, TapeMetrics};
use crate::params::PlateMetalParams;
use crate::strip::{BankStrips, StripRow};

use config::Config;

/// Which pass draws a piece. The first three bodies are
/// [`crate::shaders`]'s constants of the same names; the fourth is not a pass
/// at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pass {
    /// `plate_metal.slang`.
    Plate,
    /// `led_matrix.slang`.
    LedMatrix,
    /// `tape_label.slang`.
    TapeLabel,
    /// Not a shader: a [`crate::paint::Painting`], rasterised by the mount at
    /// the window's own ratio and blitted like the rest. This is the vector
    /// half (the numerals, the window mouldings, the pager, the selector
    /// carriage, the screw heads and the tape kit's well chrome), and the
    /// piece carries its description in [`Piece::paint`] rather than uniforms
    /// in [`Piece::params`].
    Painted,
}

/// A glyph raster, widened to the RGBA8 a texture upload wants
/// ([`crate::displays::raster::to_rgba8`]'s convention: `[a, a, a, a]`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// One thing to draw on the casting.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub pass: Pass,
    /// Where it goes, in the bank column's own coordinates, logical pixels
    /// (see [`crate::cabinet`]'s module doc on which pixel). A host on a
    /// scaled display multiplies by the window's factor.
    pub rect: Rect,
    /// The pass's uniforms, by the names its `#pragma parameter` lines
    /// declare.
    pub params: Vec<Param>,
    /// The texture the pass samples as `Source`, for the two display passes.
    /// `None` for the plate, which is procedural and samples nothing.
    pub source: Option<Raster>,
    /// What to paint, for a [`Pass::Painted`] piece and nothing else.
    ///
    /// It is a description and not an image on purpose: the mount rasterises
    /// it at the window's ratio, so the antialiasing lands on the device grid
    /// (see [`crate::paint`]'s module doc), and it re-rasterises only when the
    /// description changes, which on a bank standing still is never.
    pub paint: Option<crate::paint::Painting>,
}

impl Piece {
    /// A piece drawn by one of the three procedural passes.
    pub fn shaded(pass: Pass, rect: Rect, params: Vec<Param>, source: Option<Raster>) -> Self {
        Self {
            pass,
            rect,
            params,
            source,
            paint: None,
        }
    }

    /// A piece of painted furniture, at its rectangle in the bank column's
    /// own coordinates.
    pub fn painted(rect: Rect, painting: crate::paint::Painting) -> Self {
        Self {
            pass: Pass::Painted,
            rect,
            params: Vec::new(),
            source: None,
            paint: Some(painting),
        }
    }
}

/// The derived font colour, which is what the LED strip's lamps are struck
/// from, not the stored hex.
///
/// `crt::params`'s `screen_colors` is the same derivation for the glass, and
/// its doc carries the reasoning in full (the /256 parser, the `saturation`
/// mix, the `0.7 + contrast * 0.3` weight, and why pushing the stored string
/// instead leaves contrast moving nothing). This is that arithmetic's chrome
/// half, duplicated here for the same reason [`crate::color`] carries its
/// own copy of two colour-mixing functions the glass chain also needs: the
/// chassis does not import the glass chain.
pub fn font_color(cfg: &Config) -> Rgba {
    let saturated = color::mix(
        color::str_to_color(&cfg.screen.font_color),
        color::str_to_color("#FFFFFF"),
        cfg.screen.saturation_color as f32 * 0.5,
    );
    let background = color::str_to_color(&cfg.screen.background_color);
    color::mix(
        background,
        saturated,
        0.7 + cfg.screen.contrast as f32 * 0.3,
    )
}

/// `plate_metal.slang`'s uniforms, by name.
pub fn plate_params(p: &PlateMetalParams) -> Vec<Param> {
    vec![
        ("sizePxX", p.size_px[0]),
        ("sizePxY", p.size_px[1]),
        ("lightDirX", p.light_dir[0]),
        ("lightDirY", p.light_dir[1]),
        ("baseColorR", p.base_color[0]),
        ("baseColorG", p.base_color[1]),
        ("baseColorB", p.base_color[2]),
        ("baseColorA", 1.0),
        ("highlightColorR", p.highlight_color[0]),
        ("highlightColorG", p.highlight_color[1]),
        ("highlightColorB", p.highlight_color[2]),
        ("highlightColorA", 1.0),
        ("shadowColorR", p.shadow_color[0]),
        ("shadowColorG", p.shadow_color[1]),
        ("shadowColorB", p.shadow_color[2]),
        ("shadowColorA", 1.0),
        ("cornerRadius", p.corner_radius),
        ("bevelPx", p.bevel_px),
        ("grainAmount", p.metal.grain_amount),
        ("mottleAmount", p.metal.mottle_amount),
        ("scratchAmount", p.metal.scratch_amount),
        ("vignetteStrength", p.vignette_strength),
        ("wearAmount", p.wear_amount),
        ("seamGain", p.seam_gain),
        ("seed", p.seed),
    ]
}

/// `led_matrix.slang`'s uniforms, by name, for one window.
///
/// `spill` is the margin in pixels the drawn rectangle stands proud of the
/// strip on each side; the shader wants it as a fraction of that grown
/// rectangle, which is why this takes the grown size too.
#[allow(clippy::too_many_arguments)]
pub fn led_params(
    grid: (u32, u32),
    colors: led::Colors,
    glow: f32,
    spill_strength: f32,
    spill: (f64, f64),
    grown: (f64, f64),
) -> Vec<Param> {
    let frac = |margin: f64, size: f64| {
        if size > 0.0 {
            (margin / size) as f32
        } else {
            0.0
        }
    };
    vec![
        ("gridSizeX", grid.0 as f32),
        ("gridSizeY", grid.1 as f32),
        ("litColorR", colors.lit.r),
        ("litColorG", colors.lit.g),
        ("litColorB", colors.lit.b),
        ("litColorA", colors.lit.a),
        ("dimColorR", colors.dim.r),
        ("dimColorG", colors.dim.g),
        ("dimColorB", colors.dim.b),
        ("dimColorA", colors.dim.a),
        ("panelColorR", colors.panel.r),
        ("panelColorG", colors.panel.g),
        ("panelColorB", colors.panel.b),
        ("panelColorA", colors.panel.a),
        ("dotRadius", led::DOT_RADIUS),
        ("threshold", led::THRESHOLD),
        ("glow", glow),
        ("spillMarginX", frac(spill.0, grown.0)),
        ("spillMarginY", frac(spill.1, grown.1)),
        ("spillStrength", spill_strength),
        ("spillDeadX", led::SPILL_DEAD.0),
        ("spillDeadY", led::SPILL_DEAD.1),
    ]
}

/// `tape_label.slang`'s uniforms, by name, for one label of `size` pixels
/// whose glyph box is `glyph_rect`.
pub fn tape_params(size: (f64, f64), glyph_rect: (f32, f32, f32, f32)) -> Vec<Param> {
    let tape_color = tape::tape_color();
    let letter_color = tape::letter_color();
    vec![
        ("sizePxX", size.0 as f32),
        ("sizePxY", size.1 as f32),
        ("lightDirX", tape::DISPLAY_LIGHT_DIR.0),
        ("lightDirY", tape::DISPLAY_LIGHT_DIR.1),
        ("tapeColorR", tape_color.r),
        ("tapeColorG", tape_color.g),
        ("tapeColorB", tape_color.b),
        ("tapeColorA", tape_color.a),
        ("letterColorR", letter_color.r),
        ("letterColorG", letter_color.g),
        ("letterColorB", letter_color.b),
        ("letterColorA", letter_color.a),
        ("glyphRectPxX", glyph_rect.0),
        ("glyphRectPxY", glyph_rect.1),
        ("glyphRectPxZ", glyph_rect.2),
        ("glyphRectPxW", glyph_rect.3),
        ("bevelPx", tape::BEVEL_PX),
        ("dilatePx", tape::DILATE_PX),
        ("sheenAmount", tape::SHEEN_AMOUNT),
        ("grainAmount", tape::GRAIN_AMOUNT),
        ("seed", tape::SEED),
    ]
}

/// The lamp grid one strip's window samples, one texel per lamp.
///
/// The strip's own raster is `term::fonts::led::led_text_image`'s (the
/// proven glyph-raster path), laid into a `grid_w` x `grid_h` field of unlit
/// lamps at an offset: in by `left_pad_cells` (in *cells*, hence the
/// multiply by the cell width -- a bank strip's own window passes
/// [`led::LED_SIDE_PAD_CELLS`] as its left pad; a fixture with no side pad
/// of its own, the slide rule's page counter, passes zero) and down by
/// `top_pad_cells`. Empty text is a field of unlit lamps, which is exactly
/// what a dark slot shows.
pub fn led_grid(
    font_data: &[u8],
    pixel_size: u32,
    text: &str,
    lamp_cell_width: u32,
    grid: (u32, u32),
    left_pad_cells: u32,
    top_pad_cells: u32,
) -> Raster {
    let (grid_w, grid_h) = (grid.0.max(1), grid.1.max(1));
    let mut alpha = vec![0u8; (grid_w * grid_h) as usize];
    if !text.is_empty() {
        if let Some(r) = term::fonts::led::led_text_image(font_data, pixel_size, text) {
            let x0 = left_pad_cells * lamp_cell_width;
            let y0 = top_pad_cells;
            for y in 0..r.height.min(grid_h.saturating_sub(y0)) {
                for x in 0..r.width.min(grid_w.saturating_sub(x0)) {
                    alpha[((y + y0) * grid_w + (x + x0)) as usize] =
                        r.alpha[(y * r.width + x) as usize];
                }
            }
        }
    }
    let widened = raster::to_rgba8(&term::fonts::led::LedRaster {
        width: grid_w,
        height: grid_h,
        alpha,
    });
    Raster {
        width: grid_w,
        height: grid_h,
        rgba: widened,
    }
}

/// Where one row's display window stands, in the bank column's own
/// coordinates.
///
/// The rows stack at the content ground, and the strip stands past the
/// numeral lane and its gap, vertically centred in the row.
///
/// Shared by the drawing and the hit test on purpose: the window *is* the
/// key -- pressing it reaches the channel, so the row carries no separate
/// button -- so the rectangle a press is tested against has to be the
/// rectangle that was drawn.
pub fn strip_rect(geometry: &BankGeometry, shell: &ShellMetrics, row: usize) -> Rect {
    let strip_x = (geometry.content_x + shell.numeral_width + shell.column_gap) as f64;
    let strip_w = geometry.strip_width as f64;
    let strip_h = geometry.strip_height as f64;
    let pitch = geometry.row_height as f64 + geometry.row_spacing;
    let strip_y = geometry.top_padding as f64
        + row as f64 * pitch
        + (geometry.row_height as f64 - strip_h) / 2.0;
    Rect::new(strip_x, strip_y, strip_w, strip_h)
}

/// Which row's window a press at `(x, y)` landed in, both in the bank column's
/// own coordinates, for a page of `rows` engraved keys.
///
/// `None` for a press on the bare plate between or beside the windows: the
/// row's press area is the display item and not the row.
pub fn strip_at(
    geometry: &BankGeometry,
    shell: &ShellMetrics,
    rows: usize,
    x: f64,
    y: f64,
) -> Option<usize> {
    (0..rows).find(|&row| {
        let r = strip_rect(geometry, shell, row);
        x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
    })
}

/// The whole plan: the shell's plate, then one display per row.
///
/// `bank` is the column's rectangle, logical pixels; every returned rect is in
/// its coordinates with the origin at its top-left corner. The order is the
/// order to draw in: the plate goes down first and the rows stand on it.
pub fn bank_pieces(
    cfg: &Config,
    shell: &ShellMetrics,
    display: &Display,
    geometry: &BankGeometry,
    bank: (f64, f64),
    strips: &BankStrips,
) -> Vec<Piece> {
    let mut pieces = Vec::new();
    if bank.0 <= 0.0 || bank.1 <= 0.0 {
        return pieces;
    }

    if let Some((rect, params)) = crate::shells::plate_region(cfg.chassis.shell, bank) {
        if rect.width > 0.0 && rect.height > 0.0 {
            pieces.push(Piece::shaded(
                Pass::Plate,
                Rect::new(rect.x, rect.y, rect.width, rect.height),
                plate_params(&params),
                None,
            ));
        }
    }

    // The chassis's screws are drawn after its plate, so they sit on the
    // plate and under everything the bank draws.
    pieces.extend(crate::shells::screws(cfg.chassis.shell, bank));

    if geometry.strip_width <= 0 || geometry.strip_height <= 0 {
        return pieces;
    }

    let plastic = plastic(cfg);

    // The pager stands in the content lane at the bank's foot, its own
    // natural height above the shell's bottom padding. Measured here and
    // not only drawn here, because the selector's parked position is the
    // middle of this rectangle.
    let content_width = geometry.content_width(bank.0 as i32);
    let pager_h = shell.pager_height(content_width) as f64;
    let pager_rect = Rect::new(
        geometry.content_x as f64,
        bank.1 - geometry.bottom_padding as f64 - pager_h,
        content_width as f64,
        pager_h,
    );

    // The selector sits at the track's lane, inset by the bank's padding
    // top and bottom, and it is drawn before the rows: the carriage stands
    // beside a window, never over one.
    if strips.indicator == crate::ChannelIndicator::Pointer && strips.pointer_shown {
        let track = Rect::new(
            geometry.track_x as f64,
            geometry.bank_padding as f64,
            geometry.track_width as f64,
            (bank.1 - 2.0 * geometry.bank_padding as f64).max(0.0),
        );
        if track.width > 0.0 && track.height > 0.0 {
            let target = geometry.pointer_y(strips.current_row, pager_rect.y + pager_h / 2.0);
            let painting = crate::shells::selector_track(
                cfg.chassis.shell,
                plastic,
                (track.width, track.height),
                target,
            );
            if !painting.is_empty() {
                pieces.push(Piece::painted(track, painting));
            }
        }
    }

    for (i, row) in strips.rows.iter().enumerate() {
        let strip = strip_rect(geometry, shell, i);
        // The row is the content lane by the shell's row height, and its
        // furniture fills it. The window's rectangle inside that row is
        // computed below from the shared `strip_rect`.
        let row_rect = Rect::new(
            geometry.content_x as f64,
            strip.y - (geometry.row_height as f64 - strip.height) / 2.0,
            (bank.0 - geometry.content_x as f64).max(0.0),
            geometry.row_height as f64,
        );
        let display_rect = Rect::new(
            strip.x - row_rect.x,
            (row_rect.height - strip.height) / 2.0,
            strip.width,
            strip.height,
        );
        for mut piece in crate::shells::row_furniture(
            cfg.chassis.shell,
            plastic,
            &row.numeral,
            (row_rect.width, row_rect.height),
            display_rect,
            row.current,
            font_color(cfg),
        ) {
            piece.rect.x += row_rect.x;
            piece.rect.y += row_rect.y;
            pieces.push(piece);
        }
        // Already decided by whoever built the page: the glow law lights
        // only the channel on the air, and the other two light every open
        // window because the mark is the selector's or the lever's.
        let bright = row.bright;
        match display {
            Display::Led(kit) => pieces.extend(led_piece(cfg, kit, geometry, strip, row, bright)),
            Display::Tape(kit) => {
                // The well is fixed furniture and is drawn always; the tape
                // is only there when a channel is. So the chrome goes down
                // whatever the slot holds, and the label, when there is
                // one, lies in it.
                pieces.push(Piece::painted(strip, tape::well_chrome(strip)));
                pieces.extend(tape_piece(cfg, kit, strip, row, bright));
            }
        }
    }

    // Last, and over everything: the pager is drawn as the final piece of
    // the bank.
    if pager_rect.width > 0.0 && pager_rect.height > 0.0 {
        for mut piece in crate::shells::pager(
            cfg.chassis.shell,
            plastic,
            (pager_rect.width, pager_rect.height),
            strips.page_index,
            strips.page_count,
            cfg,
        ) {
            // The shells build the pager in its own coordinates; the bank
            // is where it stands.
            piece.rect.x += pager_rect.x;
            piece.rect.y += pager_rect.y;
            pieces.push(piece);
        }
    }
    pieces
}

/// The appliance's plastic, which every piece of painted furniture takes its
/// shade from.
///
/// Three derived colours go in, not the profile's stored hex: the frame
/// colour through the shared hex parser, and the *mixed* font and background
/// colours that [`font_color`] and [`background_color`] compute.
/// `crt::params` builds the same value for the frame pass; the duplicate is
/// [`crate::color`]'s standing decision.
pub fn plastic(cfg: &Config) -> Rgba {
    let frame = if cfg.general.chassis_shown {
        &cfg.chassis.frame_color
    } else {
        &cfg.screen.frame_color
    };
    color::frame_base_color(
        color::str_to_color(frame),
        font_color(cfg),
        background_color(cfg),
        cfg.screen.ambient_light as f32,
    )
}

/// The derived background colour, the other half of the pair [`font_color`]
/// computes; see that function's doc for the whole derivation.
pub fn background_color(cfg: &Config) -> Rgba {
    let saturated = color::mix(
        color::str_to_color(&cfg.screen.font_color),
        color::str_to_color("#FFFFFF"),
        cfg.screen.saturation_color as f32 * 0.5,
    );
    let background = color::str_to_color(&cfg.screen.background_color);
    color::mix(
        saturated,
        background,
        0.7 + cfg.screen.contrast as f32 * 0.3,
    )
}

/// One LED window, glyph raster through shader uniforms, end to end.
fn led_piece(
    cfg: &Config,
    kit: &LedMetrics,
    geometry: &BankGeometry,
    strip: Rect,
    row: &StripRow,
    bright: bool,
) -> Option<Piece> {
    // The count the strip was measured at, not the count the settings ask
    // for: in a window too narrow for the configured bank the two part
    // company, and the lamps belong to the width beside them.
    let characters = geometry.characters.max(0) as u32;
    let lamp_cell_width = kit.lamp_cell_width.max(1) as u32;
    let lamp_cell_height = kit.lamp_cell_height.max(1) as u32;
    let pad_cells_y = geometry.pad_cells_y.max(0) as u32;
    let grid = led::grid_size(
        lamp_cell_width,
        lamp_cell_height,
        characters,
        led::LED_SIDE_PAD_CELLS,
        led::LED_SIDE_PAD_CELLS,
        pad_cells_y,
    );

    let entry = term::fonts::font_by_name(&cfg.chassis.bank_font_name)
        .or_else(|| term::fonts::font_by_name(led::DEFAULT_LED_FONT_NAME))?;
    let shown = led::visible_text(&row.title, row.open, characters as usize);
    // The top band splits the vertical pad in half, rounded down.
    let source = led_grid(
        entry.data(),
        entry.pixel_size,
        shown,
        lamp_cell_width,
        grid,
        led::LED_SIDE_PAD_CELLS,
        pad_cells_y / 2,
    );

    // Both margins are struck from the strip's *height*, and the drawn
    // rectangle stands proud of the strip by them on every side.
    let (spill_x, spill_y) = led::spill_margins(strip.height);
    let (spill_x, spill_y) = (spill_x as f64, spill_y as f64);
    let grown = Rect::new(
        strip.x - spill_x,
        strip.y - spill_y,
        strip.width + 2.0 * spill_x,
        strip.height + 2.0 * spill_y,
    );

    let colors = led::window_colors(font_color(cfg), row.open, bright);
    Some(Piece::shaded(
        Pass::LedMatrix,
        grown,
        led_params(
            grid,
            colors,
            led::glow(bright),
            led::spill_strength(row.open, bright),
            (spill_x, spill_y),
            (grown.width, grown.height),
        ),
        Some(source),
    ))
}

/// One tape label: the well plus the stamped label inside it.
///
/// The well's gradient floor around the label is a vector rectangle, not a
/// pass; see this module's doc on where that line runs.
fn tape_piece(
    cfg: &Config,
    kit: &TapeMetrics,
    strip: Rect,
    row: &StripRow,
    _bright: bool,
) -> Option<Piece> {
    /// The label's inset inside the well.
    const WELL_INSET: f64 = 3.0;

    // The cabinet's own face, the one `tape_metrics` measured the wheel from,
    // so the stamp and the spacing it was cut for agree.
    let entry = term::fonts::font_by_name(&cfg.chassis.bank_font_name)
        .or_else(|| term::fonts::font_by_name(tape::FONT_NAME))?;
    let label_h = (strip.height - 2.0 * WELL_INSET).max(1.0);
    let letter_scale = tape::letter_scale(label_h);
    let raster_size = tape::raster_size(label_h, letter_scale);

    // A dark slot shows blank tape, which is a label with no glyph box
    // rather than no label: the stamp still runs over an empty slot.
    let characters = kit.min_characters.max(0) as usize;
    let shown = tape::visible_text(&row.title, row.open, characters).to_uppercase();
    let raster = term::fonts::led::led_text_image(entry.data(), raster_size, &shown);
    let (glyph_w, glyph_h) = raster
        .as_ref()
        .map(|r| (r.width as f64 / 2.0, r.height as f64 / 2.0))
        .unwrap_or((0.0, 0.0));
    let label_w = tape::tape_label_implicit_width(glyph_w, tape::END_PAD as f64).max(1) as f64;
    let rect = Rect::new(strip.x + WELL_INSET, strip.y + WELL_INSET, label_w, label_h);
    let glyph_rect = tape::glyph_rect_px(label_w, label_h, glyph_w, glyph_h);
    let source = raster.map(|r| Raster {
        width: r.width,
        height: r.height,
        rgba: raster::to_rgba8(&r),
    });
    Some(Piece::shaded(
        Pass::TapeLabel,
        rect,
        tape_params((label_w, label_h), glyph_rect),
        // A blank tape has no glyph raster at all; the pass samples nothing
        // inside a zero-area glyph box, so a 1x1 dark texel is enough to bind.
        Some(source.unwrap_or(Raster {
            width: 1,
            height: 1,
            rgba: vec![0, 0, 0, 0],
        })),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Cabinet;

    #[test]
    fn a_press_lands_in_the_window_it_was_drawn_in() {
        let cfg = Config::default();
        let cabinet = Cabinet::from_config(&cfg, 1024.0, 768.0);
        let rows = cabinet.rows_visible() as usize;
        let g = cabinet.geometry();
        let shell = crate::shell_metrics(&cfg);

        // The rectangle the hit test uses is the one the drawing used, which is
        // the whole point of sharing `strip_rect`: a press in the middle of the
        // third window is the third row and nothing else.
        let third = strip_rect(g, &shell, 2);
        let mid = |r: Rect| (r.x + r.width / 2.0, r.y + r.height / 2.0);
        let (x, y) = mid(third);
        assert_eq!(cabinet.strip_at(rows, x, y), Some(2));
        assert_eq!(cabinet.strip_at(rows, third.x, third.y), Some(2));

        // The numeral lane to the left of the windows is plate, and a row
        // carries no press area of its own there.
        assert_eq!(cabinet.strip_at(rows, third.x - 1.0, y), None);
        // ...and so is the air between two rows, where the pitch exceeds the
        // window's own height.
        let fourth = strip_rect(g, &shell, 3);
        assert!(fourth.y > third.y + third.height);
        assert_eq!(cabinet.strip_at(rows, x, third.y + third.height), None);
        // A row past the page's last engraved key is nobody's.
        assert_eq!(
            cabinet.strip_at(rows, x, mid(strip_rect(g, &shell, rows)).1),
            None
        );
    }

    #[test]
    fn the_shipped_appliance_puts_a_plate_and_a_strip_per_row_on_the_casting() {
        let cfg = Config::default();
        let cabinet = Cabinet::from_config(&cfg, 1024.0, 768.0);
        let rows = cabinet.rows_visible();
        // With the annunciator's own 130px pager:
        // floor((768 - 61 - 8 - 130 - 2 + 2) / 45) = floor(569/45) = 12.
        assert_eq!(rows, 12);

        let pieces = cabinet.furniture(&BankStrips::cold_start(rows as usize));
        // Item order, top to bottom: the plate, the chassis's four screws
        // over it, then one row's furniture and one row's window per
        // engraved key, then the pager.
        assert_eq!(pieces.len(), 1 + 4 + 2 * rows as usize + 1);
        assert_eq!(pieces[0].pass, Pass::Plate);
        // The annunciator's plate: 8px left margin, 2px top margin, no
        // right margin, 8px bottom margin, over a 205px bank.
        assert_eq!(pieces[0].rect, Rect::new(8.0, 2.0, 197.0, 758.0));
        assert!(pieces[0].source.is_none());

        // Four 28px screws, two at the plate's head and two 25px off its
        // foot, the right pair pinned to the chassis's right edge so a
        // narrower window keeps them.
        for p in &pieces[1..5] {
            assert_eq!(p.pass, Pass::Painted);
            assert_eq!((p.rect.width, p.rect.height), (28.0, 28.0));
        }
        assert_eq!((pieces[1].rect.x, pieces[1].rect.y), (18.0, 16.0));
        assert_eq!(pieces[2].rect.x, 205.0 - 12.0 - 28.0);
        assert_eq!(pieces[3].rect.y, 768.0 - 25.0 - 28.0);

        let row_piece = |i: usize| &pieces[5 + 2 * i];
        let strip_piece = |i: usize| &pieces[6 + 2 * i];

        // Every row is furniture then an LED window, and only the first one is
        // powered.
        for i in 0..rows as usize {
            assert_eq!(row_piece(i).pass, Pass::Painted);
            assert!(row_piece(i).paint.is_some());
            assert_eq!(strip_piece(i).pass, Pass::LedMatrix);
            assert!(strip_piece(i).source.is_some());
        }
        let spill = |p: &Piece| {
            p.params
                .iter()
                .find(|(n, _)| *n == "spillStrength")
                .unwrap()
                .1
        };
        assert_eq!(spill(strip_piece(0)), 1.0); // powered and bright
        assert_eq!(spill(strip_piece(1)), 0.0); // dark

        // The strip stands past the numeral lane, the row pitch is the
        // shell's, and the drawn rectangle is the strip grown by its spill
        // margins.
        let g = cabinet.geometry();
        let strip_x = (g.content_x + 46 + 16) as f64;
        let (sx, sy) = led::spill_margins(g.strip_height as f64);
        assert_eq!(strip_piece(0).rect.x, strip_x - sx as f64);
        assert_eq!(
            strip_piece(0).rect.width,
            g.strip_width as f64 + 2.0 * sx as f64
        );
        assert_eq!(
            strip_piece(1).rect.y - strip_piece(0).rect.y,
            g.row_height as f64 + g.row_spacing
        );
        // The strip's right edge is the bank less the shell's right padding.
        assert_eq!(strip_x + g.strip_width as f64, 205.0 - 14.0);
        // ...and the grown rectangle overhangs the bank on purpose: the
        // spill is light thrown on the plate, and the plate runs on.
        assert!(strip_piece(0).rect.y - sy as f64 <= 61.0);

        // The row's furniture covers the content lane at the row's own pitch,
        // and the window it paints its moulding around is the one the strip
        // was drawn in: the same sharing `strip_rect` gives the hit test.
        let furniture = row_piece(0);
        assert_eq!(furniture.rect.x, g.content_x as f64);
        assert_eq!(furniture.rect.width, 205.0 - g.content_x as f64);
        assert_eq!(furniture.rect.height, g.row_height as f64);
        assert_eq!(
            row_piece(1).rect.y - furniture.rect.y,
            g.row_height as f64 + g.row_spacing
        );

        // The pager stands last, in the content lane, its own height above
        // the shell's bottom padding.
        let pager = pieces.last().unwrap();
        assert_eq!(pager.pass, Pass::Painted);
        assert_eq!(pager.rect.x, g.content_x as f64);
        assert_eq!(pager.rect.height, 130.0);
        assert_eq!(pager.rect.y + 130.0, 768.0 - 8.0);
    }

    /// The done-test's per-piece readback for the shipped appliance: what the
    /// numerals, the moulding and the pager actually put on the plate.
    ///
    /// It reads the rasterised picture rather than the plan, because the plan
    /// is where a wrong colour or a missing gradient is invisible.
    #[test]
    fn a_rows_furniture_strikes_its_numeral_and_moulds_its_window() {
        use crate::paint::rasterize;

        let cfg = Config::default();
        let cabinet = Cabinet::from_config(&cfg, 1024.0, 768.0);
        let pieces = cabinet.furniture(&BankStrips::cold_start(cabinet.rows_visible() as usize));
        // The third row: numeral "03", so its jitter is its own.
        let row = &pieces[5 + 2 * 2];
        let painting = row.paint.as_ref().expect("a painted row");
        let out = rasterize(
            painting,
            (row.rect.width as u32, row.rect.height as u32),
            1.0,
        );
        let px = |x: u32, y: u32| {
            let i = ((y * out.width + x) * 4) as usize;
            [
                out.rgba[i],
                out.rgba[i + 1],
                out.rgba[i + 2],
                out.rgba[i + 3],
            ]
        };

        // The numeral: the lane is 46 wide less the 16px gap, the digits are
        // right-aligned in it, and the ink is the struck face's own colour
        // over nothing: the plate shows through everywhere else, which is
        // what the alpha says.
        let lane = 46 - 16;
        let ink: Vec<[u8; 4]> = (0..lane)
            .flat_map(|x| (0..out.height).map(move |y| (x as u32, y)))
            .map(|(x, y)| px(x, y))
            .filter(|p| p[3] > 200)
            .collect();
        assert!(!ink.is_empty(), "no numeral struck in the lane");
        // Two strikes: the shadow low and right, the lit face over it. So the
        // lane holds both the dark tone and the light one. The numeral's
        // fill colour is #9c8168.
        let fill = crate::color::hex_literal_to_color("#9c8168");
        let brightest = ink.iter().map(|p| p[0]).max().unwrap();
        assert!(
            brightest as f32 / 255.0 > fill.r * 0.8,
            "the lit strike is missing: brightest red {brightest}"
        );
        // The lane's own left margin is bare plate: a printer leaves a
        // margin there rather than running the ink to the cut.
        for y in 0..out.height {
            assert_eq!(px(0, y)[3], 0, "ink at the lane's left cut, row {y}");
        }

        // The moulding: a five-stop vertical gradient across the row, so the
        // rim's top edge is lit, its middle drops dark, and its foot lifts
        // again. Sampled down the rim's own left shoulder, clear of the
        // window.
        let display_x = (46 + 16) as u32;
        // Clear of the rim's own 8px corner radius, and left of the hole.
        let rim_x = display_x - 2;
        let top = px(rim_x, 0)[0];
        let middle = px(rim_x, out.height / 2)[0];
        let foot = px(rim_x, out.height - 1)[0];
        assert!(
            top > middle,
            "the rim's lit top edge is missing: {top} vs {middle}"
        );
        assert!(
            foot > middle,
            "the rim's lit foot is missing: {foot} vs {middle}"
        );
        assert_eq!(px(rim_x, 0)[3], 255, "the moulding is transparent");

        // The punched hole inside it is darker than the moulding around it at
        // every height, which is the whole point of a recess.
        for y in 6..out.height - 6 {
            let hole = px(display_x + 10, y)[0];
            let lip = px(rim_x, y)[0];
            assert!(hole <= lip, "the hole is lighter than its rim at {y}");
        }
    }

    #[test]
    fn the_pager_puts_its_two_keys_where_the_mock_measured_them() {
        use crate::paint::rasterize;

        let cfg = Config::default();
        let cabinet = Cabinet::from_config(&cfg, 1024.0, 768.0);
        let mut strips = BankStrips::cold_start(cabinet.rows_visible() as usize);
        // A single-page bank draws its rocker at 0.55, so the measured case
        // is a bank with somewhere to go.
        strips.page_count = 3;
        let pieces = cabinet.furniture(&strips);
        let pager = pieces.last().expect("a pager");
        let out = rasterize(
            pager.paint.as_ref().expect("a painted pager"),
            (pager.rect.width as u32, pager.rect.height as u32),
            1.0,
        );
        let px = |x: u32, y: u32| {
            let i = ((y * out.width + x) * 4) as usize;
            [
                out.rgba[i],
                out.rgba[i + 1],
                out.rgba[i + 2],
                out.rgba[i + 3],
            ]
        };

        // Two 56px caps, the pair centred in the lane at a 92px spread,
        // tops 38px into the block.
        let width = pager.rect.width;
        let spread = 92.0f64.min(width - 2.0 * 56.0 - 8.0);
        let prev_x = ((width - 2.0 * 56.0 - spread) / 2.0) as u32;
        let next_x = prev_x + 56 + spread as u32;
        // The key's first ridge is the brightest thing on it (#f7e8c4), two
        // pixels into the cap and two down from its top.
        for x in [prev_x, next_x] {
            let ridge = px(x + 28, 38 + 3);
            assert!(
                ridge[0] > 200 && ridge[1] > 180,
                "no lit ridge on the key at x {x}: {ridge:?}"
            );
            // ...and the cap's front face below the ridges is near-black.
            let face = px(x + 28, 38 + 30);
            assert!(face[0] < 60, "the cap's front face is not dark: {face:?}");
        }
        // Between the keys is bare plate: the pager paints nothing there.
        assert_eq!(px(prev_x + 56 + 4, 38 + 20)[3], 0);

        // A bank with one page dims the whole rocker to 0.55 rather than
        // hiding it, so the same ridge is there and darker.
        let mut one = BankStrips::cold_start(cabinet.rows_visible() as usize);
        one.page_count = 1;
        let dim = cabinet.furniture(&one);
        let dim = dim.last().unwrap();
        let dim = rasterize(
            dim.paint.as_ref().unwrap(),
            (dim.rect.width as u32, dim.rect.height as u32),
            1.0,
        );
        let i = (((38 + 3) * out.width + prev_x + 28) * 4) as usize;
        assert!(dim.rgba[i] < px(prev_x + 28, 38 + 3)[0]);
        assert!(dim.rgba[i] > 0);
    }

    #[test]
    fn a_screw_head_is_domed_lit_and_slotted() {
        use crate::paint::rasterize;

        let head = crate::shells::common::screw_head(28.0, 24.0);
        let out = rasterize(&head, (28, 28), 1.0);
        let px = |x: u32, y: u32| {
            let i = ((y * out.width + x) * 4) as usize;
            [
                out.rgba[i],
                out.rgba[i + 1],
                out.rgba[i + 2],
                out.rgba[i + 3],
            ]
        };
        // The countersink's outer ring is dark and the head inside it is not:
        // the light comes from up and left, so the dome is brightest that
        // side and falls away opposite.
        let up_left = px(9, 9);
        let down_right = px(19, 19);
        assert!(
            up_left[0] > down_right[0],
            "the dome is lit from the wrong side: {up_left:?} vs {down_right:?}"
        );
        // The slot is cut through the middle, dark against the dome around it.
        assert!(px(14, 14)[0] < up_left[0], "no slot cut in the head");
        // The corners of the item are outside the countersink disc entirely.
        assert_eq!(px(0, 0)[3], 0);
    }

    #[test]
    fn the_switchboard_has_no_plate_and_stamps_tape() {
        let mut cfg = Config::default();
        cfg.chassis.shell = config::Shell::Switchboard;
        cfg.chassis.channel_display = config::ChannelDisplay::Tape;
        cfg.chassis.channel_indicator = config::ChannelIndicator::Switch;
        let cabinet = Cabinet::from_config(&cfg, 1024.0, 768.0);
        let pieces = cabinet.furniture(&BankStrips::cold_start(3));
        // The switchboard chassis has no plate region and no screws on the
        // casting at all, so the first piece is already a row. This
        // shell's row is four of them, because its plate is a `plate_metal`
        // pass under painted rivets, well and numeral, and the tape kit lays a
        // painted well under its stamped label.
        assert_eq!(
            pieces[..4].iter().map(|p| p.pass).collect::<Vec<_>>(),
            vec![Pass::Plate, Pass::Painted, Pass::Painted, Pass::TapeLabel]
        );
        // Under the switch law every open window is bright, so the one open
        // slot's brightness is not what tells the laws apart. The geometry
        // is: the label is inset 3px inside the well at both ends.
        assert_eq!(
            pieces[3].rect.height,
            cabinet.geometry().strip_height as f64 - 6.0
        );
        // ...and the well the label lies in is the whole strip, drawn
        // whether a channel is there or not.
        assert_eq!(
            pieces[2].rect.height,
            cabinet.geometry().strip_height as f64
        );

        // The rail across the bank's foot follows the three rows.
        let pager: Vec<Pass> = pieces[12..].iter().map(|p| p.pass).collect();
        assert_eq!(pager[0], Pass::Plate, "the pager's rail plate");
        assert!(pager.contains(&Pass::Painted));
    }

    /// The done-test's per-piece readback for the switchboard's lever
    /// assembly at rest (`shells::switchboard::row_furniture`'s newly-painted
    /// half): the machined chamfer, the drop shadow sunk into the well, and
    /// the retaining screw's dark recess, each where the resting (not
    /// current) lever puts them.
    #[test]
    fn the_switchboard_lever_lies_flat_over_the_well_at_rest() {
        use crate::paint::rasterize;

        let mut cfg = Config::default();
        cfg.chassis.shell = config::Shell::Switchboard;
        let cabinet = Cabinet::from_config(&cfg, 1024.0, 768.0);
        let pieces = cabinet.furniture(&BankStrips::cold_start(3));
        // `the_switchboard_has_no_plate_and_stamps_tape`: the row's own
        // `Plate` shader is [0], its painting (numeral, well, lever) is [1].
        let row = &pieces[1];
        assert_eq!(row.pass, Pass::Painted);
        let out = rasterize(
            row.paint.as_ref().expect("a painted row"),
            (row.rect.width as u32, row.rect.height as u32),
            1.0,
        );
        let px = |x: u32, y: u32| {
            let i = ((y * out.width + x) * 4) as usize;
            [
                out.rgba[i],
                out.rgba[i + 1],
                out.rgba[i + 2],
                out.rgba[i + 3],
            ]
        };

        // `row_overhang`: the switchboard's row paints 16px proud at both
        // ends, and the piece's rect is grown to hold it; every op landed
        // shifted down by that same 16.
        let top = 16.0;
        let g = cabinet.geometry();
        // `metrics::SWITCH_WELL_X + 3`: the lever's rest position.
        let lever_x = 64.0 + 3.0;
        let lever_y = (g.row_height as f64 - 54.0) / 2.0 + top;

        // The machined chamfer down the cap's right side, the brightest
        // metal on the row at rest -- sampled at the quadrilateral's own
        // centroid.
        let chamfer = px((lever_x + 64.0) as u32, (lever_y + 26.0) as u32);
        assert!(chamfer[3] > 0, "no chamfer struck on the lever");
        assert!(
            chamfer[0] > 100,
            "the chamfer should read as the row's brightest metal: {chamfer:?}"
        );

        // The cap's drop shadow into the well, sampled clear of the face and
        // the chamfer above it.
        let shadow = px((lever_x + 20.0) as u32, (lever_y + 50.0) as u32);
        assert_eq!(shadow[3], 255, "the drop shadow region is transparent");
        assert!(
            shadow[0] < 80,
            "the drop shadow should read dark: {shadow:?}"
        );

        // The retaining screw's near-black recess in the cap's left half.
        let screw = px((lever_x + 16.0) as u32, (lever_y + 23.0) as u32);
        assert!(
            screw[0] < 40,
            "the retaining screw's recess should be near-black: {screw:?}"
        );
    }

    /// The done-test's per-piece readback for [`tape::well_chrome`] as it
    /// actually composites on the switchboard bank, not just as the function
    /// returns it in isolation (`displays::tape::mod`'s own unit tests cover
    /// that): `tape::well_chrome` paints the well's chrome, and this is the
    /// switchboard row that mounts it (`the_switchboard_has_no_plate_and_stamps_tape`'s
    /// `pieces[2]`, the well painted whether a channel lies in it or not).
    #[test]
    fn the_switchboard_tape_well_composites_its_chrome() {
        use crate::paint::rasterize;

        let mut cfg = Config::default();
        cfg.chassis.shell = config::Shell::Switchboard;
        cfg.chassis.channel_display = config::ChannelDisplay::Tape;
        cfg.chassis.channel_indicator = config::ChannelIndicator::Switch;
        let cabinet = Cabinet::from_config(&cfg, 1024.0, 768.0);
        let pieces = cabinet.furniture(&BankStrips::cold_start(3));
        let well = &pieces[2];
        assert_eq!(well.pass, Pass::Painted);
        let out = rasterize(
            well.paint.as_ref().expect("a painted well"),
            (well.rect.width as u32, well.rect.height as u32),
            1.0,
        );
        let px = |x: u32, y: u32| {
            let i = ((y * out.width + x) * 4) as usize;
            [
                out.rgba[i],
                out.rgba[i + 1],
                out.rgba[i + 2],
                out.rgba[i + 3],
            ]
        };
        let mid_x = out.width / 2;

        // `tape/mod.rs:86-95`: the floor's own vertical gradient runs darkest
        // at the top lip to lightest at the foot, so it is opaque and
        // monotone brightening top to bottom, sampled clear of the side walls
        // and the top shadow band.
        assert_eq!(px(mid_x, 6)[3], 255, "the well floor is transparent");
        let top = px(mid_x, 6)[0];
        let bottom = px(mid_x, out.height - 1)[0];
        assert!(
            bottom > top,
            "the floor's foot should be lighter than its lip: {top} vs {bottom}"
        );

        // `:96-104`: the top lip's shadow, darker right under it than a few
        // pixels down where the shade has faded to the bare gradient.
        let shadow = px(mid_x, 0)[0];
        let past_shadow = px(mid_x, 6)[0];
        assert!(
            shadow <= past_shadow,
            "no shadow under the well's top lip: {shadow} vs {past_shadow}"
        );

        // `:105-122`: the left wall falls dark with the key leaning that way
        // and the right one catches a little of it, so at the same height the
        // left edge reads darker than the right.
        let mid_y = out.height / 2;
        let left = px(1, mid_y)[0];
        let right = px(out.width - 1, mid_y)[0];
        assert!(
            left < right,
            "the well's left wall should be darker than its right: {left} vs {right}"
        );
    }

    /// Every shipped combination paints something, at its own rectangle, with
    /// no NaN geometry and nothing off the piece it belongs to.
    ///
    /// Only the annunciator over LEDs is measured against the recorded floor
    /// (it is the shipped appliance and the configuration that floor was
    /// measured on), so this is the net under the other eight: a shell
    /// whose numeral lane went negative or whose pager squeezed to nothing
    /// fails here rather than in a screenshot nobody takes.
    #[test]
    fn every_shell_and_kit_paints_a_bank_that_rasterises() {
        use crate::paint::rasterize;
        use config::{ChannelDisplay, ChannelIndicator, Shell};

        for shell in [Shell::Annunciator, Shell::SlideRule, Shell::Switchboard] {
            for display in [ChannelDisplay::Led, ChannelDisplay::Tape] {
                for indicator in [
                    ChannelIndicator::Glow,
                    ChannelIndicator::Pointer,
                    ChannelIndicator::Switch,
                ] {
                    let mut cfg = Config::default();
                    cfg.chassis.shell = shell;
                    cfg.chassis.channel_display = display;
                    cfg.chassis.channel_indicator = indicator;
                    let cabinet = Cabinet::from_config(&cfg, 1448.0, 1086.0);
                    let rows = cabinet.rows_visible() as usize;
                    let mut strips = BankStrips::cold_start(rows);
                    strips.indicator = crate::ChannelIndicator::from_setting(match indicator {
                        ChannelIndicator::Pointer => "pointer",
                        ChannelIndicator::Switch => "switch",
                        ChannelIndicator::Glow => "glow",
                    });
                    strips.page_count = 4;
                    let pieces = cabinet.furniture(&strips);
                    let what = format!("{shell:?}/{display:?}/{indicator:?}");
                    let painted: Vec<&Piece> =
                        pieces.iter().filter(|p| p.pass == Pass::Painted).collect();
                    assert!(!painted.is_empty(), "{what} paints nothing at all");
                    let mut ink = 0u64;
                    for p in painted {
                        assert!(
                            p.rect.width > 0.0 && p.rect.height > 0.0,
                            "{what}: a painted piece has no area: {:?}",
                            p.rect
                        );
                        let out = rasterize(
                            p.paint
                                .as_ref()
                                .expect("a painted piece carries a painting"),
                            (p.rect.width as u32, p.rect.height as u32),
                            1.0,
                        );
                        assert_eq!(
                            out.rgba.len() as u32,
                            out.width * out.height * 4,
                            "{what}: the raster is not its own size"
                        );
                        ink += out
                            .rgba
                            .iter()
                            .skip(3)
                            .step_by(4)
                            .filter(|&&a| a > 0)
                            .count() as u64;
                    }
                    assert!(ink > 1000, "{what} covered only {ink} pixels");
                }
            }
        }
    }

    #[test]
    fn the_slide_rule_screws_its_rail_over_the_casting() {
        let mut cfg = Config::default();
        cfg.chassis.shell = config::Shell::SlideRule;
        let cabinet = Cabinet::from_config(&cfg, 1024.0, 768.0);
        let pieces = cabinet.furniture(&BankStrips::cold_start(2));
        assert_eq!(pieces[0].pass, Pass::Plate);
        // x 29, width 41, inset 29 top and bottom: the rail, which is this
        // shell's plate region.
        assert_eq!(pieces[0].rect, Rect::new(29.0, 29.0, 41.0, 768.0 - 58.0));
    }

    /// The done-test's per-piece readback for
    /// [`crate::shells::slide_rule::screw_places`]: the hinge bracket's three
    /// screws, at their measured bank-coordinate centres, turned from the
    /// bracket's own sheet metal.
    #[test]
    fn the_slide_rule_bolts_its_hinge_bracket_screws_to_the_casting() {
        use crate::paint::rasterize;

        let mut cfg = Config::default();
        cfg.chassis.shell = config::Shell::SlideRule;
        let cabinet = Cabinet::from_config(&cfg, 1024.0, 768.0);
        let pieces = cabinet.furniture(&BankStrips::cold_start(2));
        assert_eq!(pieces[0].pass, Pass::Plate, "the rail");
        let screws = &pieces[1..4];
        assert_eq!(screws.len(), 3);
        for p in screws {
            assert_eq!(p.pass, Pass::Painted);
            assert_eq!((p.rect.width, p.rect.height), (22.0, 22.0));
        }
        // The measured bank-coordinate centres, less the 22px head's own
        // half-width.
        for (p, (cx, cy)) in screws
            .iter()
            .zip([(46.0, 69.0), (64.0, 84.0), (47.0, 102.0)])
        {
            assert_eq!((p.rect.x, p.rect.y), (cx - 11.0, cy - 11.0));
        }

        // The dome is lit from the default light key (unoverridden in the
        // bracket's three), the same `(-0.6, -0.8)` `common::screw_head`'s
        // own default is, so it reads the same way
        // `a_screw_head_is_domed_lit_and_slotted` does: brighter up-left than
        // down-right.
        let out = rasterize(
            screws[0].paint.as_ref().expect("a painted screw"),
            (22, 22),
            1.0,
        );
        let px = |x: u32, y: u32| {
            let i = ((y * out.width + x) * 4) as usize;
            [
                out.rgba[i],
                out.rgba[i + 1],
                out.rgba[i + 2],
                out.rgba[i + 3],
            ]
        };
        let up_left = px(6, 6);
        let down_right = px(15, 15);
        assert!(
            up_left[0] > down_right[0],
            "the bracket screw's dome is lit from the wrong side: {up_left:?} vs {down_right:?}"
        );
    }

    /// The done-test's per-piece readback for the slide rule's page counter
    /// lamps (`shells::slide_rule::counter_lamps`): a second display-kit
    /// instance mounts behind the page window, at a two-character,
    /// no-padding, throttled-spill configuration.
    #[test]
    fn the_slide_rule_pagers_counter_lamps_burn_the_page_number() {
        let mut cfg = Config::default();
        cfg.chassis.shell = config::Shell::SlideRule;
        let cabinet = Cabinet::from_config(&cfg, 1024.0, 768.0);
        let mut strips = BankStrips::cold_start(cabinet.rows_visible() as usize);
        strips.page_count = 3;
        strips.page_index = 0; // "01"
        let pieces = cabinet.furniture(&strips);
        // `bank_pieces`'s own doc: "Last, and over everything": the pager,
        // and within it the counter lamps are the last thing this shell's
        // own `pager` pushes.
        let counter = pieces.last().expect("a pager");
        assert_eq!(counter.pass, Pass::LedMatrix);
        let source = counter.source.as_ref().expect("a lamp raster");
        assert!(
            source.rgba.iter().any(|&b| b > 0),
            "no glyph struck for the page label"
        );

        let param = |name: &str| {
            counter
                .params
                .iter()
                .find(|(n, _)| *n == name)
                .unwrap_or_else(|| panic!("no {name} uniform"))
                .1
        };
        // `characters: 2`, every pad at 0.
        let kit = crate::led_metrics(&cfg.chassis.bank_font_name);
        assert_eq!(param("gridSizeX"), (kit.lamp_cell_width.max(1) * 2) as f32);
        assert_eq!(param("gridSizeY"), kit.lamp_cell_height.max(1) as f32);
        // `spillStrength: 0.12`, unlike a channel window's own
        // `led::spill_strength`.
        assert_eq!(param("spillStrength"), 0.12);
        // The counter's own `bright: false` binding.
        assert_eq!(param("glow"), led::glow(false));

        // A different page turns different lamps: the raster is the label's
        // own, not a fixture.
        strips.page_index = 8; // "09"
        let turned = cabinet.furniture(&strips);
        let turned_counter = turned.last().expect("a pager");
        assert_ne!(
            counter.source, turned_counter.source,
            "the counter lamps did not follow the page label"
        );
    }

    #[test]
    fn a_hidden_chassis_has_no_furniture() {
        let mut cfg = Config::default();
        cfg.general.chassis_shown = false;
        let cabinet = Cabinet::from_config(&cfg, 1024.0, 768.0);
        assert!(cabinet.furniture(&BankStrips::cold_start(12)).is_empty());
    }

    #[test]
    fn the_lamp_grid_lays_the_proven_raster_in_a_field_of_dark_ones() {
        let entry = term::fonts::font_by_name(led::DEFAULT_LED_FONT_NAME).unwrap();
        let cell = 8;
        let grid = (cell * (12 + 2), 8 + 15);
        let r = led_grid(
            entry.data(),
            entry.pixel_size,
            "1",
            cell,
            grid,
            led::LED_SIDE_PAD_CELLS,
            15 / 2,
        );
        assert_eq!((r.width, r.height), grid);
        assert_eq!(r.rgba.len() as u32, grid.0 * grid.1 * 4);

        // The side pad column and the top band are unlit, whatever the glyph.
        let lit = |x: u32, y: u32| r.rgba[((y * grid.0 + x) * 4) as usize];
        for y in 0..grid.1 {
            for x in 0..cell {
                assert_eq!(lit(x, y), 0, "the left pad cell is lit at ({x},{y})");
            }
        }
        for y in 0..(15 / 2) {
            for x in 0..grid.0 {
                assert_eq!(lit(x, y), 0, "the top band is lit at ({x},{y})");
            }
        }
        // ...and the glyph itself did arrive, in the first character cell
        // past the pad.
        let glyph = term::fonts::led::led_text_image(entry.data(), entry.pixel_size, "1").unwrap();
        assert!(glyph.alpha.iter().any(|&a| a > 0));
        for y in 0..glyph.height {
            for x in 0..glyph.width {
                assert_eq!(
                    lit(x + cell, y + 15 / 2),
                    glyph.alpha[(y * glyph.width + x) as usize],
                    "the raster moved at ({x},{y})"
                );
            }
        }

        // Empty text is a field of unlit lamps, not an absent one.
        let dark = led_grid(
            entry.data(),
            entry.pixel_size,
            "",
            cell,
            grid,
            led::LED_SIDE_PAD_CELLS,
            7,
        );
        assert!(dark.rgba.iter().all(|&b| b == 0));
        assert_eq!(dark.rgba.len(), r.rgba.len());
    }

    #[test]
    fn the_lamps_are_struck_from_the_derived_font_colour_not_the_stored_hex() {
        // `crt::params::screen_colors`' chrome half. At the shipped contrast
        // of 0.8 the stored "#ff8100" is a fifth too bright, which is the
        // whole reason this derivation exists.
        let cfg = Config::default();
        let stored = color::str_to_color(&cfg.screen.font_color);
        let derived = font_color(&cfg);
        assert_ne!(derived, stored);
        assert!(derived.r < stored.r);
        // The lamp law then normalises to full brightness, so the *lit*
        // colour is near the hue's peak either way; what moves is dim/panel.
        let a = led::window_colors(derived, true, true);
        let b = led::window_colors(stored, true, true);
        assert!((a.lit.r - b.lit.r).abs() < 1e-3);
    }
}
