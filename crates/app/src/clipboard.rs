//! Clipboard access, thin wrapper over `arboard`.
//!
//! `arboard::Clipboard` talks to the platform clipboard (X11/Wayland
//! selection, macOS pasteboard, Windows clipboard) and cannot be
//! constructed headlessly in CI, so this module keeps the untestable part
//! to two one-line methods and puts anything with logic (bracketed-paste
//! wrapping) in a pure helper that *is* unit tested.

use arboard::Clipboard;

#[derive(Debug)]
pub struct ClipboardError(pub String);

impl std::fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "clipboard error: {}", self.0)
    }
}

impl std::error::Error for ClipboardError {}

impl From<arboard::Error> for ClipboardError {
    fn from(e: arboard::Error) -> Self {
        ClipboardError(e.to_string())
    }
}

/// Copy `text` to the system clipboard.
pub fn copy(text: &str) -> Result<(), ClipboardError> {
    let mut clipboard = Clipboard::new()?;
    clipboard.set_text(text.to_owned())?;
    Ok(())
}

/// Read the system clipboard as text.
pub fn paste() -> Result<String, ClipboardError> {
    let mut clipboard = Clipboard::new()?;
    Ok(clipboard.get_text()?)
}

/// Wrap `text` in DEC bracketed-paste markers (`\x1b[200~` ... `\x1b[201~`)
/// when the terminal has bracketed paste enabled, so the shell/program on
/// the other end can tell pasted text from typed text. Pure and unit
/// tested; `paste()` above is expected to route its result through this
/// before writing to the pty.
pub fn bracket_paste(text: &str, bracketed_paste_enabled: bool) -> Vec<u8> {
    if !bracketed_paste_enabled {
        return text.as_bytes().to_vec();
    }
    let mut out = Vec::with_capacity(text.len() + 12);
    out.extend_from_slice(b"\x1b[200~");
    out.extend_from_slice(text.as_bytes());
    out.extend_from_slice(b"\x1b[201~");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bracket_paste_disabled_passes_through() {
        assert_eq!(bracket_paste("hello", false), b"hello".to_vec());
    }

    #[test]
    fn bracket_paste_enabled_wraps() {
        assert_eq!(bracket_paste("hi", true), b"\x1b[200~hi\x1b[201~".to_vec());
    }

    #[test]
    fn bracket_paste_empty_string() {
        assert_eq!(bracket_paste("", true), b"\x1b[200~\x1b[201~".to_vec());
    }
}
