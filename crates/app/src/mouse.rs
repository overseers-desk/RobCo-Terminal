//! Mouse reporting: X10 and SGR extended mouse-tracking protocol encoding.
//!
//! Pure encoders, with no window/event-loop state, matching xterm's two mouse
//! wire formats. Konsole supports both; this module is the byte-level encode
//! step the app calls once it has decided *whether* to report (tracking mode
//! enabled) and has an already-distorted-corrected grid coordinate (see
//! [`crate::distortion`]).
//!
//! The modifier state is [`crate::input::Modifiers`], the same type the
//! keyboard encoder takes: there is one keyboard under the user's hands,
//! and a mouse report reads three of the same four keys it does. Meta has
//! no bit in either wire format, so it is simply not consulted here.

use crate::input::Modifiers;

/// Which mouse button/action a report describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    /// Release, in protocols that need an explicit release code (X10).
    Release,
    WheelUp,
    WheelDown,
}

fn button_code(button: MouseButton) -> u8 {
    match button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
        MouseButton::Release => 3,
        MouseButton::WheelUp => 64,
        MouseButton::WheelDown => 65,
    }
}

fn modifier_bits(m: Modifiers) -> u8 {
    (m.shift as u8 * 4) | (m.alt as u8 * 8) | (m.control as u8 * 16)
}

/// X10-compatible mouse reporting (`\x1b[M` + 3 bytes): button, then
/// 1-based column and row, each offset by 33 (32 for the offset, +1 for
/// 1-based). Coordinates beyond 223 (255 - 32) cannot be represented and are
/// clamped, matching xterm's own X10 behavior.
pub fn encode_x10(button: MouseButton, col: u16, row: u16, modifiers: Modifiers) -> Vec<u8> {
    let cb = button_code(button) | modifier_bits(modifiers);
    let cx = (col.saturating_add(1)).min(223).saturating_add(32) as u8;
    let cy = (row.saturating_add(1)).min(223).saturating_add(32) as u8;
    vec![0x1b, b'[', b'M', cb.wrapping_add(32), cx, cy]
}

/// SGR extended mouse reporting (`\x1b[<{cb};{col};{row}M` on press,
/// trailing `m` on release). No coordinate ceiling, unlike X10.
pub fn encode_sgr(
    button: MouseButton,
    col: u16,
    row: u16,
    modifiers: Modifiers,
    pressed: bool,
) -> Vec<u8> {
    let cb = button_code(button) | modifier_bits(modifiers);
    let trailer = if pressed { 'M' } else { 'm' };
    format!("\x1b[<{};{};{}{}", cb, col + 1, row + 1, trailer).into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn x10_left_press_origin() {
        let bytes = encode_x10(MouseButton::Left, 0, 0, Modifiers::default());
        assert_eq!(bytes, vec![0x1b, b'[', b'M', 32, 33, 33]);
    }

    #[test]
    fn x10_right_press_with_shift() {
        let bytes = encode_x10(
            MouseButton::Right,
            4,
            9,
            Modifiers {
                shift: true,
                ..Default::default()
            },
        );
        // button 2 | shift(4) = 6, +32 = 38
        assert_eq!(bytes, vec![0x1b, b'[', b'M', 38, 5 + 32, 10 + 32]);
    }

    #[test]
    fn x10_coordinate_clamped() {
        let bytes = encode_x10(MouseButton::Left, 500, 500, Modifiers::default());
        assert_eq!(bytes[4], 223u8.wrapping_add(32));
        assert_eq!(bytes[5], 223u8.wrapping_add(32));
    }

    #[test]
    fn sgr_left_press() {
        let bytes = encode_sgr(MouseButton::Left, 0, 0, Modifiers::default(), true);
        assert_eq!(bytes, b"\x1b[<0;1;1M");
    }

    #[test]
    fn sgr_left_release() {
        let bytes = encode_sgr(MouseButton::Left, 0, 0, Modifiers::default(), false);
        assert_eq!(bytes, b"\x1b[<0;1;1m");
    }

    #[test]
    fn sgr_wheel_up_with_control() {
        let bytes = encode_sgr(
            MouseButton::WheelUp,
            10,
            20,
            Modifiers {
                control: true,
                ..Default::default()
            },
            true,
        );
        // 64 | control(16) = 80
        assert_eq!(bytes, b"\x1b[<80;11;21M");
    }

    #[test]
    fn sgr_large_coordinates_not_clamped() {
        let bytes = encode_sgr(MouseButton::Left, 9999, 9999, Modifiers::default(), true);
        assert_eq!(bytes, b"\x1b[<0;10000;10000M");
    }
}
