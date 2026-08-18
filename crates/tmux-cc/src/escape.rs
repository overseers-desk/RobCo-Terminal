//! How a `%output` payload carries arbitrary bytes on a line protocol.
//!
//! # The set, measured
//!
//! `man tmux` says of `%output`: "value escapes non-printable characters and
//! backslash as octal \xxx". That sentence is too generous by two ranges. A
//! pane was made to write all 256 byte values through tmux 3.5a
//! (`tests/transcripts/04-output-octal.txt` is the recording) and the answer is
//! exact:
//!
//! * `0x00..=0x1f` and `0x5c` (`\`) are escaped, as a backslash and **three**
//!   octal digits, always three, zero-padded.
//! * `0x20..=0x7e` pass through, as one would expect.
//! * `0x7f` (DEL) passes through **raw**, though it is not printable.
//! * `0x80..=0xff` pass through **raw**, every one of them.
//!
//! Two consequences the rest of the crate is built on. First, a payload is
//! bytes and not text: any UTF-8 above ASCII crosses the wire as its own raw
//! bytes, and so does a byte sequence that is not UTF-8 at all, so decoding to
//! a `String` anywhere on this path would corrupt it. Second, the decoding is
//! unambiguous without lookahead beyond four bytes, because a literal
//! backslash never reaches the wire as itself: `\` followed by three octal
//! digits is always an escape.
//!
//! Because 0x0a and 0x0d are both inside the escaped range, a payload can
//! never contain a raw newline or carriage return, which is what makes the
//! whole protocol safely line-split (see `codec`).
//!
//! The decoder works byte-oriented rather than text-oriented for the reason
//! above.

/// Which bytes tmux escapes: the C0 controls and the backslash.
pub fn is_escaped(byte: u8) -> bool {
    byte < 0x20 || byte == b'\\'
}

/// Decode a `%output` payload into the bytes the pane actually wrote.
///
/// A backslash followed by three octal digits is that byte. Anything else,
/// including a backslash that is not followed by three octal digits, passes
/// through as itself: tmux does not emit such a thing, and swallowing it would
/// lose a byte that some other server dialect meant literally. `\400` and up
/// name no byte, so they are literal text by the same rule.
pub fn unescape_octal(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len());
    let mut i = 0;
    while i < payload.len() {
        if payload[i] == b'\\' && i + 3 < payload.len() {
            let digits = &payload[i + 1..i + 4];
            // `<= b'3'` on the first digit, not merely octal: three octal
            // digits reach 0o777, and only the first 256 of those are bytes.
            let fits = digits[0] <= b'3' && digits.iter().all(|d| (b'0'..=b'7').contains(d));
            if fits {
                let value =
                    ((digits[0] - b'0') << 6) | ((digits[1] - b'0') << 3) | (digits[2] - b'0');
                out.push(value);
                i += 4;
                continue;
            }
        }
        out.push(payload[i]);
        i += 1;
    }
    out
}

/// The inverse: the payload tmux would have written for these bytes.
///
/// Not sent anywhere: a control client never writes a `%output` line. It
/// exists so the round trip is testable in both directions and so a fixture
/// can be checked against what tmux actually emitted, byte for byte.
pub fn escape_octal(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for &byte in bytes {
        if is_escaped(byte) {
            out.push(b'\\');
            out.push(b'0' + (byte >> 6));
            out.push(b'0' + ((byte >> 3) & 7));
            out.push(b'0' + (byte & 7));
        } else {
            out.push(byte);
        }
    }
    out
}

/// Decode a name field: the other escaping, the one the manual does not
/// mention at all.
///
/// Window and session names do **not** use the `%output` scheme. They come
/// through tmux's `utf8_stravis`, which is BSD `vis(3)` in its C style, and
/// that is a different alphabet. Measured against tmux 3.5a by renaming a
/// window to each awkward thing in turn
/// (`tests/transcripts/03-rename.txt`):
///
/// | written | on the wire |
/// |---|---|
/// | `back\slash` | `back\\slash` |
/// | tab, BEL | `\t`, `\a` |
/// | `你好` (valid UTF-8) | the raw bytes |
/// | `0xff 0xfe` (not UTF-8) | `\377\376` |
/// | `"` and `$` | themselves |
///
/// So a backslash arrives doubled and a control byte arrives as a C escape,
/// and a client that takes the name verbatim shows `a\\b` for a window called
/// `a\b`, which is a live cosmetic defect in this crate's channel bank.
///
/// Which fields: measured on `%window-renamed`, `%session-renamed`,
/// `%session-changed` and `%client-session-changed`. Measured **not** to
/// apply to `%paste-buffer-changed`/`-deleted` (a buffer named `buf\back`
/// arrives with one backslash) or to `%message` (`msg \ back` arrives as
/// itself), so those are left alone. Applying this to a field tmux did not
/// encode would eat every backslash a user typed.
///
/// Unrecognised escapes pass through with their backslash, on the same rule
/// as [`unescape_octal`]: nothing is dropped.
pub fn unvis(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'\\' || i + 1 >= bytes.len() {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        // `\ooo` first, so `\000` is a NUL rather than `\0` followed by "00".
        if i + 3 < bytes.len() {
            let d = &bytes[i + 1..i + 4];
            if d[0] <= b'3' && d.iter().all(|b| (b'0'..=b'7').contains(b)) {
                out.push(((d[0] - b'0') << 6) | ((d[1] - b'0') << 3) | (d[2] - b'0'));
                i += 4;
                continue;
            }
        }
        let decoded = match bytes[i + 1] {
            b'\\' => Some(b'\\'),
            b'a' => Some(0x07),
            b'b' => Some(0x08),
            b'f' => Some(0x0c),
            b'n' => Some(0x0a),
            b'r' => Some(0x0d),
            b't' => Some(0x09),
            b'v' => Some(0x0b),
            b's' => Some(b' '),
            b'0' => Some(0),
            _ => None,
        };
        match decoded {
            Some(byte) => {
                out.push(byte);
                i += 2;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    out
}

/// A name field as text: [`unvis`], then lossy UTF-8.
pub fn unvis_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(&unvis(bytes)).into_owned()
}

/// The bytes of a `send-keys -H` argument list: each byte as two lowercase
/// hex digits, space-separated, with the leading space that joins it to the
/// command.
///
/// Hexadecimal rather than text is what makes quoting a
/// non-question: no byte a user can type (a quote, a backslash, a `#`, a
/// newline, a NUL) can reach tmux's command lexer as anything but a pair of
/// hex digits. See `command::quote_format` for the arguments that cannot dodge
/// the lexer this way.
pub fn hex_arguments(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 3);
    for &byte in bytes {
        out.push(' ');
        out.push(char::from_digit((byte >> 4) as u32, 16).unwrap());
        out.push(char::from_digit((byte & 0xf) as u32, 16).unwrap());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_byte_survives_the_round_trip() {
        let all: Vec<u8> = (0..=255u8).collect();
        assert_eq!(unescape_octal(&escape_octal(&all)), all);
    }

    #[test]
    fn the_escaped_set_is_the_measured_one() {
        // Exactly the bytes `tests/transcripts/04-output-octal.txt` shows
        // escaped, and no others: DEL and the whole high half stay raw.
        for byte in 0..=255u8 {
            let escaped = escape_octal(&[byte]).len() == 4;
            let expected = byte < 0x20 || byte == b'\\';
            assert_eq!(escaped, expected, "byte {byte:#04x}");
        }
    }

    #[test]
    fn the_three_digits_are_zero_padded() {
        assert_eq!(escape_octal(b"\x00\x07\x1b\\"), b"\\000\\007\\033\\134");
    }

    #[test]
    fn a_run_between_escapes_is_kept_whole() {
        assert_eq!(
            unescape_octal(b"back\\134slash\\015\\012"),
            b"back\\slash\r\n"
        );
    }

    #[test]
    fn high_bytes_pass_through_undecoded() {
        // The UTF-8 of "\u{4f60}\u{597d}", raw on the wire.
        let raw = b"\xe4\xbd\xa0\xe5\xa5\xbd";
        assert_eq!(unescape_octal(raw), raw);
        assert_eq!(escape_octal(raw), raw);
    }

    #[test]
    fn a_lone_backslash_is_itself() {
        // Not a thing tmux emits; the rule is that no byte is ever dropped.
        assert_eq!(unescape_octal(b"a\\b"), b"a\\b");
        assert_eq!(unescape_octal(b"\\99"), b"\\99");
        assert_eq!(unescape_octal(b"\\01"), b"\\01");
        assert_eq!(unescape_octal(b"\\189"), b"\\189");
        // Octal, three digits, and still not a byte.
        assert_eq!(unescape_octal(b"\\400"), b"\\400");
        assert_eq!(unescape_octal(b"\\777"), b"\\777");
    }

    #[test]
    fn a_fourth_digit_is_not_part_of_the_escape() {
        assert_eq!(unescape_octal(b"\\1234"), b"\x534");
    }

    #[test]
    fn a_name_uses_c_escapes_not_the_output_scheme() {
        // Every row of the table in `unvis`'s doc comment.
        assert_eq!(unvis(b"back\\\\slash"), b"back\\slash");
        assert_eq!(unvis(b"tab\\there bell\\aend"), b"tab\there bell\x07end");
        assert_eq!(unvis(b"utf8 \xe4\xbd\xa0 end"), b"utf8 \xe4\xbd\xa0 end");
        assert_eq!(unvis(b"high \\377\\376 end"), b"high \xff\xfe end");
        assert_eq!(unvis(b"quote \" and $dollar"), b"quote \" and $dollar");
    }

    #[test]
    fn the_c_escape_alphabet_is_whole() {
        assert_eq!(
            unvis(b"\\a\\b\\f\\n\\r\\t\\v\\s\\0"),
            b"\x07\x08\x0c\x0a\x0d\x09\x0b \x00"
        );
        // The octal form wins over `\0`, so a NUL before a digit still reads.
        assert_eq!(unvis(b"\\0007"), b"\x007");
    }

    #[test]
    fn an_unknown_name_escape_keeps_its_backslash() {
        assert_eq!(unvis(b"a\\qb"), b"a\\qb");
        assert_eq!(unvis(b"trailing\\"), b"trailing\\");
    }

    #[test]
    fn the_two_escapings_are_not_each_other() {
        // A payload's backslash is `\134`; a name's is `\\`. Reading either
        // with the other's rule loses the byte.
        assert_eq!(unescape_octal(b"a\\134b"), b"a\\b");
        assert_eq!(unvis(b"a\\134b"), b"a\\b");
        assert_eq!(unescape_octal(b"a\\\\b"), b"a\\\\b");
        assert_eq!(unvis(b"a\\\\b"), b"a\\b");
    }

    #[test]
    fn hex_arguments_are_two_digits_each() {
        assert_eq!(hex_arguments(b"hi\r"), " 68 69 0d");
        assert_eq!(hex_arguments(&[0x00, 0xff]), " 00 ff");
        assert_eq!(hex_arguments(b""), "");
    }
}
