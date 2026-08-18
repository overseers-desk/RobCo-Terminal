//! The whole cabinet as one object: the composition order from this crate's
//! module doc, held together so a host does not have to keep it straight.
//!
//! Everything under it is usable on its own: the geometry, the layout and the
//! seam are plain values and pure functions, and stay that way. This is the
//! assembled form, shaped so that a `app::shell::Surface` implementation
//! forwards to it method for method and does no chassis arithmetic itself.
//!
//! # Wiring a `Surface` to it
//!
//! ```ignore
//! impl Surface for TerminalSurface {
//!     fn resized(&mut self, size: PhysicalSize<u32>) {
//!         let logical: LogicalSize<f64> = size.to_logical(self.scale_factor);
//!         self.cabinet.resized(logical.width, logical.height);
//!         // ...the rest of the resize
//!     }
//!
//!     fn cursor_moved(&mut self, position: PhysicalPosition<f64>, _: ModifiersState) {
//!         let x = position.x / self.scale_factor;
//!         if let Some(update) = self.cabinet.cursor_moved(x) {
//!             self.settings.set_led_characters(update.led_characters);
//!             self.shell.set_bank_width(update.bank_width);
//!             self.window.set_cursor(CursorIcon::EwResize);
//!             return;
//!         }
//!         // ...the grid's own pointer handling
//!     }
//!
//!     fn mouse_pressed(&mut self, button: MouseButton, position: PhysicalPosition<f64>, _: ModifiersState) {
//!         let x = position.x / self.scale_factor;
//!         if button == MouseButton::Left && self.cabinet.pointer_pressed(x) {
//!             return; // the press was the seam's
//!         }
//!         // ...the grid's own press handling
//!     }
//!
//!     fn mouse_released(&mut self, _: MouseButton, _: PhysicalPosition<f64>, _: ModifiersState) {
//!         self.cabinet.pointer_released();
//!     }
//!
//!     fn focus_changed(&mut self, focused: bool) {
//!         if !focused {
//!             self.cabinet.pointer_released();
//!         }
//!     }
//! }
//! ```
//!
//! # Which pixel
//!
//! Every measure in this crate is in the **logical** pixel, which winit
//! calls the logical pixel and other UI toolkits call the
//! device-independent pixel. The shells' constants are read off mocks at 1x
//! (a 45 px row pitch, a 3 px shoulder, the 320 px the well is never given
//! less than), and on a 2x display each of them is drawn at twice the
//! physical size, because these coordinates are logical throughout.
//!
//! `app::shell`'s `Surface` delivers physical pixels: `PhysicalSize`,
//! `PhysicalPosition`. So a host on a scaled display divides by the window's
//! scale factor on the way in. On the way out it does not have to: the
//! minimum-size hint is converted per window inside
//! `chassis::layout::min_inner_size_physical`, because a shell holds several
//! windows and they need not share a factor.
//! [`Cabinet::bank_width_physical`] is the multiplication for the other
//! direction of the same question: the bank's footprint in device pixels, for
//! whoever draws it. On an unscaled display the factor is 1.0 and none of this
//! is visible, which is exactly why it is written down here.
//!
//! # Two obligations
//!
//! Neither is the chassis's to discharge. [`SeamUpdate::led_characters`] has
//! to reach the settings, because the seam is a settings edit and the
//! configuration file is the source of truth; and
//! [`SeamUpdate::bank_width`] has to reach `Shell::set_bank_width`, because the
//! window's minimum-size hint is what stops the well being dragged under its
//! floor from the other side. The cabinet updates its own copy of the character
//! count as it goes, so a settings reload that arrives later carrying the same
//! value is a no-op rather than a jump.

use crate::bank::BankGeometry;
use crate::frame::{self, ChassisStyle, FrameStyle, Param};
use crate::layout::WindowLayout;
use crate::metrics::{DisplayMetrics, LedMetrics, ShellMetrics, TapeMetrics};
use crate::seam::{SeamContext, SeamCursor, SeamDrag};
use config::Config;

/// Which display kit the strips are cut from.
///
/// The kit is a measured value rather than a profile: the tape's one
/// character is an advance of Departure Mono at 20 px and the LED's is the
/// profile lamp font's cell. Keeping it a value is what lets every measure
/// under this module be tested with no font stack at all.
/// [`crate::display_kit`] is the one place a `Config` becomes one, and
/// [`Cabinet::from_config`] the constructor that goes through it, so a host
/// with a profile and a window never handles a kit itself.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Display {
    Led(LedMetrics),
    Tape(TapeMetrics),
}

impl DisplayMetrics for Display {
    fn unit_width(&self) -> f64 {
        match self {
            Display::Led(d) => d.unit_width(),
            Display::Tape(d) => d.unit_width(),
        }
    }

    fn min_units(&self) -> i32 {
        match self {
            Display::Led(d) => d.min_units(),
            Display::Tape(d) => d.min_units(),
        }
    }

    fn width_for_units(&self, n: i32) -> i32 {
        match self {
            Display::Led(d) => d.width_for_units(n),
            Display::Tape(d) => d.width_for_units(n),
        }
    }

    fn height_for_pad_cells(&self, pad_cells: i32) -> i32 {
        match self {
            Display::Led(d) => d.height_for_pad_cells(pad_cells),
            Display::Tape(d) => d.height_for_pad_cells(pad_cells),
        }
    }

    fn pad_cells_for_hole(&self, hole_height: i32) -> i32 {
        match self {
            Display::Led(d) => d.pad_cells_for_hole(hole_height),
            Display::Tape(d) => d.pad_cells_for_hole(hole_height),
        }
    }
}

/// What a drag landed, and the two places it has to go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeamUpdate {
    /// The new `general.led_characters`, for the settings.
    pub led_characters: i32,
    /// The new bank width in logical pixels, which is what
    /// `app::shell::Shell::set_bank_width` takes: the shell holds several
    /// windows and they need not share a scale factor, so the conversion
    /// happens per window at the hint site
    /// (`chassis::layout::min_inner_size_physical`). Use
    /// [`Cabinet::bank_width_physical`] for the other question: the bank's
    /// own footprint in device pixels, for drawing it.
    pub bank_width: u32,
}

/// The assembled cabinet.
#[derive(Clone, Debug)]
pub struct Cabinet {
    cfg: Config,
    display: Display,
    shell: ShellMetrics,
    frame_style: FrameStyle,
    chassis_style: ChassisStyle,
    geometry: BankGeometry,
    layout: WindowLayout,
    seam: SeamDrag,
}

impl Cabinet {
    /// Build for a profile, a display kit and a window size in logical pixels
    /// (see the module doc on which pixel, and why it matters only on a scaled
    /// display).
    pub fn new(cfg: &Config, display: Display, width: f64, height: f64) -> Self {
        let shell = crate::shell_metrics(cfg);
        let geometry = Self::measure(cfg, &shell, &display);
        let layout = WindowLayout::new(width, height, Self::footprint(cfg, &geometry));
        Cabinet {
            cfg: cfg.clone(),
            display,
            shell,
            frame_style: crate::frame_style(cfg),
            chassis_style: crate::chassis_style(cfg),
            geometry,
            layout,
            seam: SeamDrag::new(),
        }
    }

    /// Build for a profile alone, measuring the display kit off the font stack
    /// ([`crate::display_kit`]) rather than taking one.
    ///
    /// This is the constructor a host uses: it has a `Config` and a window, and
    /// no reason to know that one of the two kits reads `general.led_font_name`
    /// and the other carries its own face.
    pub fn from_config(cfg: &Config, width: f64, height: f64) -> Self {
        Cabinet::new(cfg, crate::display_kit(cfg), width, height)
    }

    fn measure(cfg: &Config, shell: &ShellMetrics, display: &Display) -> BankGeometry {
        BankGeometry::new(
            shell,
            display,
            cfg.general.led_characters,
            crate::channel_indicator(cfg),
        )
    }

    /// The bank's footprint, zero when no bank stands. A hidden chassis is
    /// not a bank of no width; it is no bank, and the well takes the whole
    /// window.
    fn footprint(cfg: &Config, geometry: &BankGeometry) -> f64 {
        if cfg.general.chassis_shown {
            geometry.implicit_width as f64
        } else {
            0.0
        }
    }

    /// A settings reload, or a display kit re-measured after a font change.
    /// Returns the bank width to hand `Shell::set_bank_width`.
    pub fn apply_settings(&mut self, cfg: &Config, display: Display) -> u32 {
        self.cfg = cfg.clone();
        self.display = display;
        self.shell = crate::shell_metrics(cfg);
        self.frame_style = crate::frame_style(cfg);
        self.chassis_style = crate::chassis_style(cfg);
        self.remeasure();
        self.bank_width()
    }

    /// A settings reload, re-measuring the display kit from the new profile.
    /// The [`Cabinet::from_config`] half of [`Cabinet::apply_settings`], and
    /// the one a host that never handles a kit itself calls: a change of
    /// `general.led_font_name` or of `chassis.channel_display` moves the cell
    /// the strips are cut from, so the kit has to be re-measured and not only
    /// re-applied.
    pub fn apply_config(&mut self, cfg: &Config) -> u32 {
        self.apply_settings(cfg, crate::display_kit(cfg))
    }

    /// The window changed size, in logical pixels. The bank does not move,
    /// but the well does, and the well is what every curvature-bearing
    /// uniform is scaled by.
    pub fn resized(&mut self, width: f64, height: f64) {
        self.layout = WindowLayout::new(width, height, self.layout.bank.width);
    }

    fn remeasure(&mut self) {
        self.geometry = Self::measure(&self.cfg, &self.shell, &self.display);
        self.layout = WindowLayout::new(
            self.layout.crt.right(),
            self.layout.bank.height,
            Self::footprint(&self.cfg, &self.geometry),
        );
    }

    /// The bank's footprint in logical pixels.
    pub fn bank_width(&self) -> u32 {
        self.layout.bank.width.max(0.0) as u32
    }

    /// The bank's footprint in physical pixels, which is the unit
    /// `Window::set_min_inner_size` measures a `Size::Physical` in.
    pub fn bank_width_physical(&self, scale_factor: f64) -> u32 {
        (self.layout.bank.width.max(0.0) * scale_factor.max(0.0)).round() as u32
    }

    /// The window's least inner size for the bank as it now stands, in
    /// logical pixels.
    pub fn min_inner_size(&self) -> (i32, i32) {
        crate::layout::min_inner_size(self.bank_width() as i32)
    }

    pub fn geometry(&self) -> &BankGeometry {
        &self.geometry
    }

    pub fn layout(&self) -> &WindowLayout {
        &self.layout
    }

    /// Whether the chassis is drawn at all.
    pub fn is_shown(&self) -> bool {
        self.cfg.general.chassis_shown
    }

    /// The uniforms for the bezel over the screen well.
    pub fn frame_params(&self) -> Vec<Param> {
        frame::frame_params(&self.frame_style, &self.shell, &self.cfg, &self.layout)
    }

    /// The uniforms for the casting under the bank column.
    pub fn chassis_params(&self) -> Vec<Param> {
        frame::chassis_params(&self.chassis_style, &self.shell, &self.layout)
    }

    /// How many rows this bank shows, the shell's own pager taken off its
    /// foot first.
    ///
    /// Zero for a bank that is not shown, and zero for the pre-load beat the
    /// arithmetic refuses (`BankGeometry::rows_visible`), so a caller can
    /// always iterate the answer.
    pub fn rows_visible(&self) -> i32 {
        if !self.is_shown() {
            return 0;
        }
        self.geometry
            .rows_visible(self.layout.bank.height as i32, self.pager_height())
            .unwrap_or(0)
    }

    /// What the shell's pager takes off the bank's foot before the rows
    /// divide what is left.
    ///
    /// Public because the row-count settle lives in the app (it owns the
    /// channel model the count feeds), and it measures against the window's own
    /// height rather than the layout's; it still has to subtract the same
    /// footprint this cabinet's own [`Cabinet::rows_visible`] does, and one
    /// answer is the point.
    pub fn pager_height(&self) -> i32 {
        let content_width = self.geometry.content_width(self.bank_width() as i32);
        self.shell.pager_height(content_width)
    }

    /// What stands on the casting: the shell's plate and one channel display
    /// per row, in draw order, in the bank column's own logical pixels.
    ///
    /// The step 4 of this crate's composition order that the mount owns, and
    /// the one that takes an argument from outside the chassis:
    /// [`crate::BankStrips`] is the channel model's half. See
    /// [`crate::furniture`] for that seam and for what is drawn here.
    pub fn furniture(&self, strips: &crate::BankStrips) -> Vec<crate::Piece> {
        if !self.is_shown() {
            return Vec::new();
        }
        crate::furniture::bank_pieces(
            &self.cfg,
            &self.shell,
            &self.display,
            &self.geometry,
            (self.layout.bank.width, self.layout.bank.height),
            strips,
        )
    }

    /// Which row's window a press at `(x, y)` landed in, both in the bank
    /// column's own logical pixels, for a page of `rows` engraved keys.
    ///
    /// The hit test belongs to whoever drew the windows, which is why it is
    /// here and not in the channel model; what to *do* about the press is
    /// `app::bank::BankPager::press`'s (open a dark slot, select an open one).
    pub fn strip_at(&self, rows: usize, x: f64, y: f64) -> Option<usize> {
        if !self.is_shown() {
            return None;
        }
        crate::furniture::strip_at(&self.geometry, &self.shell, rows, x, y)
    }

    /// The cursor the seam claims, if any.
    pub fn cursor(&self) -> SeamCursor {
        self.seam.cursor()
    }

    /// A left button went down at window x, in logical pixels. Returns whether
    /// the seam took it; a press the seam took must not also reach the grid.
    pub fn pointer_pressed(&mut self, x: f64) -> bool {
        self.seam
            .pointer_pressed(x, self.layout.bank.width, self.is_shown())
    }

    /// The button came up, or the window lost focus with it still down.
    pub fn pointer_released(&mut self) {
        self.seam.pointer_released();
    }

    /// The pointer moved to window x, in logical pixels. Returns an update when
    /// the drag landed a new character count.
    pub fn cursor_moved(&mut self, x: f64) -> Option<SeamUpdate> {
        let ctx = SeamContext {
            geometry: &self.geometry,
            led_characters: self.cfg.general.led_characters,
            unit_width: self.display.unit_width(),
            window_width: self.layout.crt.right(),
            bank_width: self.layout.bank.width,
        };
        let shown = self.cfg.general.chassis_shown;
        let chars = self.seam.pointer_moved(x, &ctx, shown)?;
        // Take the new count on the spot. The settings write is the host's, and
        // the reload that follows it is a round trip: measuring the next motion
        // against the old geometry until it came back would put the seam a
        // frame behind the hand, and at speed several characters behind.
        self.cfg.general.led_characters = chars;
        self.remeasure();
        Some(SeamUpdate {
            led_characters: chars,
            bank_width: self.bank_width(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stock() -> Cabinet {
        Cabinet::new(
            &Config::default(),
            Display::Led(LedMetrics::default()),
            1024.0,
            768.0,
        )
    }

    #[test]
    fn the_assembled_cabinet_agrees_with_its_parts() {
        let c = stock();
        assert_eq!(c.bank_width(), 184);
        assert_eq!(c.layout().crt.width, 840.0);
        assert_eq!(c.min_inner_size(), (504, 240));
        assert_eq!(c.frame_params().len(), 36);
        assert_eq!(c.chassis_params().len(), 14);
    }

    #[test]
    fn a_hidden_chassis_gives_the_well_the_whole_window_and_no_seam() {
        let mut cfg = Config::default();
        cfg.general.chassis_shown = false;
        let mut c = Cabinet::new(&cfg, Display::Led(LedMetrics::default()), 1024.0, 768.0);
        assert_eq!(c.bank_width(), 0);
        assert_eq!(c.layout().crt.width, 1024.0);
        assert_eq!(c.min_inner_size(), (320, 240));
        // The bank's own measures still exist; nothing is drawn from them.
        assert_eq!(c.geometry().implicit_width, 184);
        assert!(!c.pointer_pressed(184.0));
        assert_eq!(c.cursor_moved(300.0), None);
        assert_eq!(c.cursor(), SeamCursor::Unclaimed);
    }

    #[test]
    fn the_bank_width_the_window_hint_wants_is_the_scaled_one() {
        // The shells' constants are logical: a 3 px shoulder is 3 px of mock,
        // and on a 2x display it is 6 px of screen. The hint takes physical.
        let c = stock();
        assert_eq!(c.bank_width(), 184);
        assert_eq!(c.bank_width_physical(1.0), 184);
        assert_eq!(c.bank_width_physical(2.0), 368);
        // A fractional factor is the ordinary case on a scaled desktop.
        assert_eq!(c.bank_width_physical(1.5), 276);
        assert_eq!(c.bank_width_physical(1.25), 230);
    }

    #[test]
    fn a_resize_moves_the_well_and_leaves_the_bank_alone() {
        let mut c = stock();
        let before = c.bank_width();
        c.resized(1920.0, 1080.0);
        assert_eq!(c.bank_width(), before);
        assert_eq!(c.layout().crt.width, 1920.0 - 184.0);
        assert_eq!(c.layout().crt.height, 1080.0);
        assert_eq!(c.layout().bank.height, 1080.0);
    }

    #[test]
    fn a_drag_updates_the_cabinet_before_the_settings_come_back() {
        let mut c = stock();
        assert!(c.pointer_pressed(184.0));
        let update = c
            .cursor_moved(300.0)
            .expect("a 116 px drag lands somewhere");
        // The count and the width move together, and the width is the one the
        // window's minimum-size hint needs.
        assert_eq!(update.led_characters, 27);
        assert_eq!(update.bank_width, c.bank_width());
        assert_eq!(c.geometry().implicit_width as u32, update.bank_width);
        // The well gave up exactly what the bank took.
        assert_eq!(c.layout().crt.right(), 1024.0);
        assert_eq!(c.layout().crt.width, 1024.0 - update.bank_width as f64);
        // A settings reload carrying what the drag just wrote is a no-op.
        let mut cfg = Config::default();
        cfg.general.led_characters = update.led_characters;
        let width = c.apply_settings(&cfg, Display::Led(LedMetrics::default()));
        assert_eq!(width, update.bank_width);
    }

    #[test]
    fn a_drag_across_the_window_tracks_the_hand_the_whole_way() {
        // The loop closed: press, then walk the pointer out and back, with the
        // cabinet re-measuring itself at every step. What is checked is that
        // the bank's edge stays under the hand -- the property that an
        // accumulating drag would lose to rounding residue.
        let mut c = stock();
        assert!(c.pointer_pressed(184.0));
        let unit = LedMetrics::default().unit_width();
        let mut xs: Vec<f64> = (0..40).map(|i| 184.0 + i as f64 * 12.0).collect();
        xs.extend((0..40).map(|i| 664.0 - i as f64 * 12.0));
        for x in xs {
            c.cursor_moved(x);
            let error = (c.bank_width() as f64 - x).abs();
            assert!(
                error <= unit,
                "seam drifted {error} px from the hand at {x}"
            );
        }
        // ...and the floor still holds when the hand runs off the left edge.
        c.cursor_moved(-500.0);
        assert_eq!(c.cfg.general.led_characters, 8);
    }

    #[test]
    fn the_display_kit_travels_with_the_profile() {
        let mut cfg = Config::default();
        cfg.chassis.shell = config::Shell::Switchboard;
        cfg.chassis.channel_indicator = config::ChannelIndicator::Switch;
        cfg.chassis.channel_display = config::ChannelDisplay::Tape;
        let tape = TapeMetrics {
            unit_width: (177.0 - 24.0) / 12.0,
            end_pad: 12,
            min_characters: 8,
        };
        let c = Cabinet::new(&cfg, Display::Tape(tape), 1440.0, 900.0);
        // 8 + 170 + 18 + 177 + 29, which is the mock's own plate running
        // x 8..402 to the screen well's trough.
        assert_eq!(c.bank_width(), 402);
        assert_eq!(c.layout().crt.width, 1440.0 - 402.0);
    }
}
