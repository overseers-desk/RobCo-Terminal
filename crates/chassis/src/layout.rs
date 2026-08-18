//! How the window divides into the bank's column and the screen well, and the
//! measures everything else in the crate reads off that division.
//!
//! The split is two rectangles: the screen well runs from the bank column's
//! right edge to the window's, and the bank column takes the window's left
//! edge for exactly the bank's own width. Neither ever moves for the other:
//! the bank's width follows the LED strip settings alone, so dragging the
//! window edge cannot move the screen well's left edge.
//!
//! One thing worth stating rather than leaving implicit: this crate draws no
//! menu bar inside the window, so the content area is the window. Anything
//! that later puts a bar back inside the window has to subtract it before
//! calling in here.

/// The least screen the well is ever given; the seam's travel stops here
/// too.
///
/// This is the one copy: the seam clamp and the window's minimum-size hint
/// are the same rule seen from two sides and the seam is the chassis's. The
/// unit is logical pixels. `app`'s hint is a `Size::Physical`, so it scales
/// on the way out through [`min_inner_size_physical`].
pub const CRT_MINIMUM_WIDTH: i32 = 320;

pub const MINIMUM_HEIGHT: i32 = 240;

/// The numerator of the screen-scale normalisation, and this crate's own
/// default window width.
const NORMALISATION_WIDTH: f64 = 1024.0;

/// A rectangle in window coordinates, logical pixels, y down. See
/// [`crate::cabinet`]'s module doc on which pixel and why.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    pub fn right(&self) -> f64 {
        self.x + self.width
    }
}

/// The window divided.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowLayout {
    /// The appliance's left column, chassis and bank together. Zero-width
    /// when no bank stands.
    pub bank: Rect,
    /// The screen well: everything the bank leaves. The frame is drawn to
    /// fill exactly this, and the glass sits inside the frame's own bezel
    /// margins.
    pub crt: Rect,
}

impl WindowLayout {
    /// Divide a window of `width` x `height` for a bank of `bank_width`.
    ///
    /// `bank_width` is zero when no bank stands: a hidden chassis is not a
    /// bank of width zero but no bank at all, and the well takes the whole
    /// window.
    pub fn new(width: f64, height: f64, bank_width: f64) -> Self {
        // The split cannot produce a negative width; a window narrower than
        // its own minimum hint (a window manager free to ignore the hint
        // can make one) would otherwise hand the well a negative width here.
        let bank_width = bank_width.clamp(0.0, width.max(0.0));
        WindowLayout {
            bank: Rect::new(0.0, 0.0, bank_width, height),
            crt: Rect::new(bank_width, 0.0, width - bank_width, height),
        }
    }

    /// The scale every curvature-bearing uniform is normalised by, so a
    /// frame drawn at 1024x768 and the same frame drawn on a wall keep their
    /// proportions.
    ///
    /// The `max(1, ...)` guards a window manager free to ignore
    /// `minimumWidth`, which can hand the region no size at all, and every
    /// consumer of the scale divides by it.
    ///
    /// Note which rectangle it measures. This is the *screen well*, not the
    /// window: widening the bank shrinks the well and so raises the scale,
    /// which is why the seam drag moves the frame's apparent curvature as well
    /// as its width.
    pub fn normalized_screen_scale(&self) -> f64 {
        NORMALISATION_WIDTH / (0.5 * self.crt.width + 0.5 * self.crt.height).max(1.0)
    }

    /// The chassis metal continues the frame's field leftwards, so it is
    /// drawn in the frame's coordinates rather than its own: one casting,
    /// not two that meet at a seam.
    ///
    /// Returns `(viewport_size, field_scale, field_offset)`, the three uniforms
    /// `chassis_metal.slang` takes for that continuation.
    pub fn chassis_field(&self) -> ([f32; 2], [f32; 2], [f32; 2]) {
        // The frame's own size, floored at one pixel so the divides below
        // are defined before the window has been laid out.
        let field_w = self.crt.width.max(1.0);
        let field_h = self.crt.height.max(1.0);
        let viewport = [field_w as f32, field_h as f32];
        let scale = [
            (self.bank.width / field_w) as f32,
            (self.bank.height / field_h) as f32,
        ];
        let offset = [
            ((self.bank.x - self.crt.x) / field_w) as f32,
            ((self.bank.y - self.crt.y) / field_h) as f32,
        ];
        (viewport, scale, offset)
    }
}

/// The window's least inner size for a bank of this width. The hint has to
/// follow the seam drag, which is what `app::shell::Shell::set_bank_width`
/// exists to do.
pub fn min_inner_size(bank_width: i32) -> (i32, i32) {
    (bank_width.max(0) + CRT_MINIMUM_WIDTH, MINIMUM_HEIGHT)
}

/// The same floor in the physical pixels winit's `Size::Physical` hint and
/// X11's `WM_NORMAL_HINTS` are measured in.
///
/// Everything goes in logical and the conversion happens here, at the one
/// point that knows which window is being hinted. That is deliberate: a shell
/// holds several windows and they need not share a scale factor, so a bank
/// width converted once for all of them would be wrong on the second monitor.
/// [`Cabinet::bank_width_physical`](crate::cabinet::Cabinet::bank_width_physical)
/// is the same conversion for a caller that wants the bank's own footprint in
/// device pixels: to draw it, rather than to hint with it.
///
/// At scale factor 1 this is [`min_inner_size`] exactly, which is the relation
/// `xtask verify`'s contract item 4 reads (it recovers the bank width as
/// `min_width - 320`).
pub fn min_inner_size_physical(bank_width: u32, scale_factor: f64) -> (u32, u32) {
    let scale = scale_factor.max(0.0);
    let (w, h) = min_inner_size(bank_width.min(i32::MAX as u32) as i32);
    (
        (w as f64 * scale).round() as u32,
        (h as f64 * scale).round() as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_well_takes_what_the_bank_leaves() {
        // The stock appliance at this crate's own default window size,
        // amber shell over LED strips.
        let l = WindowLayout::new(1024.0, 768.0, 184.0);
        assert_eq!(l.bank, Rect::new(0.0, 0.0, 184.0, 768.0));
        assert_eq!(l.crt, Rect::new(184.0, 0.0, 840.0, 768.0));
        assert_eq!(l.crt.x, l.bank.right());
        assert_eq!(l.bank.width + l.crt.width, 1024.0);
    }

    #[test]
    fn a_hidden_chassis_gives_the_well_the_whole_window() {
        let l = WindowLayout::new(1024.0, 768.0, 0.0);
        assert_eq!(l.crt, Rect::new(0.0, 0.0, 1024.0, 768.0));
        assert_eq!(l.bank.width, 0.0);
    }

    #[test]
    fn the_well_never_goes_negative() {
        // A window manager free to ignore the minimum-width hint can hand the
        // window less than the bank alone needs.
        let l = WindowLayout::new(100.0, 768.0, 184.0);
        assert_eq!(l.bank.width, 100.0);
        assert_eq!(l.crt.width, 0.0);
        // ...and the scale guard still yields a finite number.
        assert!(l.normalized_screen_scale().is_finite());
    }

    #[test]
    fn normalized_scale_measures_the_well_not_the_window() {
        // 1024 / (0.5 * 840 + 0.5 * 768).
        let l = WindowLayout::new(1024.0, 768.0, 184.0);
        assert_eq!(l.normalized_screen_scale(), 1024.0 / 804.0);

        // The same window with no bank measures a wider well and so scales lower.
        let bare = WindowLayout::new(1024.0, 768.0, 0.0);
        assert_eq!(bare.normalized_screen_scale(), 1024.0 / 896.0);
        assert!(bare.normalized_screen_scale() < l.normalized_screen_scale());

        // A well of exactly 1024x1024 is the fixed point: scale 1.0.
        let unit = WindowLayout::new(1024.0, 1024.0, 0.0);
        assert_eq!(unit.normalized_screen_scale(), 1.0);
    }

    #[test]
    fn the_scale_guard_survives_a_window_of_no_size() {
        // The `max(1, ...)` guard: every consumer divides by this.
        let l = WindowLayout::new(0.0, 0.0, 0.0);
        assert_eq!(l.normalized_screen_scale(), 1024.0);
    }

    #[test]
    fn the_chassis_field_continues_the_frame_leftwards() {
        let l = WindowLayout::new(1024.0, 768.0, 184.0);
        let (viewport, scale, offset) = l.chassis_field();
        // The field is the frame's rect, not the chassis's own.
        assert_eq!(viewport, [840.0, 768.0]);
        // The chassis covers 184/840 of that field's width and all of its height.
        assert_eq!(scale, [184.0 / 840.0, 1.0]);
        // ...and stands to the left of the field's origin, so the offset is negative.
        assert_eq!(offset, [-184.0 / 840.0, 0.0]);
        assert!(offset[0] < 0.0);
    }

    #[test]
    fn min_inner_size_tracks_the_bank() {
        assert_eq!(min_inner_size(0), (320, 240));
        assert_eq!(min_inner_size(184), (504, 240));
        // The width the seam is not allowed to cross is exactly the difference.
        let (w, _) = min_inner_size(184);
        assert_eq!(w - 184, CRT_MINIMUM_WIDTH);
    }

    #[test]
    fn the_physical_hint_is_the_logical_one_scaled() {
        // Contract item 4 recovers the bank width as `min_width - 320`, so at
        // scale 1 the two functions have to agree exactly.
        assert_eq!(min_inner_size_physical(184, 1.0), (504, 240));
        assert_eq!(min_inner_size_physical(0, 1.0), (320, 240));

        // On a doubled display both halves double: the bank the user sees is
        // the same 184 logical pixels wide, drawn across 368 device ones.
        assert_eq!(min_inner_size_physical(184, 2.0), (1008, 480));
        assert_eq!(min_inner_size_physical(184, 1.25), (630, 300));
    }
}
