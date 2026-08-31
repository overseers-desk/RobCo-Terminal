//! The seam between the bank column and the glass, and the bank's own keys.
//!
//! Three claims on a pointer event that the grid never sees. The grab strip
//! over the bank's right edge is the narrowest one and gets first refusal;
//! a press inside one of the drawn windows below it is the second, and one
//! on a key of the pager at the bank's foot is the third.
//!
//! Fields touched: `cabinet`, which owns the geometry and the hit tests and
//! takes a drag on the spot; `seam_press`, the press the seam is holding;
//! `pending_led_characters`, what a drag has landed and the settings file
//! does not carry yet; `seam_cursor`, the shape the window is wearing; and
//! `shell_events`, because the bank's width is the window's minimum-width
//! hint and only the shell can apply one.

use chassis::{SeamCursor, SeamUpdate};
use config::toml::Scalar;
use winit::dpi::PhysicalPosition;
use winit::event::MouseButton;
use winit::window::CursorIcon;

use crate::shell::ShellEvent;

use super::{spawn, TerminalSurface};

impl TerminalSurface {
    /// A window x in the logical pixels every chassis measure is in
    /// (`chassis::cabinet`'s "which pixel").
    fn logical_x(&self, position: PhysicalPosition<f64>) -> f64 {
        position.x / self.viewport.scale_factor.max(f64::EPSILON)
    }

    /// A left press on the grab strip is the seam's and nobody else's.
    /// Returns whether it took it.
    pub(super) fn seam_pressed(&mut self, button: MouseButton, position: PhysicalPosition<f64>) -> bool {
        if button != MouseButton::Left {
            return false;
        }
        let x = self.logical_x(position);
        self.seam_press = self
            .cabinet
            .as_mut()
            .is_some_and(|cabinet| cabinet.pointer_pressed(x));
        self.seam_press
    }

    /// The window itself is the key: pressing it reaches the channel, so the
    /// row carries no separate button. A left press inside one of the bank's
    /// drawn windows is that key going down.
    ///
    /// Returns whether it took the press. The hit test is the furniture's
    /// ([`chassis::Cabinet::strip_at`], over the same rectangles it drew) and
    /// what to do about it is the bank's ([`crate::bank::BankPager::press`],
    /// through [`TerminalSurface::press_strip`]): a dark slot starts a session
    /// and an open one comes to the screen.
    pub(super) fn strip_pressed(&mut self, button: MouseButton, position: PhysicalPosition<f64>) -> bool {
        if button != MouseButton::Left {
            return false;
        }
        let scale = self.viewport.scale_factor.max(f64::EPSILON);
        let (x, y) = (position.x / scale, position.y / scale);
        let strips = self.bank_strips();
        let Some(row) = self
            .cabinet
            .as_ref()
            .and_then(|cabinet| cabinet.strip_at(strips.rows.len(), x, y))
        else {
            return false;
        };
        self.press_strip(strips.rows[row].channel);
        true
    }

    /// The pager at the bank's foot: a left press on PREV or NEXT walks the
    /// pages, which is the press the drawn rocker has always looked like.
    ///
    /// Returns whether it took the press. The hit test is the furniture's
    /// ([`chassis::Cabinet::pager_at`], over the same key rectangles it drew)
    /// and the step is [`TerminalSurface::step_bank`]'s, the one the
    /// `Alt`+`PgUp`/`PgDown` keys make: within a bank it views another
    /// screenful, and onto another bank's stretch it brings that bank's
    /// remembered channel back to the glass. A press past the last page is
    /// taken and does nothing, the step's own clamp answering for the dimmed
    /// key.
    pub(super) fn pager_pressed(
        &mut self,
        button: MouseButton,
        position: PhysicalPosition<f64>,
    ) -> bool {
        if button != MouseButton::Left {
            return false;
        }
        let scale = self.viewport.scale_factor.max(f64::EPSILON);
        let (x, y) = (position.x / scale, position.y / scale);
        let Some(direction) = self.cabinet.as_ref().and_then(|c| c.pager_at(x, y)) else {
            return false;
        };
        self.step_bank(direction);
        true
    }

    /// A motion the seam claims: it is either dragging the boundary or hovering
    /// the strip, and either way the grid does not also see it.
    pub(super) fn seam_moved(&mut self, position: PhysicalPosition<f64>) -> bool {
        let x = self.logical_x(position);
        let Some(cabinet) = self.cabinet.as_mut() else {
            return false;
        };
        let update = cabinet.cursor_moved(x);
        let claimed = cabinet.cursor() == SeamCursor::ResizeHorizontal;
        self.set_seam_cursor(claimed);
        if let Some(update) = update {
            self.seam_landed(update);
        }
        claimed
    }

    /// The drag landed a new character count. The cabinet has already taken it;
    /// what is left is the two obligations that are not the chassis's to
    /// discharge (`chassis::cabinet`'s "Two obligations").
    fn seam_landed(&mut self, update: SeamUpdate) {
        // The write waits for the button to come up; the count is remembered
        // here so a drag that ends outside the strip still writes what it left.
        self.pending_led_characters = Some(update.led_characters);
        self.relayout();
        if let Some(proxy) = self.shell_events.as_ref() {
            // The drag re-based the configured count, so the bank's least
            // moved with it: dragging below the display's own floor takes the
            // hint down too, since the fit does not widen what a hand set.
            let minimum = self
                .cabinet
                .as_ref()
                .map_or(update.bank_width, |c| c.min_bank_width());
            let _ = proxy.send_event(ShellEvent::SetBankWidth {
                width: update.bank_width,
                minimum,
            });
        }
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    /// The button came up, or the window lost focus with it still down.
    pub(super) fn seam_released(&mut self) {
        self.seam_press = false;
        if let Some(cabinet) = self.cabinet.as_mut() {
            cabinet.pointer_released();
        }
        let Some(characters) = self.pending_led_characters.take() else {
            return;
        };
        let Some(settings) = self.settings.as_ref() else {
            // No handle: `--default-settings`, or a headless surface that never
            // attached one. The drag stands on screen and is not persisted,
            // which is what "never touch the user's real config" means.
            log::debug!("the seam landed on {characters} characters with no config to write");
            return;
        };
        // The file is the source of truth, so this is the write that makes the
        // drag real; the watcher then delivers it back as an ordinary reload,
        // carrying the count the cabinet already holds.
        if let Err(e) = settings.write_key(
            "general.led_characters",
            Scalar::Integer(i64::from(characters)),
        ) {
            log::error!("could not write general.led_characters = {characters}: {e}");
        }
    }

    /// The shape is the only thing that says the seam is there, so it
    /// changes on hover and not only while dragging.
    fn set_seam_cursor(&mut self, claimed: bool) {
        if claimed == self.seam_cursor {
            return;
        }
        self.seam_cursor = claimed;
        if let Some(window) = self.window.as_ref() {
            window.set_cursor(if claimed {
                CursorIcon::EwResize
            } else {
                CursorIcon::Default
            });
        }
    }

    /// A press on one of the bank's strips. The `channel` is
    /// [`chassis::StripRow::channel`], the absolute slot behind
    /// the engraved numeral, so the caller never has to know where the pager
    /// stands. A dark slot starts a session on it and an open one comes to the
    /// screen; either way the shell gets the keyboard back, which here it never
    /// gave up.
    ///
    /// The hit test is the furniture's: the window *is* the key, and
    /// only whoever drew it knows where it is.
    pub fn press_strip(&mut self, channel: u32) {
        let (config, size) = (self.session_now(), self.viewport.term_size());
        let pager = self.pager.clone();
        pager.press(&mut self.channels, channel, || spawn(&config, size));
        self.channel_changed();
    }
}
