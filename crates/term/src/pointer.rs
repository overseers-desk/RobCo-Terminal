//! What a pointer event means: mark the screen, or forward it to the program?
//!
//! It is here rather than in the window layer because it is terminal
//! semantics: it depends on whether the program below has asked for mouse
//! reporting, and because keeping it as plain functions makes it testable
//! without a pointer.
//!
//! The rule underneath everything: a program that turned mouse reporting on
//! owns the pointer, and Shift is the user's override. One twist on top of
//! that: on frozen glass (a detached, scrolled-back view) Shift is held down
//! for the user, so dragging always marks a selection and nothing the mouse
//! does is written to the wire.

/// The modifier keys.
///
/// One type for the whole application, not just for this module: there is
/// one keyboard under the user's hands, and the keyboard encoder, the
/// mouse reporter and the routing table below all read the same state.
/// `crates/app` reaches it as `app::input::Modifiers`. The rules here
/// consult only Shift, Control and Alt; `meta` is carried because the
/// keytab's CSI modifier parameter counts it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub meta: bool,
}

impl Modifiers {
    pub const NONE: Modifiers = Modifiers {
        shift: false,
        control: false,
        alt: false,
        meta: false,
    };

    pub fn with_shift(mut self) -> Self {
        self.shift = true;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Middle,
    Right,
}

/// The state the pointer decisions read.
#[derive(Clone, Copy, Debug)]
pub struct PointerContext {
    /// The program below turned on mouse reporting (`terminalUsesMouse`).
    pub terminal_uses_mouse: bool,
    /// The channel is an attachment's anchor, so the glass below is the
    /// picture its shell left when it entered control mode and the pty under
    /// it is the protocol's wire.
    pub frozen_glass: bool,
}

impl PointerContext {
    /// Whether dragging marks the screen rather than driving the program.
    /// True whenever the program is not listening, and forced true on
    /// frozen glass.
    pub fn marks_selection(&self) -> bool {
        self.frozen_glass || !self.terminal_uses_mouse
    }

    /// On frozen glass, Shift is held down for the user.
    pub fn marking(&self, modifiers: Modifiers) -> Modifiers {
        if self.frozen_glass {
            modifiers.with_shift()
        } else {
            modifiers
        }
    }

    /// Whether the pointer should show as an I-beam: true exactly when
    /// marking is active.
    pub fn shows_ibeam_cursor(&self) -> bool {
        self.marks_selection()
    }

    /// The emulation marks unless the program asked for the mouse.
    fn mouse_marks(&self) -> bool {
        !self.terminal_uses_mouse
    }
}

/// What should happen to a pointer event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PointerAction {
    /// Begin or extend a selection at this cell.
    Mark,
    /// Report the event to the program below.
    ReportToProgram,
    /// Paste the primary selection (middle click while marking).
    PastePrimary { bracketed: bool },
    /// A link was pressed: anchor a selection as usual *and* activate the
    /// link. `mousePressEvent` does both, in that order: the anchor is set
    /// first and the hotspot check follows it, so a press on a link that turns
    /// into a drag selects the link text instead of following it.
    MarkAndActivateHotSpot,
    /// Swallow it. Middle click on frozen glass: a paste is inert on an
    /// anchor, so the event never reaches the core.
    Ignore,
}

/// The press routing table.
///
/// `over_hot_spot` reports whether a link lies under the cell; a left press
/// that marks and lands on one activates it, which is why a plain click opens
/// a URL without a modifier.
pub fn on_press(
    ctx: PointerContext,
    button: Button,
    modifiers: Modifiers,
    over_hot_spot: bool,
) -> PointerAction {
    // Two cases are intercepted before the core sees anything.
    if ctx.frozen_glass && button == Button::Middle {
        return PointerAction::Ignore;
    }
    let marking = ctx.marking(modifiers);
    // A right press while marking (or with Shift held) is deliberately inert:
    // it neither marks nor reaches the program.
    if (ctx.marks_selection() || modifiers.shift) && button == Button::Right {
        return PointerAction::Ignore;
    }

    let marks = ctx.mouse_marks() || marking.shift;
    match button {
        Button::Left => {
            if marks {
                if over_hot_spot {
                    PointerAction::MarkAndActivateHotSpot
                } else {
                    PointerAction::Mark
                }
            } else {
                PointerAction::ReportToProgram
            }
        }
        Button::Middle => {
            if marks {
                PointerAction::PastePrimary {
                    bracketed: marking.control,
                }
            } else {
                PointerAction::ReportToProgram
            }
        }
        Button::Right => {
            if marks {
                PointerAction::Ignore
            } else {
                PointerAction::ReportToProgram
            }
        }
    }
}

/// `_preserveLineBreaks`, set from the press modifiers: Ctrl without Alt
/// copies a wrapped selection as one unbroken run.
pub fn preserve_line_breaks(modifiers: Modifiers) -> bool {
    !(modifiers.control && !modifiers.alt)
}

/// `_columnSelectionMode`: Ctrl+Alt makes the drag rectangular.
pub fn column_selection_mode(modifiers: Modifiers) -> bool {
    modifiers.control && modifiers.alt
}
