//! The keyboard: what a key press means before the emulation ever sees it.
//!
//! One order, and it is not a matter of taste. The window's own shortcuts run
//! first, because they are the window's; then whatever is standing on the
//! glass and holding the keyboard (the picker, a question, the find line, a
//! gateway); then the keytab, which is the authority on every key it binds;
//! and only then the plain text the platform decoded.
//!
//! Fields touched: `modes`, the keyboard modes the keytab encodes against;
//! `scroll`, which the keytab's four scrolling actions move and which a
//! typed byte snaps to the bottom; and `channels`, whose session takes the
//! bytes.

use winit::keyboard::ModifiersState;

use crate::clipboard;
use crate::input::{encode_winit_key, KeyAction, KeyboardModes, Modifiers};

use super::TerminalSurface;

impl TerminalSurface {
    /// One key press, reduced to the three things anything downstream reads:
    /// the logical key, the text winit decoded for it, and the modifiers.
    ///
    /// `text` is the text the platform produced with **every** modifier
    /// applied, Control included: winit's `text_with_all_modifiers`.
    /// [`key_text`] is where that is picked, and why; a
    /// caller passing winit's plain `text` field instead would hand `"c"` to
    /// the child where the user pressed `Ctrl+C`.
    ///
    /// [`Surface::key_pressed`] is this with a `winit::event::KeyEvent` around
    /// it. The split is not decoration: `KeyEvent` carries a crate-private
    /// platform field, so no test outside winit can build one, and the wiring
    /// below -- which key reaches the pty and which one moves the viewport --
    /// would otherwise be reachable only from a running window.
    pub fn key_input(
        &mut self,
        logical: &winit::keyboard::Key,
        text: Option<&str>,
        modifiers: ModifiersState,
    ) {
        // The shortcut layer first: these are window-level keys that run
        // before the emulation ever sees the event, so the keytab is not the
        // authority on them.
        if self.shortcut_key(logical, modifiers) {
            return;
        }
        // The destination picker's keyboard, when its page is on the air:
        // a digit connects, Esc steps back, everything else is swallowed
        // so the page stays put. The `gateway_key` shape.
        if self.picker_key(logical) {
            return;
        }
        // Then a question the connection on the air is waiting on. Same
        // shape, same reason: while it stands it holds the keyboard, so
        // nothing typed at a password reaches a wire or a keytab.
        if self.prompt_key(logical, text) {
            return;
        }
        // Then the find line, on the same terms: while it stands it holds
        // every key that is not a window shortcut, because a find line is
        // typed into and Enter means "the next one" rather than a newline
        // for the child.
        if self.find_key(logical, text, modifiers) {
            return;
        }
        // Then the gateway's own keyboard, which stands between the shortcuts
        // and the keytab: it holds the focus in the terminal's place, so the
        // emulation never sees a key at all, while the window's own
        // shortcuts (above) run before any focused item either way.
        if self.gateway_key(logical) {
            return;
        }
        let mods = modifiers_from(modifiers);
        // The keytab next: it is the authority on every key it binds.
        if let Some(action) = encode_winit_key(logical, mods, self.keyboard_modes()) {
            match action {
                KeyAction::Bytes(bytes) => self.type_bytes(&bytes),
                other => self.scroll_key(other),
            }
            return;
        }
        // Everything the keytab does not bind is ordinary text, which is
        // the path winit's own `text` field already decoded for us.
        if let Some(text) = text {
            if !text.is_empty() {
                let bytes = text.as_bytes().to_vec();
                self.type_bytes(&bytes);
            }
        }
    }

    /// The modes the keytab is read under, off the channel on the air.
    ///
    /// Asked at the key rather than held, the way the pointer asks for its
    /// own modes (`mode_contains`): a mode is one screen's, and a window
    /// that kept a copy would answer for the channel that set it long after
    /// another one came to the glass. `ansi` is true because rio-vt has no
    /// VT52 mode to leave.
    fn keyboard_modes(&self) -> KeyboardModes {
        use term::rio_vt::crosswords::Mode;
        KeyboardModes {
            ansi: true,
            application_cursor_keys: self.mode_contains(Mode::APP_CURSOR),
            new_line_mode: self.mode_contains(Mode::LINE_FEED_NEW_LINE),
            app_screen: self.mode_contains(Mode::ALT_SCREEN),
        }
    }

    /// Bytes the user produced on purpose -- a key, a paste, a composition
    /// committed -- as opposed to a report the pointer path sends on a
    /// program's behalf. The view snaps to the live screen first, in one
    /// step rather than a glide: what was typed lands at the bottom, and
    /// the user has to see it land. A modifier alone, a shortcut the window
    /// keeps, a key the gateway keyboard holds never reach here, so they
    /// move nothing; neither does a gateway, where nothing is
    /// written at all (see [`Self::write`]).
    pub(super) fn type_bytes(&mut self, bytes: &[u8]) {
        if self.is_gateway_on_air() {
            return;
        }
        self.wheel_pixels = 0.0;
        if let Some(session) = self.channels.session_mut() {
            self.scroll.to_bottom(session.term_mut());
        }
        self.write(bytes);
    }

    pub(super) fn is_gateway_on_air(&self) -> bool {
        self.channels
            .current()
            .is_some_and(|row| self.channels.is_gateway(row))
    }

    /// The keytab's non-byte actions: `scrollLineUp`, `scrollLineDown`,
    /// `scrollPageUp`, `scrollPageDown` and `scrollLock`, which the keytab
    /// binds to Shift+Up/Down/PageUp/PageDown and the Scroll Lock key
    /// (`app::input`'s table, from `default.keytab`).
    ///
    /// The four that move go to the same [`ScrollPosition`] the wheel moves,
    /// with the same sign: positive is up and into history. Routing them
    /// anywhere else would give the keyboard a second idea of where the view
    /// is, and rio-vt's display offset is the only authority there is.
    fn scroll_key(&mut self, action: KeyAction) {
        let Some(session) = self.channels.session_mut() else {
            return;
        };
        let term = session.term_mut();
        match action {
            KeyAction::ScrollLineUp => self.scroll.scroll(term, 1),
            KeyAction::ScrollLineDown => self.scroll.scroll(term, -1),
            KeyAction::ScrollPageUp => self.scroll.page_up(term),
            KeyAction::ScrollPageDown => self.scroll.page_down(term),
            // Not a movement, and deliberately not a hold either. Konsole
            // holds output on Scroll Lock; the choice here was between
            // porting that hold and leaving the binding inert, and it
            // settled on inert: the keytab names the action, and the action
            // does nothing. The keytab row stays, because the keytab is
            // transcribed as written and the key really is bound to an
            // action that really does nothing.
            //
            // The XON/XOFF hold is a different mechanism and already exists:
            // Ctrl+S/Ctrl+Q reach the pty's `IXON` through the tty
            // discipline, not through this table.
            KeyAction::ScrollLock => {
                log::debug!("scroll lock is bound to nothing");
            }
            // `key_input` has already taken this arm.
            KeyAction::Bytes(_) => {}
        }
    }

    /// The window-level shortcuts, which run before the keytab. Answers
    /// whether the key was one of them.
    ///
    /// | key | handler |
    /// |---|---|
    /// | `Ctrl+Shift+C` | [`Self::copy_selection`] |
    /// | `Ctrl+Shift+F` | [`Self::open_find`] |
    /// | `Ctrl+Shift+V` | [`Self::paste_from`] |
    /// | `Ctrl+Shift+T` | [`Self::new_channel`] |
    /// | `Ctrl+Shift+W` | [`Self::close_channel`] |
    /// | `Ctrl+Shift+Left/Right` | [`Self::move_channel`] |
    /// | `Ctrl+PgUp/PgDown` | [`Self::cycle_channel`] |
    /// | `Alt+PgUp/PgDown` | [`Self::step_bank`] |
    /// | `Alt+<digit>` | [`Self::chord_digit`] (select) |
    /// | `Alt+Shift+<digit>` | [`Self::chord_digit`] (store) |
    /// | `Alt+Shift+T` | [`Self::open_picker`] |
    ///
    /// The clipboard pair, the find line and the tab-moving arrows are the
    /// chords Konsole and GNOME Terminal both put here, so a hand arriving
    /// from either finds them where it left them.
    ///
    /// `Ctrl+Shift+N`/`Q` and `F11` are the *shell*'s (a window, not a
    /// channel) and never reach here; [`crate::shell`] takes them first.
    fn shortcut_key(&mut self, logical: &winit::keyboard::Key, modifiers: ModifiersState) -> bool {
        use winit::keyboard::{Key, NamedKey};

        let ctrl = modifiers.control_key();
        let shift = modifiers.shift_key();
        let chord_mod = crate::chord::modifier_down(modifiers);

        match logical {
            Key::Named(NamedKey::PageUp) if chord_mod => self.step_bank(-1),
            Key::Named(NamedKey::PageDown) if chord_mod => self.step_bank(1),
            Key::Named(NamedKey::PageUp) if ctrl && !shift => {
                self.cycle_channel(-1);
                true
            }
            Key::Named(NamedKey::PageDown) if ctrl && !shift => {
                self.cycle_channel(1);
                true
            }
            Key::Character(c) if chord_mod && is_digit(c) => {
                self.chord_digit(c.as_bytes()[0], shift)
            }
            // The destination picker: the owner's chord, chosen over the
            // desktop-grabbed Ctrl+Alt+T (#14).
            Key::Character(c) if chord_mod && shift && c.eq_ignore_ascii_case("t") => {
                self.open_picker();
                true
            }
            Key::Named(NamedKey::ArrowLeft) if ctrl && shift => {
                self.move_channel(-1);
                true
            }
            Key::Named(NamedKey::ArrowRight) if ctrl && shift => {
                self.move_channel(1);
                true
            }
            Key::Character(c) if ctrl && shift && c.eq_ignore_ascii_case("t") => {
                self.new_channel();
                true
            }
            Key::Character(c) if ctrl && shift && c.eq_ignore_ascii_case("w") => {
                self.close_channel();
                true
            }
            Key::Character(c) if ctrl && shift && c.eq_ignore_ascii_case("c") => {
                self.copy_selection();
                true
            }
            Key::Character(c) if ctrl && shift && c.eq_ignore_ascii_case("f") => {
                self.open_find();
                true
            }
            Key::Character(c) if ctrl && shift && c.eq_ignore_ascii_case("v") => {
                self.paste_from(clipboard::Target::Clipboard, false);
                true
            }
            _ => false,
        }
    }
}

/// The text a key press produced, with **every** modifier applied.
///
/// `KeyEvent::text` is not that: on the X11/Wayland backend it is
/// `keysym_to_utf8_raw(keysym)`, the text of the keysym the layout resolved,
/// which for `Ctrl+C` is `"c"`: Control is not in it (winit
/// `platform_impl/linux/common/xkb/mod.rs:315-327`, and the trait below says so
/// outright: "Identical to `KeyEvent::text` but this is affected by Ctrl").
/// `text_with_all_modifiers` is `get_utf8_raw(keycode)` against the live xkb
/// state, so `Ctrl+C` is `"\x03"`.
///
/// The second is what reaches Konsole. On X11 that text comes from
/// `XLookupString`, which folds Control into the ASCII control range, and
/// Konsole's keytab binds no `Ctrl+<letter>` at all: `Ctrl+C` reaches the
/// emulation as the *text* of the event and nowhere else. Taking winit's plain
/// `text` here sent a literal `c` to the child for every `Ctrl+<letter>` the
/// keytab does not bind (no interrupt, no `Ctrl+D`, no readline bindings)
/// on local channels and, once the diversion existed, through `send-keys` to
/// tmux panes as well.
///
/// winit implements the trait on Windows, macOS, X11 and Wayland
/// (`platform/mod.rs:44-52`), which is every platform this appliance builds
/// for; a target without it would fail to compile here rather than quietly lose
/// its control keys.
pub(super) fn key_text(event: &winit::event::KeyEvent) -> Option<&str> {
    use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
    event.text_with_all_modifiers()
}

/// A single ASCII digit, which is what the chord's ten shortcuts each carry.
fn is_digit(c: &str) -> bool {
    c.len() == 1 && c.as_bytes()[0].is_ascii_digit()
}

/// winit's modifier state as the routing tables and the mouse encoder read
/// it.
pub(super) fn modifiers_from(modifiers: ModifiersState) -> Modifiers {
    Modifiers {
        shift: modifiers.shift_key(),
        control: modifiers.control_key(),
        alt: modifiers.alt_key(),
        meta: modifiers.super_key(),
    }
}
