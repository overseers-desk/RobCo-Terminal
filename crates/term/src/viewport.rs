//! Scrollback viewport.
//!
//! rio-vt already owns the authoritative number (`Grid::display_offset`, in
//! lines above the bottom), and `Line` indexing folds it in, so this type is
//! deliberately not a second copy of that state. It is the policy layer: how a
//! wheel notch or a Shift-PageUp turns into a `Scroll`, and whether the view
//! follows new output. The rule that matters to the renderer is the last one:
//! a viewport that moved invalidates every line on screen, because rio-vt's
//! own damage is expressed in viewport coordinates and says nothing about the
//! rows that scrolled in from history.

use rio_vt::crosswords::grid::Scroll;
use rio_vt::crosswords::Crosswords;
use rio_vt::event::EventListener;

/// Lines per wheel notch.
pub const WHEEL_LINES: i32 = 3;

/// Where the view sits in the scrollback, and whether it follows output.
///
/// Named for the question it answers rather than for the module, because
/// [`crate::size::Viewport`] is the *other* viewport (a window's pixels
/// and cells), and the two arriving under one name is how the alias this
/// replaced came about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScrollPosition {
    offset: usize,
    /// True while the view is pinned to the bottom, so new output scrolls it.
    /// Set by reaching the bottom, cleared by scrolling up, exactly as every
    /// terminal behaves.
    follow: bool,
}

impl Default for ScrollPosition {
    fn default() -> Self {
        Self {
            offset: 0,
            follow: true,
        }
    }
}

impl ScrollPosition {
    /// Lines above the bottom of the scrollback that the top of the screen
    /// sits at. Zero means "showing live output".
    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn is_following(&self) -> bool {
        self.follow
    }

    /// Positive `delta` scrolls back into history, matching rio-vt's
    /// `Scroll::Delta` sign.
    pub fn scroll<L: EventListener>(&mut self, term: &mut Crosswords<L>, delta: i32) {
        self.apply(term, Scroll::Delta(delta));
    }

    pub fn scroll_wheel<L: EventListener>(&mut self, term: &mut Crosswords<L>, notches: i32) {
        self.scroll(term, notches * WHEEL_LINES);
    }

    pub fn page_up<L: EventListener>(&mut self, term: &mut Crosswords<L>) {
        self.apply(term, Scroll::PageUp);
    }

    pub fn page_down<L: EventListener>(&mut self, term: &mut Crosswords<L>) {
        self.apply(term, Scroll::PageDown);
    }

    pub fn to_top<L: EventListener>(&mut self, term: &mut Crosswords<L>) {
        self.apply(term, Scroll::Top);
    }

    pub fn to_bottom<L: EventListener>(&mut self, term: &mut Crosswords<L>) {
        self.apply(term, Scroll::Bottom);
    }

    fn apply<L: EventListener>(&mut self, term: &mut Crosswords<L>, scroll: Scroll) {
        term.scroll_display(scroll);
        self.offset = term.grid.display_offset();
        self.follow = self.offset == 0;
    }

    /// Re-read the offset the terminal actually holds. Output written while
    /// the view is pinned to the bottom leaves the offset at zero; output
    /// written while it is scrolled up moves it, because the history under the
    /// view grew. Either way the terminal is the authority and this is how the
    /// renderer hears about it.
    pub fn sync<L: EventListener>(&mut self, term: &Crosswords<L>) -> ViewportChange {
        let now = term.grid.display_offset();
        let moved = now != self.offset;
        self.offset = now;
        if now == 0 {
            self.follow = true;
        }
        ViewportChange { moved }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewportChange {
    /// The view is looking at different lines than last frame. Every row on
    /// screen is stale, whatever rio-vt's per-line damage says.
    pub moved: bool,
}
