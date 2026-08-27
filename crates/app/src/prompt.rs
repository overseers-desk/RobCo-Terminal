//! The line editor a question on the glass is answered into: what one
//! keystroke does to the answer, and what the glass owes the eye in return.
//!
//! The `picker.rs` shape, for the same reason: pure functions and one small
//! state, no session, no I/O, no transport. The surface holds the [`Line`],
//! feeds it keys ahead of the keytab, and puts the bytes it hands back onto
//! the asking channel's own grid. So the question, the answer and the cursor
//! all wear the phosphor and the curvature, and nothing new is rendered
//! anywhere.
//!
//! The echo rule is the whole security of the thing. An echoing line writes
//! back what was typed. A secret writes back nothing at all -- not asterisks:
//! an asterisk count is a length, a length is information about a password,
//! and a shoulder is enough to read it. The same silence covers a secret's
//! backspace, since a rub-out that erases a visible column would restore the
//! count the silence was hiding.
//!
//! A secret does not outlive its use either: [`Line`] zeroes its buffer on
//! drop, so an abandoned prompt leaves no passphrase in a freed page.

use winit::keyboard::{Key, NamedKey};

/// What one key did to the line.
#[derive(Debug, PartialEq, Eq)]
pub enum Stroke {
    /// The answer grew.
    Typed,
    /// The answer lost its last character, or was already empty.
    Backspace,
    /// The user is done: the answer is [`Line::take`]'s to hand over.
    Commit,
    /// The user withdrew the question. Nothing is answered.
    Cancel,
    /// Not the line's key: swallowed, so the question stays put.
    Ignored,
}

/// One question's answer as it is being typed.
pub struct Line {
    text: String,
    /// Whether the glass may show what is in `text`.
    echo: bool,
}

impl Line {
    /// An empty answer. `echo` false is a secret: a password, a passphrase,
    /// a keyboard-interactive prompt the server marked no-echo.
    pub fn new(echo: bool) -> Self {
        Self { text: String::new(), echo }
    }

    /// Read one key, and hand back what the glass owes for it.
    ///
    /// `text` is winit's own decoding of the event, which is what makes a
    /// space a space and a dead-key composition the character it composed;
    /// the logical key stands in when winit decoded nothing. Anything whose
    /// text carries a control character is not an answer's character --
    /// `Tab`, `Enter`'s own `\r`, a pasted escape -- and is ignored, so the
    /// three named keys below stay the only way out of the line.
    pub fn key(&mut self, logical: &Key, text: Option<&str>) -> (Stroke, Vec<u8>) {
        match logical {
            Key::Named(NamedKey::Backspace) => (Stroke::Backspace, self.rub_out()),
            Key::Named(NamedKey::Enter) => (Stroke::Commit, b"\r\n".to_vec()),
            // The line closes on Escape and the cursor leaves the question's
            // row, so whatever the caller says next says it on a row of its
            // own rather than after a half-typed answer.
            Key::Named(NamedKey::Escape) => (Stroke::Cancel, b"\r\n".to_vec()),
            _ => {
                let typed = text
                    .filter(|t| !t.is_empty())
                    .or(match logical {
                        Key::Character(c) => Some(c.as_str()),
                        _ => None,
                    })
                    .filter(|t| !t.chars().any(char::is_control));
                match typed {
                    Some(typed) => {
                        self.text.push_str(typed);
                        (Stroke::Typed, self.echoed(typed))
                    }
                    None => (Stroke::Ignored, Vec::<u8>::new()),
                }
            }
        }
    }

    /// A clipboard's worth at once, under the same echo rule.
    ///
    /// Control characters are dropped rather than obeyed: a password copied
    /// with its trailing newline is a password, not a password and a commit,
    /// and a paste is not where a line should decide it is finished.
    pub fn paste(&mut self, text: &str) -> Vec<u8> {
        let kept: String = text.chars().filter(|c| !c.is_control()).collect();
        self.text.push_str(&kept);
        self.echoed(&kept)
    }

    /// The answer, leaving the line empty behind it.
    pub fn take(&mut self) -> String {
        std::mem::take(&mut self.text)
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Whether the glass is showing this answer.
    pub fn echo(&self) -> bool {
        self.echo
    }

    /// Backspace-space-backspace: the rub-out a terminal has always spelled,
    /// and nothing at all when there is no column to rub out or no column
    /// the eye was given in the first place.
    fn rub_out(&mut self) -> Vec<u8> {
        match self.text.pop() {
            Some(_) if self.echo => b"\x08 \x08".to_vec(),
            _ => Vec::new(),
        }
    }

    fn echoed(&self, typed: &str) -> Vec<u8> {
        if self.echo {
            typed.as_bytes().to_vec()
        } else {
            Vec::new()
        }
    }
}

impl Drop for Line {
    fn drop(&mut self) {
        // The buffer is overwritten before it is freed: a passphrase the
        // user abandoned should not still be legible in the allocator's
        // pages. NUL is valid UTF-8, so the string stays a string while it
        // is being erased; the fence is there because a write nobody reads
        // is exactly what an optimiser is entitled to delete.
        let bytes = unsafe { self.text.as_mut_vec() };
        for byte in bytes.iter_mut() {
            *byte = 0;
        }
        std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
        bytes.clear();
    }
}

/// The question as bytes for the asking channel's parser.
///
/// Newlines become carriage-return newlines, because a grid in its ordinary
/// mode moves down without moving left. The text is written at normal
/// intensity: a notice is dim because it is the transport talking about
/// itself, and a question is the program talking to the user, who has to be
/// in no doubt it is their turn.
pub fn paint(prompt: &str) -> Vec<u8> {
    let mut out = String::from("\x1b[0m");
    out.push_str(&prompt.replace('\n', "\r\n"));
    out.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(s: &str) -> Key {
        Key::Character(s.into())
    }

    fn named(key: NamedKey) -> Key {
        Key::Named(key)
    }

    #[test]
    fn an_echoing_line_shows_what_it_collects() {
        let mut line = Line::new(true);
        assert!(line.is_empty());
        assert_eq!(line.key(&ch("h"), Some("h")), (Stroke::Typed, b"h".to_vec()));
        assert_eq!(line.key(&ch("i"), Some("i")), (Stroke::Typed, b"i".to_vec()));
        assert_eq!(
            line.key(&named(NamedKey::Space), Some(" ")),
            (Stroke::Typed, b" ".to_vec())
        );
        assert_eq!(
            line.key(&named(NamedKey::Backspace), None),
            (Stroke::Backspace, b"\x08 \x08".to_vec())
        );
        assert_eq!(
            line.key(&named(NamedKey::Enter), Some("\r")),
            (Stroke::Commit, b"\r\n".to_vec())
        );
        assert_eq!(line.take(), "hi");
        assert!(line.is_empty(), "taking the answer empties the line");
    }

    #[test]
    fn a_secret_gives_the_eye_nothing_and_still_collects() {
        let mut line = Line::new(false);
        for c in ["s", "3", "c"] {
            assert_eq!(line.key(&ch(c), Some(c)), (Stroke::Typed, Vec::<u8>::new()));
        }
        assert_eq!(
            line.key(&named(NamedKey::Backspace), None),
            (Stroke::Backspace, Vec::<u8>::new()),
            "a rub-out that moved a column would give the length away"
        );
        assert_eq!(
            line.key(&named(NamedKey::Enter), None),
            (Stroke::Commit, b"\r\n".to_vec()),
            "the commit still leaves the question's row"
        );
        assert_eq!(line.take(), "s3");
    }

    #[test]
    fn a_backspace_on_an_empty_line_owes_the_glass_nothing() {
        let mut line = Line::new(true);
        assert_eq!(
            line.key(&named(NamedKey::Backspace), None),
            (Stroke::Backspace, Vec::<u8>::new()),
            "there is no column to rub out, and the question above is not one"
        );
        assert!(line.is_empty());
    }

    #[test]
    fn escape_cancels_and_the_rest_is_swallowed() {
        let mut line = Line::new(true);
        assert_eq!(
            line.key(&named(NamedKey::Escape), None),
            (Stroke::Cancel, b"\r\n".to_vec())
        );
        assert_eq!(
            line.key(&named(NamedKey::ArrowLeft), None),
            (Stroke::Ignored, Vec::<u8>::new())
        );
        assert_eq!(
            line.key(&named(NamedKey::Tab), Some("\t")),
            (Stroke::Ignored, Vec::<u8>::new()),
            "a control character is not an answer's character"
        );
        assert!(line.is_empty());
    }

    #[test]
    fn a_paste_obeys_the_same_echo_rule_and_commits_nothing() {
        let mut shown = Line::new(true);
        assert_eq!(shown.paste("yes\n"), b"yes".to_vec());
        assert_eq!(shown.take(), "yes");

        let mut secret = Line::new(false);
        assert_eq!(secret.paste("hunter2\n"), Vec::<u8>::new());
        assert_eq!(secret.take(), "hunter2", "the newline was not a commit");
    }

    #[test]
    fn the_question_is_painted_at_normal_intensity_with_grid_newlines() {
        let bytes = paint("cannot be established.\nType yes or no: ");
        let page = String::from_utf8(bytes).unwrap();
        assert!(page.starts_with("\x1b[0m"), "a question is not the transport muttering");
        assert!(page.contains("established.\r\nType yes"), "{page}");
        assert!(!page.contains("\x1b[2m"));
    }
}
