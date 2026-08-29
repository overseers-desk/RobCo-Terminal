//! The size overlay: the transient "columns x rows" badge shown while a
//! window is being resized.
//!
//! A black rounded rectangle centred in the screen region, holding
//! `columns + "x" + rows`. The behavior in full:
//!
//! - a size change shows it at 0.5 opacity and (re)starts a 1 s timer;
//! - when the timer lapses it fades to 0 over 200 ms.
//!
//! It is gated by the `showTerminalSize` setting.
//!
//! This is a pure state machine over elapsed time, deliberately holding no
//! renderer: the shell feeds it size changes and the frame clock, and the
//! glyph renderer asks it for text and opacity. That is what
//! lets it be tested without a display, and what lets it merge with the
//! window's renderer code without either owning the other.
//!
//! One label detail worth keeping: a size type whose fields are named
//! generically (height/width, or lines/columns) makes printing "columns x
//! rows" a swap that is easy to get backwards. Here the field names say what
//! they mean and [`SizeOverlay::text`] prints columns x rows, the
//! conventional way round, directly off them -- no swap to get wrong.

use std::time::{Duration, Instant};

/// How long the badge stays at full strength after the last size change.
pub const HOLD: Duration = Duration::from_millis(1000);

/// How long the fade-out takes.
pub const FADE: Duration = Duration::from_millis(200);

/// The opacity the badge shows at while held.
pub const PEAK_OPACITY: f32 = 0.5;

/// A terminal's size in character cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GridSize {
    pub columns: u32,
    pub rows: u32,
}

/// The overlay's state for one window.
#[derive(Debug, Clone, Default)]
pub struct SizeOverlay {
    size: GridSize,
    /// Time elapsed since the last size change; `None` before the first
    /// one, which is why a window that has never been resized shows
    /// nothing.
    since_change: Option<Duration>,
    /// The `showTerminalSize` setting.
    enabled: bool,
}

impl SizeOverlay {
    /// A new overlay for a window, with the `showTerminalSize` setting.
    pub fn new(enabled: bool) -> Self {
        SizeOverlay {
            enabled,
            ..Default::default()
        }
    }

    /// Applies the `showTerminalSize` setting (it is live-reloadable, so
    /// this can change mid-run).
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// The grid size last reported.
    pub fn size(&self) -> GridSize {
        self.size
    }

    /// Reports the window's current grid size. Only a *change* restarts
    /// the timer: a resize that does not cross a cell boundary must not
    /// keep the badge alive.
    pub fn resized(&mut self, size: GridSize) {
        if self.since_change.is_none() {
            // The very first report is the window coming up, not a user
            // resizing it, but the badge would be invisible in practice
            // anyway because the window is not yet mapped. Record the
            // size as already faded out.
            self.size = size;
            self.since_change = Some(HOLD + FADE);
            return;
        }
        if self.size != size {
            self.size = size;
            self.since_change = Some(Duration::ZERO);
        }
    }

    /// Advances the clock by one frame's worth of wall time.
    pub fn tick(&mut self, delta: Duration) {
        if let Some(since) = self.since_change.as_mut() {
            // Saturate rather than grow forever: a window left alone for
            // an hour must not accumulate an hour of elapsed time.
            if *since < HOLD + FADE {
                *since = (*since + delta).min(HOLD + FADE);
            }
        }
    }

    /// The badge's opacity right now: 0.5 while held, fading linearly to
    /// 0 over the 200 ms after the 1 s hold, and 0 when the setting is
    /// off or no size has ever been reported.
    pub fn opacity(&self) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        let Some(since) = self.since_change else {
            return 0.0;
        };
        if since < HOLD {
            PEAK_OPACITY
        } else if since < HOLD + FADE {
            let into_fade = (since - HOLD).as_secs_f32() / FADE.as_secs_f32();
            PEAK_OPACITY * (1.0 - into_fade)
        } else {
            0.0
        }
    }

    /// Whether the renderer needs to draw it at all this frame.
    pub fn visible(&self) -> bool {
        self.opacity() > 0.0
    }

    /// The badge's text, columns first: `"80x24"`.
    pub fn text(&self) -> String {
        format!("{}x{}", self.size.columns, self.size.rows)
    }
}

/// How long a notice stays at full strength after the last thing that raised
/// it. Longer than the size badge's second, because a notice is news the user
/// did not ask for and did not see coming: the size badge answers a resize they
/// are in the middle of doing, and their eyes are already on the glass.
pub const NOTICE_HOLD: Duration = Duration::from_secs(3);

/// A notice fades out over the same 200 ms as the size badge ([`FADE`]).
/// There is one fade in this appliance and this is it; a second duration
/// would be a second answer to how fast things leave this screen.
pub const NOTICE_FADE: Duration = FADE;

/// A notice is drawn more strongly than the size badge's `opacity: 0.5`. The
/// size badge deliberately lets the grid read through it (it announces
/// something the user is doing); a notice says something was *lost*, and the
/// plate that says so has to be legible at a glance.
pub const NOTICE_OPACITY: f32 = 0.8;

/// The transient line the appliance puts on the glass when something happened
/// that the user would otherwise only find in a log.
///
/// The events it reports are native to this design: the queues that shed
/// here (`term::Session`'s input queue and `app::tmux::Gateway`'s command
/// queue, both capped) come from this appliance's own
/// non-blocking transports, and a profile save has nowhere else to report to
/// -- there is no settings window here to carry that acknowledgement. So the
/// placement and the wording are judged on their own terms: the mount is the
/// size badge's plate ([`crate::chrome::Badges`]), one slot below its own centred
/// position, and the words are short enough to read in the two seconds they
/// are up. See the call sites in `app::window` for each.
///
/// Time is carried as an [`Instant`] rather than accumulated deltas, unlike
/// [`SizeOverlay`]: a notice is raised by an event (a shed, a save) and sampled
/// by a frame, and those two clocks are the same wall clock. The size overlay
/// is fed the frame delta by the shell and keeps its own running sum instead;
/// nothing here needs a second clock shape.
#[derive(Debug, Clone, Default)]
pub struct Notice {
    text: String,
    /// When it was last raised. `None` before the first one.
    raised: Option<Instant>,
}

impl Notice {
    /// Put `text` on the glass, holding from `now`. Raising it again -- the same
    /// words or different ones -- restarts the hold, so a queue that goes on
    /// shedding keeps its notice lit rather than flickering once per event.
    pub fn raise(&mut self, text: impl Into<String>, now: Instant) {
        self.text = text.into();
        self.raised = Some(now);
    }

    /// What it says. Empty when nothing has ever been raised.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// How strongly it should be drawn at `now`: [`NOTICE_OPACITY`] while held,
    /// fading linearly to nothing over the [`NOTICE_FADE`] after that.
    pub fn opacity_at(&self, now: Instant) -> f32 {
        let Some(raised) = self.raised else {
            return 0.0;
        };
        if self.text.is_empty() {
            return 0.0;
        }
        // A clock that went backwards (an `Instant` from before the raise, which
        // a test can hand over and a frame cannot) reads as freshly raised
        // rather than as a fade already over.
        let since = now.saturating_duration_since(raised);
        if since < NOTICE_HOLD {
            NOTICE_OPACITY
        } else if since < NOTICE_HOLD + NOTICE_FADE {
            let into_fade = (since - NOTICE_HOLD).as_secs_f32() / NOTICE_FADE.as_secs_f32();
            NOTICE_OPACITY * (1.0 - into_fade)
        } else {
            0.0
        }
    }

    /// Whether the renderer needs to draw it at all this frame.
    pub fn visible_at(&self, now: Instant) -> bool {
        self.opacity_at(now) > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(columns: u32, rows: u32) -> GridSize {
        GridSize { columns, rows }
    }

    #[test]
    fn text_is_columns_by_rows() {
        let mut o = SizeOverlay::new(true);
        o.resized(grid(80, 24));
        assert_eq!(o.text(), "80x24");
    }

    #[test]
    fn nothing_shows_before_a_resize() {
        let o = SizeOverlay::new(true);
        assert!(!o.visible());
        // Nor does the window's first size report, which is the window
        // coming up rather than a user resizing it.
        let mut o = SizeOverlay::new(true);
        o.resized(grid(80, 24));
        assert!(!o.visible());
    }

    #[test]
    fn a_resize_shows_it_at_half_opacity_for_a_second() {
        let mut o = SizeOverlay::new(true);
        o.resized(grid(80, 24));
        o.resized(grid(100, 30));
        assert_eq!(o.opacity(), PEAK_OPACITY);
        o.tick(Duration::from_millis(999));
        assert_eq!(o.opacity(), PEAK_OPACITY);
    }

    #[test]
    fn it_fades_out_over_two_hundred_milliseconds() {
        let mut o = SizeOverlay::new(true);
        o.resized(grid(80, 24));
        o.resized(grid(100, 30));
        o.tick(HOLD);
        assert_eq!(o.opacity(), PEAK_OPACITY);
        o.tick(Duration::from_millis(100));
        assert!(
            (o.opacity() - PEAK_OPACITY / 2.0).abs() < 1e-6,
            "{}",
            o.opacity()
        );
        o.tick(Duration::from_millis(100));
        assert_eq!(o.opacity(), 0.0);
        assert!(!o.visible());
    }

    #[test]
    fn a_further_resize_restarts_the_hold() {
        let mut o = SizeOverlay::new(true);
        o.resized(grid(80, 24));
        o.resized(grid(100, 30));
        o.tick(HOLD + FADE);
        assert!(!o.visible());
        o.resized(grid(101, 30));
        assert_eq!(o.opacity(), PEAK_OPACITY);
    }

    /// A resize that lands on the same cell count must not keep the badge
    /// alive: only a change in cell count should restart the hold.
    #[test]
    fn a_resize_within_the_same_cell_count_does_not_restart_it() {
        let mut o = SizeOverlay::new(true);
        o.resized(grid(80, 24));
        o.resized(grid(100, 30));
        o.tick(HOLD + FADE);
        o.resized(grid(100, 30));
        assert!(!o.visible());
    }

    #[test]
    fn the_setting_gates_it_entirely() {
        let mut o = SizeOverlay::new(false);
        o.resized(grid(80, 24));
        o.resized(grid(100, 30));
        assert!(!o.visible());
        o.set_enabled(true);
        assert!(o.visible());
    }

    #[test]
    fn a_notice_shows_nothing_until_it_is_raised() {
        let notice = Notice::default();
        let now = Instant::now();
        assert_eq!(notice.text(), "");
        assert!(!notice.visible_at(now));
    }

    #[test]
    fn a_raised_notice_holds_then_fades() {
        let mut notice = Notice::default();
        let raised = Instant::now();
        notice.raise("input dropped", raised);
        assert_eq!(notice.text(), "input dropped");
        assert_eq!(notice.opacity_at(raised), NOTICE_OPACITY);
        assert_eq!(
            notice.opacity_at(raised + NOTICE_HOLD - Duration::from_millis(1)),
            NOTICE_OPACITY
        );
        let half = notice.opacity_at(raised + NOTICE_HOLD + NOTICE_FADE / 2);
        assert!(
            (half - NOTICE_OPACITY / 2.0).abs() < 1e-6,
            "halfway through the fade it reads {half}"
        );
        assert_eq!(notice.opacity_at(raised + NOTICE_HOLD + NOTICE_FADE), 0.0);
        assert!(!notice.visible_at(raised + NOTICE_HOLD + NOTICE_FADE));
    }

    /// A queue that goes on shedding raises the notice again; the badge has to
    /// stay lit rather than fade under a fault that is still happening.
    #[test]
    fn raising_it_again_restarts_the_hold() {
        let mut notice = Notice::default();
        let first = Instant::now();
        notice.raise("input dropped", first);
        let late = first + NOTICE_HOLD + NOTICE_FADE;
        assert!(!notice.visible_at(late));
        notice.raise("input dropped", late);
        assert_eq!(notice.opacity_at(late), NOTICE_OPACITY);
    }

    #[test]
    fn idle_time_does_not_accumulate_without_bound() {
        let mut o = SizeOverlay::new(true);
        o.resized(grid(80, 24));
        o.resized(grid(100, 30));
        for _ in 0..1000 {
            o.tick(Duration::from_secs(1));
        }
        assert_eq!(o.opacity(), 0.0);
        o.resized(grid(120, 40));
        assert_eq!(o.opacity(), PEAK_OPACITY);
    }
}
