//! The seam where the bank's plastic meets the screen well, and the drag that
//! moves it.
//!
//! Nothing is drawn there: the cursor's change of shape is the only tell. A
//! drag re-fits the LED strips at the character count nearest the hand, so
//! the seam travels in whole-character steps rather than following the
//! pointer continuously.
//!
//! **The drag is absolute, not accumulated.** Every motion recomputes the
//! count from where the pointer now stands against where the bank's right
//! edge now stands, rather than adding up deltas. The two framings agree
//! (what the formula measures *is* a delta, the one between the pointer and
//! the standing edge), but only the absolute form is self-correcting: the
//! strip width is rounded to whole pixels, so an accumulated drag would
//! collect the rounding residue and the seam would drift out from under the
//! hand over a long drag. This does not.
//!
//! # Wiring
//!
//! [`SeamDrag`] is shaped for `app::shell`'s `Surface` trait, one method each:
//!
//! | `Surface` method | call |
//! |---|---|
//! | `cursor_moved(position, _)` | [`SeamDrag::pointer_moved`] |
//! | `mouse_pressed(MouseButton::Left, position, _)` | [`SeamDrag::pointer_pressed`] |
//! | `mouse_released(MouseButton::Left, _, _)` | [`SeamDrag::pointer_released`] |
//! | `focus_changed(false)` | [`SeamDrag::pointer_released`] |
//!
//! [`SeamDrag::pointer_pressed`] answers whether it took the press; a press it
//! took is the seam's and must not also reach the grid. When
//! [`SeamDrag::pointer_moved`] returns a count, write it to the settings'
//! `general.led_characters`, rebuild the [`BankGeometry`], and hand the new
//! [`BankGeometry::implicit_width`] to `app::shell::Shell::set_bank_width` so
//! the window's minimum-size hint follows the seam.

use crate::bank::BankGeometry;

/// The grab strip leans into the moulding, taking 3 px of the LED windows'
/// clickable right edge.
pub const SEAM_GRAB_INSET: f64 = 3.0;

/// 3 px of window edge and 7 px of inert plastic.
pub const SEAM_GRAB_WIDTH: f64 = 10.0;

/// What the pointer should look like over the seam. Mapping this to the
/// window layer's own cursor icon (e.g. winit's `CursorIcon::EwResize`) is
/// the window layer's business, not the chassis's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SeamCursor {
    /// The pointer is elsewhere; whoever else wants the cursor may have it.
    #[default]
    Unclaimed,
    /// Over the grab strip, or dragging it.
    ResizeHorizontal,
}

/// Everything the drag law reads, gathered at the moment of the motion.
///
/// Rebuilt per motion rather than held, because a drag that lands a new
/// character count changes `geometry` and `led_characters` both, and the next
/// motion must measure against the new ones.
#[derive(Clone, Copy, Debug)]
pub struct SeamContext<'a> {
    /// The bank as it stands now.
    pub geometry: &'a BankGeometry,
    /// `general.led_characters` as it stands now.
    pub led_characters: i32,
    /// The display's `unit_width()`.
    pub unit_width: f64,
    /// The window's inner width in logical pixels -- the same pixel every
    /// other measure in this struct is in, and the one the pointer arrives on
    /// (`Cabinet::cursor_moved` passes `layout.crt.right()`, and
    /// `crate::layout` is logical throughout).
    pub window_width: f64,
    /// The width the screen well is never dragged under, in the same logical
    /// pixel as `window_width`: what the host's font needs for its floor grid
    /// (`Cabinet::well_floor`).
    pub well_floor_width: f64,
    /// The bank column's width, which is where the grab strip stands.
    ///
    /// In every standing layout this is `geometry.implicit_width`: the
    /// bank's own width *is* its implicit width. It is named separately
    /// because the two part company in the one state reached by hiding the
    /// bank rather than resizing it: a hidden chassis, where the column is
    /// nothing wide and the measures still exist. Taking the hit test off
    /// the column means that state needs no special case to come out right.
    pub bank_width: f64,
}

impl SeamContext<'_> {
    /// The character count for a pointer at `pointer_x`, clamped by both of
    /// the drag's rules.
    ///
    /// The upper clamp is the interesting one. It is not a fixed maximum: it is
    /// the count whose bank width would leave the screen well exactly its
    /// floor ([`SeamContext::well_floor_width`]), measured through the same
    /// `characters_for_width` the pointer goes through, so the two bounds are
    /// commensurable and `min` between them is meaningful. It therefore
    /// tightens as the window narrows, and a drag that has hit it will start
    /// moving again the moment the window is widened.
    ///
    /// The lower clamp is the display's own floor.
    pub fn characters_at(&self, pointer_x: f64) -> i32 {
        let at_pointer =
            self.geometry
                .characters_for_width(pointer_x, self.led_characters, self.unit_width);
        // What the window has room for.
        let at_limit = self.geometry.characters_for_width(
            self.window_width - self.well_floor_width,
            self.led_characters,
            self.unit_width,
        );
        // ...and never below what the display will hold.
        self.geometry.min_units.max(at_pointer.min(at_limit))
    }
}

/// The seam's interaction state: whether the pointer is over the grab strip and
/// whether it is holding it.
#[derive(Clone, Copy, Debug, Default)]
pub struct SeamDrag {
    over: bool,
    held: bool,
}

impl SeamDrag {
    pub fn new() -> Self {
        SeamDrag::default()
    }

    /// Whether `x` falls in the grab strip for a bank of `bank_width`.
    ///
    /// The strip straddles the boundary rather than sitting beside it: it takes
    /// 3 px of the LED windows' own clickable edge and 7 px of the frame's
    /// inert plastic, because the strips run to the boundary and there is no
    /// plate of the bank's own to grab.
    pub fn hit(x: f64, bank_width: f64) -> bool {
        let left = bank_width - SEAM_GRAB_INSET;
        x >= left && x < left + SEAM_GRAB_WIDTH
    }

    /// The cursor the seam claims, put on the grab area itself, so it changes
    /// on hover and not only while dragging: the shape is the only thing that
    /// tells the user the seam is there at all.
    pub fn cursor(&self) -> SeamCursor {
        if self.over || self.held {
            SeamCursor::ResizeHorizontal
        } else {
            SeamCursor::Unclaimed
        }
    }

    /// Whether a drag is in progress.
    pub fn is_dragging(&self) -> bool {
        self.held
    }

    /// A left button went down at `x`. Returns whether the seam took it; a
    /// press the seam took belongs to the seam and must not also reach the grid.
    ///
    /// The strip exists only while the chassis is shown, and answers to the
    /// left button alone. Pass `chassis_shown` through rather than hiding the
    /// check inside: a hidden chassis has no bank, so no width to measure the
    /// strip against.
    pub fn pointer_pressed(&mut self, x: f64, bank_width: f64, chassis_shown: bool) -> bool {
        self.held = chassis_shown && Self::hit(x, bank_width);
        self.held
    }

    /// The pointer moved to `x`. Returns the new character count when it
    /// differs from the standing one, and `None` otherwise: including on every
    /// motion that is not a drag.
    ///
    /// A motion with no button down only updates the cursor: the strip
    /// tracks the pointer for its shape, but moves nothing until it is held.
    /// Once held, the pointer is free to leave the strip and the drag
    /// continues -- an ordinary mouse-grab convention, and the reason
    /// hit-testing is not repeated below.
    pub fn pointer_moved(
        &mut self,
        x: f64,
        ctx: &SeamContext<'_>,
        chassis_shown: bool,
    ) -> Option<i32> {
        self.over = chassis_shown && Self::hit(x, ctx.bank_width);
        if !self.held {
            return None;
        }
        let chars = ctx.characters_at(x);
        // A drag that lands on the count already set writes nothing.
        (chars != ctx.led_characters).then_some(chars)
    }

    /// The button came up, or the window lost focus with it still down. Either
    /// way the seam is no longer held.
    pub fn pointer_released(&mut self) {
        self.held = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bank::ChannelIndicator;
    use crate::metrics::{shells, DisplayMetrics, LedMetrics};

    /// The stock appliance: amber shell, LED strips, twelve characters, glow.
    /// Its bank is 184 px wide and one character is 7.5 px
    /// (see `bank::tests::annunciator_led_measures_follow_the_defining_formula_chain`).
    fn stock() -> (BankGeometry, LedMetrics) {
        let led = LedMetrics::default();
        let g = BankGeometry::new(&shells::annunciator(), &led, 12, ChannelIndicator::Glow);
        (g, led)
    }

    fn ctx<'a>(g: &'a BankGeometry, led: &LedMetrics, window_width: f64) -> SeamContext<'a> {
        SeamContext {
            geometry: g,
            led_characters: 12,
            unit_width: led.unit_width(),
            window_width,
            well_floor_width: crate::layout::CRT_MINIMUM_WIDTH as f64,
            bank_width: g.implicit_width as f64,
        }
    }

    #[test]
    fn the_grab_strip_straddles_the_boundary() {
        // x from bank_width - 3, ten wide.
        assert!(!SeamDrag::hit(180.0, 184.0));
        assert!(SeamDrag::hit(181.0, 184.0)); // 3 px of the LED windows' edge
        assert!(SeamDrag::hit(184.0, 184.0)); // the boundary itself
        assert!(SeamDrag::hit(190.0, 184.0)); // 7 px of inert plastic
        assert!(!SeamDrag::hit(191.0, 184.0));
    }

    #[test]
    fn the_cursor_changes_on_hover_not_only_on_drag() {
        let (g, led) = stock();
        let c = ctx(&g, &led, 1024.0);
        let mut seam = SeamDrag::new();
        assert_eq!(seam.cursor(), SeamCursor::Unclaimed);

        // Hovering the strip claims the cursor, and moves nothing.
        assert_eq!(seam.pointer_moved(184.0, &c, true), None);
        assert_eq!(seam.cursor(), SeamCursor::ResizeHorizontal);

        // Moving off it gives the cursor back.
        assert_eq!(seam.pointer_moved(400.0, &c, true), None);
        assert_eq!(seam.cursor(), SeamCursor::Unclaimed);
    }

    #[test]
    fn the_grab_strip_stands_on_the_column_not_on_the_measures() {
        // A hidden chassis has a bank column of no width and a bank whose
        // measures are all still there. Because the strip is placed off the
        // column, it lands at the window's left edge and nowhere near the
        // strips -- so even with the `chassis_shown` guard taken away, a
        // pointer out at the old boundary is not on the seam.
        let (g, led) = stock();
        assert_eq!(g.implicit_width, 184);
        assert!(!SeamDrag::hit(184.0, 0.0));
        let mut seam = SeamDrag::new();
        let c = SeamContext {
            geometry: &g,
            led_characters: 12,
            unit_width: led.unit_width(),
            window_width: 1024.0,
            well_floor_width: crate::layout::CRT_MINIMUM_WIDTH as f64,
            bank_width: 0.0,
        };
        assert_eq!(seam.pointer_moved(184.0, &c, true), None);
        assert_eq!(seam.cursor(), SeamCursor::Unclaimed);
    }

    #[test]
    fn a_hidden_chassis_has_no_seam() {
        let (g, led) = stock();
        let c = ctx(&g, &led, 1024.0);
        let mut seam = SeamDrag::new();
        assert!(!seam.pointer_pressed(184.0, 184.0, false));
        assert_eq!(seam.pointer_moved(184.0, &c, false), None);
        assert_eq!(seam.cursor(), SeamCursor::Unclaimed);
    }

    #[test]
    fn a_drag_moves_in_whole_characters() {
        let (g, led) = stock();
        let c = ctx(&g, &led, 1024.0);
        let mut seam = SeamDrag::new();
        assert!(seam.pointer_pressed(184.0, 184.0, true));
        assert!(seam.is_dragging());

        // Standing still writes nothing: the count is the one already set.
        assert_eq!(seam.pointer_moved(184.0, &c, true), None);
        // A third of a character is still nothing.
        assert_eq!(seam.pointer_moved(186.5, &c, true), None);
        // Past the half-character mark, one character.
        assert_eq!(seam.pointer_moved(188.0, &c, true), Some(13));
        // Four characters out.
        assert_eq!(seam.pointer_moved(184.0 + 30.0, &c, true), Some(16));
        // ...and back in the other direction.
        assert_eq!(seam.pointer_moved(184.0 - 30.0, &c, true), Some(8));
    }

    #[test]
    fn the_drag_continues_once_the_pointer_leaves_the_strip() {
        // An ordinary mouse-grab convention: the strip is 10 px wide, but a
        // drag is not confined to it. Without this the seam would stick
        // after the first few pixels.
        let (g, led) = stock();
        let c = ctx(&g, &led, 1024.0);
        let mut seam = SeamDrag::new();
        seam.pointer_pressed(184.0, 184.0, true);
        assert!(!SeamDrag::hit(300.0, 184.0));
        assert_eq!(seam.pointer_moved(300.0, &c, true), Some(12 + 15));
        assert_eq!(seam.cursor(), SeamCursor::ResizeHorizontal);
    }

    #[test]
    fn motion_without_a_press_moves_nothing() {
        let (g, led) = stock();
        let c = ctx(&g, &led, 1024.0);
        let mut seam = SeamDrag::new();
        assert_eq!(seam.pointer_moved(300.0, &c, true), None);
        // ...and a release mid-drag stops it there.
        seam.pointer_pressed(184.0, 184.0, true);
        assert_eq!(seam.pointer_moved(300.0, &c, true), Some(27));
        seam.pointer_released();
        assert!(!seam.is_dragging());
        assert_eq!(seam.pointer_moved(400.0, &c, true), None);
    }

    #[test]
    fn a_press_off_the_strip_is_not_the_seams() {
        let (g, led) = stock();
        let c = ctx(&g, &led, 1024.0);
        let mut seam = SeamDrag::new();
        assert!(!seam.pointer_pressed(400.0, 184.0, true));
        assert!(!seam.is_dragging());
        assert_eq!(seam.pointer_moved(500.0, &c, true), None);
    }

    #[test]
    fn the_floor_is_the_displays_own() {
        // The LED panel holds eight characters at the least, so dragging to
        // the window's left edge and beyond stops there rather than
        // collapsing the strip.
        let (g, led) = stock();
        let c = ctx(&g, &led, 1024.0);
        assert_eq!(g.min_units, 8);
        assert_eq!(c.characters_at(0.0), 8);
        assert_eq!(c.characters_at(-500.0), 8);
        // Just above the floor the drag still tracks: 184 - 4 * 7.5 = 154.
        assert_eq!(c.characters_at(154.0), 8);
        assert_eq!(c.characters_at(161.5), 9);
    }

    #[test]
    fn the_ceiling_is_what_the_window_has_room_for() {
        // The count whose bank leaves the well exactly 320 px. In a 1024 px
        // window that is a bank of 704 px, which is (704 - 184) / 7.5 =
        // 69.33 characters past the standing twelve, so round() gives 69:
        // eighty-one characters.
        let (g, led) = stock();
        let c = ctx(&g, &led, 1024.0);
        // A deliberately wild pointer position (further right than any window
        // ever is), so what comes back is the clamp and nothing else.
        let ceiling = c.characters_at(f64::MAX / 4.0);
        assert_eq!(ceiling, 81);
        assert_eq!(c.characters_at(704.0), 81);
        // Dragging past it pins there rather than pushing the well under its floor.
        assert_eq!(c.characters_at(900.0), ceiling);

        // The ceiling is not fixed: it follows the window. A narrower window
        // pulls it in...
        let narrow = ctx(&g, &led, 700.0);
        assert!(narrow.characters_at(f64::MAX / 4.0) < ceiling);
        // ...and a wider one lets it back out, so a drag pinned at the edge
        // starts moving again the moment the window is widened.
        let wide = ctx(&g, &led, 1920.0);
        assert!(wide.characters_at(f64::MAX / 4.0) > ceiling);
    }

    #[test]
    fn the_two_clamps_can_cross_and_the_floor_wins() {
        // A window too narrow to hold even the least strip: the ceiling
        // falls below the floor, and the `max(min_units, min(...))`
        // ordering means the display's floor is what survives. The
        // window's own minimum hint is what normally keeps this off the
        // screen, but a window manager is free to ignore it.
        let (g, led) = stock();
        let c = ctx(&g, &led, 340.0);
        assert!(c.characters_at(f64::MAX / 4.0) >= 8);
        assert_eq!(c.characters_at(1000.0), 8);
        assert_eq!(c.characters_at(0.0), 8);
    }

    #[test]
    fn a_drag_does_not_drift_over_its_length() {
        // The absolute law, exercised the way a hand exercises it: fifty
        // motions across the window, each measured against the geometry the
        // previous one produced. If the count ever disagreed with a single
        // measurement taken from the same pointer position, the seam would be
        // drifting out from under the hand.
        let led = LedMetrics::default();
        let shell = shells::annunciator();
        let mut characters = 12;
        let mut geometry = BankGeometry::new(&shell, &led, characters, ChannelIndicator::Glow);
        let mut seam = SeamDrag::new();
        seam.pointer_pressed(
            geometry.implicit_width as f64,
            geometry.implicit_width as f64,
            true,
        );

        for step in 0..50 {
            let x = 184.0 + step as f64 * 6.0;
            let c = SeamContext {
                geometry: &geometry,
                led_characters: characters,
                unit_width: led.unit_width(),
                window_width: 1400.0,
                well_floor_width: crate::layout::CRT_MINIMUM_WIDTH as f64,
                bank_width: geometry.implicit_width as f64,
            };
            if let Some(next) = seam.pointer_moved(x, &c, true) {
                characters = next;
                geometry = BankGeometry::new(&shell, &led, characters, ChannelIndicator::Glow);
            }
            // The seam's right edge tracks the hand to under one character.
            let error = (geometry.implicit_width as f64 - x).abs();
            assert!(
                error <= led.unit_width(),
                "seam drifted {error} px from the pointer at x={x} (step {step})"
            );
        }
        // ...and it did move, so the assertion above was not vacuous.
        assert!(characters > 12);
    }
}
