//! What the input method has told this window so far, and what the window
//! tells it back.
//!
//! Three fields are the composition itself -- `enabled`, `preedit`, `cursor`
//! -- and the fourth, `area`, is the caret rectangle the platform was last
//! given. Nothing else here touches the surface: the bytes a commit produced
//! are handed back for the caller to type at its child, and the rectangle is
//! computed by the surface (`window::TerminalSurface::ime_cursor_area`,
//! which reads the viewport, the scroll position and the distortion) and
//! passed in.

use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::Ime;
use winit::window::Window;

/// What the input method has told this surface so far.
///
/// Deliberately small. The commit is written straight through and kept nowhere;
/// what is held is what a caller may need to *ask* about -- whether composition
/// is open, and what the half-typed word is. The frame reads the word every
/// redraw and draws it at the cursor, so this is the composition's one home
/// rather than a copy of one.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ImeState {
    /// The input method has taken the keyboard: `Ime::Enabled` arrived and no
    /// `Ime::Disabled` has since.
    pub enabled: bool,
    /// The uncommitted composition string, `""` when there is none.
    pub preedit: String,
    /// The cursor/selection inside the pre-edit, in byte offsets, as winit
    /// reports it; `None` is the input method asking for no caret inside the
    /// composition.
    ///
    /// Kept and not drawn: the whole pre-edit is painted as a single block, so
    /// this offset is stored for a caller that might want it but is not
    /// consulted to draw the composition.
    pub cursor: Option<(usize, usize)>,
    /// The caret rectangle the input method was last told about, in whole
    /// physical pixels, so a caret that has not moved is not republished every
    /// turn of the loop. See [`ImeState::publish`].
    area: Option<(i32, i32, i32, i32)>,
}

impl ImeState {
    /// One thing the input method said, applied. Answers the bytes a commit
    /// produced, which are the caller's to write.
    ///
    /// **What a commit is.** `Ime::Commit` is the composed text, and it goes
    /// to the PTY as UTF-8, unchanged and unencoded: a commit is text the user
    /// finished choosing, not a keystroke, so it is carried as bytes and
    /// nothing else -- it is neither run through the keytab (there is no key
    /// to bind) nor escaped. In particular it is not bracketed: a program
    /// that reads a paste bracket where the user typed a word is worse off
    /// than one that reads the word.
    ///
    /// **What the pre-edit does.** It is held here and drawn at the cursor, in
    /// the grid, by `term::render::GridRenderer::set_preedit`, so the
    /// half-typed word appears where it is being typed, through the curvature
    /// and in the phosphor, and vanishes on the commit or the abandon. The
    /// frame reads this field on every redraw rather than being pushed at from
    /// here: the composition is state, not an event, and the cursor it stands
    /// at can move underneath it.
    ///
    /// Winit's inner cursor offsets ([`ImeState::cursor`]) are kept and not
    /// drawn: the whole composition is painted as one block, so the offset
    /// inside it is not consulted.
    ///
    /// The other half is [`ImeState::publish`], which tells the platform where
    /// the caret is so the candidate window follows it.
    pub fn apply(&mut self, event: &Ime) -> Option<Vec<u8>> {
        match event {
            Ime::Enabled => {
                *self = ImeState {
                    enabled: true,
                    ..ImeState::default()
                };
                None
            }
            Ime::Preedit(text, cursor) => {
                self.preedit.clear();
                self.preedit.push_str(text);
                self.cursor = *cursor;
                None
            }
            Ime::Commit(text) => {
                // The composition is over whether or not a `Preedit("")`
                // follows, and every input method sends the two in a different
                // order. Clearing here means the state is never a stale word
                // the user already committed.
                self.preedit.clear();
                self.cursor = None;
                (!text.is_empty()).then(|| text.as_bytes().to_vec())
            }
            Ime::Disabled => {
                *self = ImeState::default();
                None
            }
        }
    }

    /// Drop the half-typed composition, the input method left enabled.
    ///
    /// What the user was composing was being composed into one channel's
    /// program. The commit can only go to whatever is on the air when it
    /// arrives, so a composition outlives the channel it belongs to or it
    /// does not outlive the switch: this is the second.
    pub fn abandon(&mut self) {
        self.preedit.clear();
        self.cursor = None;
    }

    /// Tell the platform where the caret is, if it has moved since last time.
    ///
    /// Rounded to whole pixels before the comparison, since that is the
    /// resolution the question is asked at, and a caret that has not moved
    /// must not cost a round trip to the input method 120 times a second.
    pub fn publish(&mut self, window: &Window, rect: (PhysicalPosition<f64>, PhysicalSize<f64>)) {
        let (position, size) = rect;
        let area = (
            position.x.round() as i32,
            position.y.round() as i32,
            size.width.round() as i32,
            size.height.round() as i32,
        );
        if self.area == Some(area) {
            return;
        }
        self.area = Some(area);
        window.set_ime_cursor_area(position, size);
    }
}
