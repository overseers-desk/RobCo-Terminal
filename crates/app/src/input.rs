//! Keyboard encoding: a lookup table of `key <name> <conditions> :
//! <bytes-or-action>` rules, in the vocabulary of Konsole's keytab format.
//! This implements the "risc" default keymap; a fuller table exists in that
//! format but is not implemented here (see module notes below).
//!
//! Two layers:
//!
//! - [`PhysicalKey`] + [`encode_key`]: a pure function, no winit dependency,
//!   matching each `key <name> <conditions> : <bytes-or-action>` line in the
//!   keytab against ([`PhysicalKey`], [`Modifiers`], [`KeyboardModes`]).
//! - [`encode_winit_key`]: the thin winit adapter that keeps the contract
//!   pure function winit-key → bytes, mapping `winit::keyboard::Key` to
//!   [`PhysicalKey`] and delegating to [`encode_key`]. It carries no window
//!   state, so it is exercised directly in tests without an event loop.
//!
//! # Meta is not a keytab rule
//!
//! Holding Alt puts an ESC in front of the bytes, which is what readline,
//! zsh and emacs read as a Meta key and what xterm calls `metaSendsEscape`.
//! No line of the keytab format says so: Konsole and xterm both settle Alt
//! outside the table. It lands here anyway, because the keytab's own bytes
//! are what the ESC has to sit in front of, and a caller holding the table
//! at arm's length would have to know which rules spent the modifier on a
//! CSI parameter already. [`KeyboardModes::alt_is_meta`] carries the
//! answer in, so this stays a pure function that can be read both ways;
//! [`alt_is_meta`] is where the platform picks one.

use winit::keyboard::{Key, NamedKey};

/// The physical/logical key identity the keytab conditions dispatch on,
/// named as the keytab format itself spells them (`Up`, `Prior`, ...).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalKey {
    Escape,
    Tab,
    /// The main Return key (keytab's `Return`).
    Return,
    /// Keypad Enter (keytab's `Enter`): the numpad enter key, distinct from
    /// `Return`.
    Enter,
    Backspace,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    Insert,
    Delete,
    /// `Prior` in the keytab (Page Up).
    PageUp,
    /// `Next` in the keytab (Page Down).
    PageDown,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    Space,
    ScrollLock,
}

/// Modifier state at the time of the key event.
///
/// Defined one crate down, in [`term::pointer`], because there is one
/// keyboard under the user's hands and every consumer of its state reads
/// the same four keys: this encoder, the mouse reporter, and the pointer
/// routing table that decides whether a drag marks the screen. `term` is
/// the lower crate, so it is the one both halves can reach.
pub use term::pointer::Modifiers;

/// The keytab's own modifier conditions.
///
/// An extension trait rather than methods on [`Modifiers`] itself: `AnyMod`
/// is a word from `default.keytab`, not terminal vocabulary, so it belongs
/// beside the table that reads it rather than on the shared type.
trait KeytabConditions {
    fn any_mod(self) -> bool;
    fn any_mod_incl_shift(self) -> bool;
    fn param(self) -> u8;
}

impl KeytabConditions for Modifiers {
    /// The keytab's `AnyMod` condition: any modifier *other than Shift*
    /// held. Shift alone does not set `AnyMod`: the keytab's Shift-only
    /// arrow-key behavior is scroll, gated separately (see
    /// `KeyboardModes::app_screen`).
    fn any_mod(self) -> bool {
        self.control || self.alt || self.meta
    }

    /// The keytab's plain `AnyMod` condition on keys that carry no
    /// `-Shift`/`+Shift` qualifier of their own (Insert, Delete, Home, End,
    /// F1-F12): Shift counts here, unlike [`KeytabConditions::any_mod`]
    /// which is only for the arrow keys' shift-reserved-for-scroll special
    /// case.
    fn any_mod_incl_shift(self) -> bool {
        self.shift || self.any_mod()
    }

    /// xterm/CSI modifier parameter: `1 + shift*1 + alt*2 + ctrl*4 + meta*8`.
    /// This is the value the keytab's `*` placeholder stands for in escapes
    /// like `\E[1;*A`.
    fn param(self) -> u8 {
        1 + (self.shift as u8)
            + (self.alt as u8) * 2
            + (self.control as u8) * 4
            + (self.meta as u8) * 8
    }
}

/// Terminal-mode flags the keytab's `+Ansi`, `+AppCuKeys`, `+NewLine`,
/// `+AppScreen` conditions read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyboardModes {
    /// ANSI mode vs VT52 mode (`+Ansi` / `-Ansi`).
    pub ansi: bool,
    /// Application Cursor Keys mode (DECCKM, `+AppCuKeys` / `-AppCuKeys`).
    pub application_cursor_keys: bool,
    /// Newline mode (LNM, `+NewLine` / `-NewLine`): Return sends `\r\n`
    /// instead of `\r`.
    pub new_line_mode: bool,
    /// Alternate screen active (`+AppScreen` / `-AppScreen`): Shift+Up/Down
    /// /PageUp/PageDown only scroll history when this is false.
    pub app_screen: bool,
    /// Whether Alt puts an ESC in front of the bytes (see the module notes).
    /// Not a terminal mode and not a keytab condition: it is the platform's
    /// answer, carried in so the table itself stays platform-free and both
    /// readings are testable. [`alt_is_meta`] is what a caller fills it from.
    pub alt_is_meta: bool,
}

impl Default for KeyboardModes {
    /// Matches a freshly reset terminal: ANSI mode, normal (not
    /// application) cursor keys, no newline mode, primary screen, and Alt
    /// meaning Meta wherever this build runs.
    fn default() -> Self {
        KeyboardModes {
            ansi: true,
            application_cursor_keys: false,
            new_line_mode: false,
            app_screen: false,
            alt_is_meta: alt_is_meta(),
        }
    }
}

/// Whether the Alt key means Meta on this platform.
///
/// It does everywhere but macOS, where Option composes the character the
/// session is meant to receive: winit hands over `NSEvent.characters()`, so
/// Option+8 is the `{` a German layout puts there, and an ESC in front of it
/// would cost a Mac the brace rather than buy it a Meta key. Apple's
/// Terminal and iTerm2 both ship that way, and the gesture that needs no
/// setting at all -- Escape, released, then the key -- reaches readline the
/// same on every platform.
///
/// [`crate::chord::modifier_down`] turns on the same fact from the other
/// side: the chord takes Cmd on macOS for the same reason Meta declines
/// Option there.
pub fn alt_is_meta() -> bool {
    !cfg!(target_os = "macos")
}

/// What a keytab entry produces: bytes to write to the pty, or one of the
/// two non-byte actions the keytab names (`scrollLineUp` etc., `scrollLock`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    Bytes(Vec<u8>),
    ScrollLineUp,
    ScrollPageUp,
    ScrollLineDown,
    ScrollPageDown,
    ScrollLock,
}

impl KeyAction {
    fn bytes(s: &str) -> KeyAction {
        KeyAction::Bytes(s.as_bytes().to_vec())
    }
}

/// Alt as Meta, for the keytab rules that encode no modifier of their own.
///
/// The rules that reach [`csi_letter`], [`csi_tilde`] and [`function_key`]
/// have already spent Alt on a CSI parameter, so they are not wrapped: an
/// ESC in front of `\E[1;3D` would be one modifier reported twice, and the
/// program on the other end would read the second as a keystroke.
fn meta(action: KeyAction, modifiers: Modifiers, modes: KeyboardModes) -> KeyAction {
    match action {
        KeyAction::Bytes(bytes) if modifiers.alt && modes.alt_is_meta => {
            let mut out = vec![0x1b];
            out.extend_from_slice(&bytes);
            KeyAction::Bytes(out)
        }
        other => other,
    }
}

/// `any_mod` selects which of [`KeytabConditions::any_mod`] (arrow keys,
/// Shift excluded, reserved for scroll) or
/// [`KeytabConditions::any_mod_incl_shift`]
/// (every other key here) the caller's keytab rule uses.
fn csi_letter(letter: char, modifiers: Modifiers, app_mode: bool, any_mod: bool) -> KeyAction {
    if !any_mod {
        if app_mode {
            KeyAction::bytes(&format!("\x1bO{letter}"))
        } else {
            KeyAction::bytes(&format!("\x1b[{letter}"))
        }
    } else {
        KeyAction::bytes(&format!("\x1b[1;{}{letter}", modifiers.param()))
    }
}

fn csi_tilde(num: u8, modifiers: Modifiers) -> KeyAction {
    if modifiers.any_mod_incl_shift() {
        KeyAction::bytes(&format!("\x1b[{num};{}~", modifiers.param()))
    } else {
        KeyAction::bytes(&format!("\x1b[{num}~"))
    }
}

fn function_key(final_letter_or_num: FnKeyEncoding, modifiers: Modifiers) -> KeyAction {
    match (final_letter_or_num, modifiers.any_mod_incl_shift()) {
        (FnKeyEncoding::SsLetter(c), false) => KeyAction::bytes(&format!("\x1bO{c}")),
        (FnKeyEncoding::SsLetter(c), true) => {
            KeyAction::bytes(&format!("\x1bO{}{c}", modifiers.param()))
        }
        (FnKeyEncoding::Tilde(n), false) => KeyAction::bytes(&format!("\x1b[{n}~")),
        (FnKeyEncoding::Tilde(n), true) => {
            KeyAction::bytes(&format!("\x1b[{n};{}~", modifiers.param()))
        }
    }
}

enum FnKeyEncoding {
    SsLetter(char),
    Tilde(u8),
}

/// Pure keytab lookup: matches `default.keytab`'s rules against a
/// (key, modifiers, terminal-mode) triple. Returns `None` for keys the
/// keytab does not bind (e.g. plain printable characters, which are
/// encoded by the app's text-input path, not this table).
pub fn encode_key(
    key: PhysicalKey,
    modifiers: Modifiers,
    modes: KeyboardModes,
) -> Option<KeyAction> {
    use PhysicalKey::*;

    match key {
        // key Escape : "\E"
        Escape => Some(meta(KeyAction::bytes("\x1b"), modifiers, modes)),

        // key Tab -Shift : "\t"
        // key Tab +Shift+Ansi : "\E[Z"
        // key Tab +Shift-Ansi : "\t"
        // key Backtab +Ansi : "\E[Z"
        // key Backtab -Ansi : "\t"
        // (Backtab is winit's Shift+Tab; folded into the Tab arm below.)
        Tab => {
            let bytes = if modifiers.shift && modes.ansi {
                KeyAction::bytes("\x1b[Z")
            } else {
                KeyAction::bytes("\t")
            };
            Some(meta(bytes, modifiers, modes))
        }

        // key Return-Shift-NewLine : "\r"
        // key Return-Shift+NewLine : "\r\n"
        // key Return+Shift : "\EOM"
        Return => {
            let bytes = if modifiers.shift {
                KeyAction::bytes("\x1bOM")
            } else if modes.new_line_mode {
                KeyAction::bytes("\r\n")
            } else {
                KeyAction::bytes("\r")
            };
            Some(meta(bytes, modifiers, modes))
        }

        // Keypad Enter is not a separate keytab entry in this "risc" table --
        // keypad characters are not offered differently here -- so it is
        // treated the same as Return.
        Enter => encode_key(Return, modifiers, modes),

        // key Backspace : "\x7f"  (unconditional)
        Backspace => Some(meta(KeyAction::bytes("\x7f"), modifiers, modes)),

        // Arrow keys: VT52 mode (-Ansi) vs ANSI mode (+Ansi), and within
        // ANSI mode, Application vs Normal Cursor Keys, and AnyMod overrides
        // both when a non-Shift modifier is held.
        Up | Down | Left | Right => {
            if !modes.ansi {
                // key {Up,Down,Right,Left} -Shift-Ansi : "\EA" / "\EB" / "\EC" / "\ED"
                // (shift is reserved for scrolling; handled below.)
                if modifiers.shift {
                    return shift_scroll(key, modes);
                }
                let letter = match key {
                    Up => 'A',
                    Down => 'B',
                    Right => 'C',
                    Left => 'D',
                    _ => unreachable!(),
                };
                return Some(KeyAction::bytes(&format!("\x1b{letter}")));
            }

            if modifiers.shift && !modifiers.any_mod() {
                // Shift alone: reserved for scrolling (Up/Down only per the
                // keytab; Left/Right are hardcoded to tab-switching upstream
                // and unbound here).
                if let Some(action) = shift_scroll(key, modes) {
                    return Some(action);
                }
                // Shift+Left/Right, or Shift+Up/Down while on the alt
                // screen: the keytab has no fallback rule, so nothing is
                // sent (matches the "risc" table's silence here).
                return None;
            }

            let letter = match key {
                Up => 'A',
                Down => 'B',
                Right => 'C',
                Left => 'D',
                _ => unreachable!(),
            };
            // csi_letter only consults app_mode when no non-Shift modifier
            // is held (any_mod forces the "\E[1;*<letter>" form regardless).
            // Arrow keys use the Shift-excluded AnyMod (Shift is handled
            // above, reserved for scrolling).
            Some(csi_letter(
                letter,
                modifiers,
                modes.application_cursor_keys,
                modifiers.any_mod(),
            ))
        }

        // key Home -AnyMod -AppCuKeys : "\E[H"
        // key End  -AnyMod -AppCuKeys : "\E[F"
        // key Home -AnyMod +AppCuKeys : "\EOH"
        // key End  -AnyMod +AppCuKeys : "\EOF"
        // key Home +AnyMod : "\E[1;*H"
        // key End  +AnyMod : "\E[1;*F"
        Home => Some(csi_letter(
            'H',
            modifiers,
            modes.application_cursor_keys,
            modifiers.any_mod_incl_shift(),
        )),
        End => Some(csi_letter(
            'F',
            modifiers,
            modes.application_cursor_keys,
            modifiers.any_mod_incl_shift(),
        )),

        // key Insert -AnyMod : "\E[2~"
        // key Delete -AnyMod : "\E[3~"
        // key Insert +AnyMod : "\E[2;*~"
        // key Delete +AnyMod : "\E[3;*~"
        Insert => Some(csi_tilde(2, modifiers)),
        Delete => Some(csi_tilde(3, modifiers)),

        // key Prior -Shift-AnyMod : "\E[5~"
        // key Next  -Shift-AnyMod : "\E[6~"
        // key Prior -Shift+AnyMod : "\E[5;*~"
        // key Next  -Shift+AnyMod : "\E[6;*~"
        // key Up/Prior +Shift-AppScreen : scrollPageUp / scrollLineUp (handled above for Up/Down)
        PageUp => {
            if modifiers.shift && !modifiers.any_mod() {
                return shift_scroll(key, modes);
            }
            Some(csi_tilde(5, modifiers))
        }
        PageDown => {
            if modifiers.shift && !modifiers.any_mod() {
                return shift_scroll(key, modes);
            }
            Some(csi_tilde(6, modifiers))
        }

        // key F1..F4 -AnyMod : "\EOP".."\EOS"; +AnyMod : "\EO*P".."\EO*S"
        // key F5..F12 -AnyMod : "\E[15~".."\E[24~"; +AnyMod adds ";*"
        F1 => Some(function_key(FnKeyEncoding::SsLetter('P'), modifiers)),
        F2 => Some(function_key(FnKeyEncoding::SsLetter('Q'), modifiers)),
        F3 => Some(function_key(FnKeyEncoding::SsLetter('R'), modifiers)),
        F4 => Some(function_key(FnKeyEncoding::SsLetter('S'), modifiers)),
        F5 => Some(function_key(FnKeyEncoding::Tilde(15), modifiers)),
        F6 => Some(function_key(FnKeyEncoding::Tilde(17), modifiers)),
        F7 => Some(function_key(FnKeyEncoding::Tilde(18), modifiers)),
        F8 => Some(function_key(FnKeyEncoding::Tilde(19), modifiers)),
        F9 => Some(function_key(FnKeyEncoding::Tilde(20), modifiers)),
        F10 => Some(function_key(FnKeyEncoding::Tilde(21), modifiers)),
        F11 => Some(function_key(FnKeyEncoding::Tilde(23), modifiers)),
        F12 => Some(function_key(FnKeyEncoding::Tilde(24), modifiers)),

        // key Space +Control : "\x00"
        Space => {
            if modifiers.control {
                Some(meta(KeyAction::Bytes(vec![0x00]), modifiers, modes))
            } else {
                None
            }
        }

        // key ScrollLock : scrollLock
        ScrollLock => Some(KeyAction::ScrollLock),
    }
}

fn shift_scroll(key: PhysicalKey, modes: KeyboardModes) -> Option<KeyAction> {
    if modes.app_screen {
        return None;
    }
    match key {
        PhysicalKey::Up => Some(KeyAction::ScrollLineUp),
        PhysicalKey::PageUp => Some(KeyAction::ScrollPageUp),
        PhysicalKey::Down => Some(KeyAction::ScrollLineDown),
        PhysicalKey::PageDown => Some(KeyAction::ScrollPageDown),
        _ => None,
    }
}

/// Map a winit logical key to this module's [`PhysicalKey`]. Returns `None`
/// for keys outside the keytab's vocabulary (printable characters, modifier
/// keys themselves, etc.), which are not this table's concern.
pub fn physical_key_from_winit(key: &Key) -> Option<PhysicalKey> {
    match key {
        Key::Named(NamedKey::Escape) => Some(PhysicalKey::Escape),
        Key::Named(NamedKey::Tab) => Some(PhysicalKey::Tab),
        Key::Named(NamedKey::Enter) => Some(PhysicalKey::Return),
        Key::Named(NamedKey::Backspace) => Some(PhysicalKey::Backspace),
        Key::Named(NamedKey::ArrowUp) => Some(PhysicalKey::Up),
        Key::Named(NamedKey::ArrowDown) => Some(PhysicalKey::Down),
        Key::Named(NamedKey::ArrowLeft) => Some(PhysicalKey::Left),
        Key::Named(NamedKey::ArrowRight) => Some(PhysicalKey::Right),
        Key::Named(NamedKey::Home) => Some(PhysicalKey::Home),
        Key::Named(NamedKey::End) => Some(PhysicalKey::End),
        Key::Named(NamedKey::Insert) => Some(PhysicalKey::Insert),
        Key::Named(NamedKey::Delete) => Some(PhysicalKey::Delete),
        Key::Named(NamedKey::PageUp) => Some(PhysicalKey::PageUp),
        Key::Named(NamedKey::PageDown) => Some(PhysicalKey::PageDown),
        Key::Named(NamedKey::F1) => Some(PhysicalKey::F1),
        Key::Named(NamedKey::F2) => Some(PhysicalKey::F2),
        Key::Named(NamedKey::F3) => Some(PhysicalKey::F3),
        Key::Named(NamedKey::F4) => Some(PhysicalKey::F4),
        Key::Named(NamedKey::F5) => Some(PhysicalKey::F5),
        Key::Named(NamedKey::F6) => Some(PhysicalKey::F6),
        Key::Named(NamedKey::F7) => Some(PhysicalKey::F7),
        Key::Named(NamedKey::F8) => Some(PhysicalKey::F8),
        Key::Named(NamedKey::F9) => Some(PhysicalKey::F9),
        Key::Named(NamedKey::F10) => Some(PhysicalKey::F10),
        Key::Named(NamedKey::F11) => Some(PhysicalKey::F11),
        Key::Named(NamedKey::F12) => Some(PhysicalKey::F12),
        Key::Named(NamedKey::Space) => Some(PhysicalKey::Space),
        Key::Named(NamedKey::ScrollLock) => Some(PhysicalKey::ScrollLock),
        Key::Character(s) if s.as_str() == " " => Some(PhysicalKey::Space),
        _ => None,
    }
}

/// The pure `winit-key -> bytes` function: no window or
/// event-loop state, so it is unit-testable without a display.
pub fn encode_winit_key(
    key: &Key,
    modifiers: Modifiers,
    modes: KeyboardModes,
) -> Option<KeyAction> {
    let physical = physical_key_from_winit(key)?;
    encode_key(physical, modifiers, modes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use PhysicalKey::*;

    fn bytes(s: &str) -> KeyAction {
        KeyAction::bytes(s)
    }

    fn modes(ansi: bool, app_cu: bool, new_line: bool, app_screen: bool) -> KeyboardModes {
        KeyboardModes {
            ansi,
            application_cursor_keys: app_cu,
            new_line_mode: new_line,
            app_screen,
            // Stated rather than taken from the platform, so every
            // assertion below reads the same on a Mac as on a Linux box.
            alt_is_meta: true,
        }
    }

    /// The modes a Mac runs in, where Option composes the character instead
    /// of standing for Meta.
    fn composing() -> KeyboardModes {
        KeyboardModes {
            alt_is_meta: false,
            ..modes(true, false, false, false)
        }
    }

    fn alt() -> Modifiers {
        Modifiers {
            alt: true,
            ..Modifiers::NONE
        }
    }

    // key Escape : "\E"
    #[test]
    fn escape() {
        assert_eq!(
            encode_key(Escape, Modifiers::NONE, KeyboardModes::default()),
            Some(bytes("\x1b"))
        );
    }

    // key Tab -Shift : "\t"
    #[test]
    fn tab_no_shift() {
        assert_eq!(
            encode_key(Tab, Modifiers::NONE, KeyboardModes::default()),
            Some(bytes("\t"))
        );
    }

    // key Tab +Shift+Ansi : "\E[Z"
    #[test]
    fn tab_shift_ansi() {
        let m = Modifiers {
            shift: true,
            ..Modifiers::NONE
        };
        assert_eq!(
            encode_key(Tab, m, modes(true, false, false, false)),
            Some(bytes("\x1b[Z"))
        );
    }

    // key Tab +Shift-Ansi : "\t"
    #[test]
    fn tab_shift_no_ansi() {
        let m = Modifiers {
            shift: true,
            ..Modifiers::NONE
        };
        assert_eq!(
            encode_key(Tab, m, modes(false, false, false, false)),
            Some(bytes("\t"))
        );
    }

    // key Return-Shift-NewLine : "\r"
    #[test]
    fn return_no_shift_no_newline() {
        assert_eq!(
            encode_key(Return, Modifiers::NONE, modes(true, false, false, false)),
            Some(bytes("\r"))
        );
    }

    // key Return-Shift+NewLine : "\r\n"
    #[test]
    fn return_no_shift_newline_mode() {
        assert_eq!(
            encode_key(Return, Modifiers::NONE, modes(true, false, true, false)),
            Some(bytes("\r\n"))
        );
    }

    // key Return+Shift : "\EOM"
    #[test]
    fn return_shift() {
        let m = Modifiers {
            shift: true,
            ..Modifiers::NONE
        };
        // Shift+Return overrides newline mode either way.
        assert_eq!(
            encode_key(Return, m, modes(true, false, true, false)),
            Some(bytes("\x1bOM"))
        );
        assert_eq!(
            encode_key(Return, m, modes(true, false, false, false)),
            Some(bytes("\x1bOM"))
        );
    }

    // Keypad Enter treated the same as Return (no separate keytab rule).
    #[test]
    fn keypad_enter_same_as_return() {
        assert_eq!(
            encode_key(Enter, Modifiers::NONE, modes(true, false, false, false)),
            Some(bytes("\r"))
        );
    }

    // key Backspace : "\x7f"
    #[test]
    fn backspace() {
        assert_eq!(
            encode_key(Backspace, Modifiers::NONE, KeyboardModes::default()),
            Some(KeyAction::Bytes(vec![0x7f]))
        );
        // Unconditional: even with Control held.
        let m = Modifiers {
            control: true,
            ..Modifiers::NONE
        };
        assert_eq!(
            encode_key(Backspace, m, KeyboardModes::default()),
            Some(KeyAction::Bytes(vec![0x7f]))
        );
    }

    // Alt as Meta, on the rules that encode no modifier of their own. The
    // first of these is what readline reads as backward-kill-word, and the
    // rest are the same key with the same ESC in front of it.
    #[test]
    fn alt_puts_an_escape_in_front_of_the_keys_that_spend_no_modifier() {
        let m = modes(true, false, false, false);
        assert_eq!(
            encode_key(Backspace, alt(), m),
            Some(bytes("\x1b\x7f")),
            "Alt+Backspace is the word behind the cursor"
        );
        assert_eq!(encode_key(Escape, alt(), m), Some(bytes("\x1b\x1b")));
        assert_eq!(encode_key(Return, alt(), m), Some(bytes("\x1b\r")));
        assert_eq!(encode_key(Enter, alt(), m), Some(bytes("\x1b\r")));

        let shift_alt = Modifiers {
            shift: true,
            ..alt()
        };
        assert_eq!(encode_key(Tab, shift_alt, m), Some(bytes("\x1b\x1b[Z")));

        let ctrl_alt = Modifiers {
            control: true,
            ..alt()
        };
        assert_eq!(
            encode_key(Space, ctrl_alt, m),
            Some(KeyAction::Bytes(vec![0x1b, 0x00]))
        );
    }

    // A rule that already carries Alt in its CSI parameter takes no ESC as
    // well: the program on the other end would read the second report as a
    // keystroke of its own.
    #[test]
    fn a_key_that_reports_alt_as_a_parameter_is_not_prefixed_too() {
        let m = modes(true, false, false, false);
        assert_eq!(encode_key(Up, alt(), m), Some(bytes("\x1b[1;3A")));
        assert_eq!(encode_key(PageUp, alt(), m), Some(bytes("\x1b[5;3~")));
        assert_eq!(encode_key(Home, alt(), m), Some(bytes("\x1b[1;3H")));
    }

    // Where Option composes the character instead, every one of those keys
    // sends what it sends with no modifier at all.
    #[test]
    fn option_composing_leaves_the_bytes_where_they_were() {
        assert_eq!(
            encode_key(Backspace, alt(), composing()),
            Some(bytes("\x7f"))
        );
        assert_eq!(encode_key(Escape, alt(), composing()), Some(bytes("\x1b")));
        assert_eq!(encode_key(Return, alt(), composing()), Some(bytes("\r")));
        assert_eq!(encode_key(Tab, alt(), composing()), Some(bytes("\t")));
        // The parameterised rules are the platform's business either way:
        // Option+Left reports its modifier on a Mac the same as anywhere.
        assert_eq!(
            encode_key(Left, alt(), composing()),
            Some(bytes("\x1b[1;3D"))
        );
    }

    // key Up -Shift-Ansi : "\EA"  (VT52 mode)
    #[test]
    fn up_vt52_mode() {
        assert_eq!(
            encode_key(Up, Modifiers::NONE, modes(false, false, false, false)),
            Some(bytes("\x1bA"))
        );
    }

    // key Down -Shift-Ansi : "\EB"
    #[test]
    fn down_vt52_mode() {
        assert_eq!(
            encode_key(Down, Modifiers::NONE, modes(false, false, false, false)),
            Some(bytes("\x1bB"))
        );
    }

    // key Right -Shift-Ansi : "\EC"
    #[test]
    fn right_vt52_mode() {
        assert_eq!(
            encode_key(Right, Modifiers::NONE, modes(false, false, false, false)),
            Some(bytes("\x1bC"))
        );
    }

    // key Left -Shift-Ansi : "\ED"
    #[test]
    fn left_vt52_mode() {
        assert_eq!(
            encode_key(Left, Modifiers::NONE, modes(false, false, false, false)),
            Some(bytes("\x1bD"))
        );
    }

    // key Up -Shift-AnyMod+Ansi+AppCuKeys : "\EOA"
    #[test]
    fn up_ansi_app_cursor_keys() {
        assert_eq!(
            encode_key(Up, Modifiers::NONE, modes(true, true, false, false)),
            Some(bytes("\x1bOA"))
        );
    }

    // key Down -Shift-AnyMod+Ansi-AppCuKeys : "\E[B"
    #[test]
    fn down_ansi_normal_cursor_keys() {
        assert_eq!(
            encode_key(Down, Modifiers::NONE, modes(true, false, false, false)),
            Some(bytes("\x1b[B"))
        );
    }

    // key Right -Shift-AnyMod+Ansi-AppCuKeys : "\E[C"
    #[test]
    fn right_ansi_normal_cursor_keys() {
        assert_eq!(
            encode_key(Right, Modifiers::NONE, modes(true, false, false, false)),
            Some(bytes("\x1b[C"))
        );
    }

    // key Left -Shift+AnyMod+Ansi : "\E[1;*D"  (Control+Left)
    #[test]
    fn left_ansi_control() {
        let m = Modifiers {
            control: true,
            ..Modifiers::NONE
        };
        assert_eq!(
            encode_key(Left, m, modes(true, true, false, false)),
            Some(bytes("\x1b[1;5D"))
        );
    }

    // key Up -Shift+AnyMod+Ansi : "\E[1;*A"  (Alt+Up, param = 1+2 = 3)
    #[test]
    fn up_ansi_alt() {
        let m = Modifiers {
            alt: true,
            ..Modifiers::NONE
        };
        assert_eq!(
            encode_key(Up, m, modes(true, false, false, false)),
            Some(bytes("\x1b[1;3A"))
        );
    }

    // key Up +Shift-AppScreen : scrollLineUp
    #[test]
    fn shift_up_scrolls_on_primary_screen() {
        let m = Modifiers {
            shift: true,
            ..Modifiers::NONE
        };
        assert_eq!(
            encode_key(Up, m, modes(true, false, false, false)),
            Some(KeyAction::ScrollLineUp)
        );
    }

    // key Prior +Shift-AppScreen : scrollPageUp
    #[test]
    fn shift_page_up_scrolls_on_primary_screen() {
        let m = Modifiers {
            shift: true,
            ..Modifiers::NONE
        };
        assert_eq!(
            encode_key(PageUp, m, modes(true, false, false, false)),
            Some(KeyAction::ScrollPageUp)
        );
    }

    // key Down +Shift-AppScreen : scrollLineDown
    #[test]
    fn shift_down_scrolls_on_primary_screen() {
        let m = Modifiers {
            shift: true,
            ..Modifiers::NONE
        };
        assert_eq!(
            encode_key(Down, m, modes(true, false, false, false)),
            Some(KeyAction::ScrollLineDown)
        );
    }

    // key Next +Shift-AppScreen : scrollPageDown
    #[test]
    fn shift_page_down_scrolls_on_primary_screen() {
        let m = Modifiers {
            shift: true,
            ..Modifiers::NONE
        };
        assert_eq!(
            encode_key(PageDown, m, modes(true, false, false, false)),
            Some(KeyAction::ScrollPageDown)
        );
    }

    // Shift+Up on the alternate screen: AppScreen gates scrolling off, and
    // the keytab has no other rule for this combination.
    #[test]
    fn shift_up_no_action_on_alt_screen() {
        let m = Modifiers {
            shift: true,
            ..Modifiers::NONE
        };
        assert_eq!(encode_key(Up, m, modes(true, false, false, true)), None);
    }

    // key Home -AnyMod -AppCuKeys : "\E[H"
    #[test]
    fn home_normal_cursor_keys() {
        assert_eq!(
            encode_key(Home, Modifiers::NONE, modes(true, false, false, false)),
            Some(bytes("\x1b[H"))
        );
    }

    // key End -AnyMod -AppCuKeys : "\E[F"
    #[test]
    fn end_normal_cursor_keys() {
        assert_eq!(
            encode_key(End, Modifiers::NONE, modes(true, false, false, false)),
            Some(bytes("\x1b[F"))
        );
    }

    // key Home -AnyMod +AppCuKeys : "\EOH"
    #[test]
    fn home_app_cursor_keys() {
        assert_eq!(
            encode_key(Home, Modifiers::NONE, modes(true, true, false, false)),
            Some(bytes("\x1bOH"))
        );
    }

    // key End -AnyMod +AppCuKeys : "\EOF"
    #[test]
    fn end_app_cursor_keys() {
        assert_eq!(
            encode_key(End, Modifiers::NONE, modes(true, true, false, false)),
            Some(bytes("\x1bOF"))
        );
    }

    // key Home +AnyMod : "\E[1;*H"  (Control+Home, param = 1+4 = 5)
    #[test]
    fn home_control() {
        let m = Modifiers {
            control: true,
            ..Modifiers::NONE
        };
        assert_eq!(
            encode_key(Home, m, modes(true, true, false, false)),
            Some(bytes("\x1b[1;5H"))
        );
    }

    // key End +AnyMod : "\E[1;*F"
    #[test]
    fn end_control() {
        let m = Modifiers {
            control: true,
            ..Modifiers::NONE
        };
        assert_eq!(
            encode_key(End, m, modes(true, true, false, false)),
            Some(bytes("\x1b[1;5F"))
        );
    }

    // key Insert -AnyMod : "\E[2~"
    #[test]
    fn insert_plain() {
        assert_eq!(
            encode_key(Insert, Modifiers::NONE, KeyboardModes::default()),
            Some(bytes("\x1b[2~"))
        );
    }

    // key Delete -AnyMod : "\E[3~"
    #[test]
    fn delete_plain() {
        assert_eq!(
            encode_key(Delete, Modifiers::NONE, KeyboardModes::default()),
            Some(bytes("\x1b[3~"))
        );
    }

    // key Insert +AnyMod : "\E[2;*~"  (Shift+Insert, param = 1+1 = 2)
    #[test]
    fn insert_shift() {
        let m = Modifiers {
            shift: true,
            ..Modifiers::NONE
        };
        assert_eq!(
            encode_key(Insert, m, KeyboardModes::default()),
            Some(bytes("\x1b[2;2~"))
        );
    }

    // key Delete +AnyMod : "\E[3;*~"
    #[test]
    fn delete_control() {
        let m = Modifiers {
            control: true,
            ..Modifiers::NONE
        };
        assert_eq!(
            encode_key(Delete, m, KeyboardModes::default()),
            Some(bytes("\x1b[3;5~"))
        );
    }

    // key Prior -Shift-AnyMod : "\E[5~"
    #[test]
    fn page_up_plain() {
        assert_eq!(
            encode_key(PageUp, Modifiers::NONE, KeyboardModes::default()),
            Some(bytes("\x1b[5~"))
        );
    }

    // key Next -Shift-AnyMod : "\E[6~"
    #[test]
    fn page_down_plain() {
        assert_eq!(
            encode_key(PageDown, Modifiers::NONE, KeyboardModes::default()),
            Some(bytes("\x1b[6~"))
        );
    }

    // key Prior -Shift+AnyMod : "\E[5;*~"  (Alt+PageUp, param = 1+2 = 3)
    #[test]
    fn page_up_alt() {
        let m = Modifiers {
            alt: true,
            ..Modifiers::NONE
        };
        assert_eq!(
            encode_key(PageUp, m, KeyboardModes::default()),
            Some(bytes("\x1b[5;3~"))
        );
    }

    // key Next -Shift+AnyMod : "\E[6;*~"
    #[test]
    fn page_down_control() {
        let m = Modifiers {
            control: true,
            ..Modifiers::NONE
        };
        assert_eq!(
            encode_key(PageDown, m, KeyboardModes::default()),
            Some(bytes("\x1b[6;5~"))
        );
    }

    // key F1 -AnyMod : "\EOP"  .. key F4 -AnyMod : "\EOS"
    #[test]
    fn f1_through_f4_plain() {
        assert_eq!(
            encode_key(F1, Modifiers::NONE, KeyboardModes::default()),
            Some(bytes("\x1bOP"))
        );
        assert_eq!(
            encode_key(F2, Modifiers::NONE, KeyboardModes::default()),
            Some(bytes("\x1bOQ"))
        );
        assert_eq!(
            encode_key(F3, Modifiers::NONE, KeyboardModes::default()),
            Some(bytes("\x1bOR"))
        );
        assert_eq!(
            encode_key(F4, Modifiers::NONE, KeyboardModes::default()),
            Some(bytes("\x1bOS"))
        );
    }

    // key F1 +AnyMod : "\EO*P"  (Shift+F1, param = 2)
    #[test]
    fn f1_shift() {
        let m = Modifiers {
            shift: true,
            ..Modifiers::NONE
        };
        assert_eq!(
            encode_key(F1, m, KeyboardModes::default()),
            Some(bytes("\x1bO2P"))
        );
    }

    // key F5 -AnyMod : "\E[15~"  .. key F12 -AnyMod : "\E[24~"
    #[test]
    fn f5_through_f12_plain() {
        let cases = [
            (F5, 15),
            (F6, 17),
            (F7, 18),
            (F8, 19),
            (F9, 20),
            (F10, 21),
            (F11, 23),
            (F12, 24),
        ];
        for (key, num) in cases {
            assert_eq!(
                encode_key(key, Modifiers::NONE, KeyboardModes::default()),
                Some(bytes(&format!("\x1b[{num}~"))),
                "{key:?}"
            );
        }
    }

    // key F12 +AnyMod : "\E[24;*~"  (Control+F12, param = 5)
    #[test]
    fn f12_control() {
        let m = Modifiers {
            control: true,
            ..Modifiers::NONE
        };
        assert_eq!(
            encode_key(F12, m, KeyboardModes::default()),
            Some(bytes("\x1b[24;5~"))
        );
    }

    // key Space +Control : "\x00"
    #[test]
    fn space_control() {
        let m = Modifiers {
            control: true,
            ..Modifiers::NONE
        };
        assert_eq!(
            encode_key(Space, m, KeyboardModes::default()),
            Some(KeyAction::Bytes(vec![0x00]))
        );
    }

    // Plain Space is not in the keytab (handled as ordinary text input).
    #[test]
    fn space_plain_unbound() {
        assert_eq!(
            encode_key(Space, Modifiers::NONE, KeyboardModes::default()),
            None
        );
    }

    // key ScrollLock : scrollLock
    #[test]
    fn scroll_lock() {
        assert_eq!(
            encode_key(ScrollLock, Modifiers::NONE, KeyboardModes::default()),
            Some(KeyAction::ScrollLock)
        );
    }

    // The winit adapter: same table, reached through winit::keyboard::Key.
    #[test]
    fn winit_adapter_escape() {
        assert_eq!(
            encode_winit_key(
                &Key::Named(NamedKey::Escape),
                Modifiers::NONE,
                KeyboardModes::default()
            ),
            Some(bytes("\x1b"))
        );
    }

    #[test]
    fn winit_adapter_f5() {
        assert_eq!(
            encode_winit_key(
                &Key::Named(NamedKey::F5),
                Modifiers::NONE,
                KeyboardModes::default()
            ),
            Some(bytes("\x1b[15~"))
        );
    }

    #[test]
    fn winit_adapter_unbound_character_returns_none() {
        assert_eq!(
            encode_winit_key(
                &Key::Character("a".into()),
                Modifiers::NONE,
                KeyboardModes::default()
            ),
            None
        );
    }
}
