//! The two selections a terminal writes to, and the store that holds them.
//!
//! X11 and Wayland have had two of them since the beginning: CLIPBOARD, which
//! `Ctrl+Shift+C` fills and `Ctrl+Shift+V` reads, and PRIMARY, which filling
//! is what selecting text *is* and which the middle button pastes. Keeping
//! them apart is the whole point of the pair: a selection made to paste
//! somewhere with the middle button must not throw away what the user copied
//! ten minutes ago. gnome-terminal and Konsole both do it this way.
//!
//! macOS and Windows have one pasteboard and no PRIMARY. There the selection
//! is remembered here instead, so a middle click still pastes what this
//! window last selected while the pasteboard the rest of the machine shares
//! stays exactly where the user left it.
//!
//! One [`arboard::Clipboard`] is held open for the life of the process, and
//! that is load-bearing rather than an economy. On X11 the selection lives
//! inside the application that owns it: `arboard`'s `Drop` hands CLIPBOARD to
//! a clipboard manager on the way out and then destroys its window, and
//! nothing anywhere takes PRIMARY. A handle made per call would therefore
//! leave PRIMARY empty the instant the copy returned.
//!
//! A surface with no window ([`crate::window::TerminalSurface::headless`])
//! has no display to reach, so both targets are the in-memory slots. That is
//! also what a test reads.

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

/// Which of the two selections a copy or a paste means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// What `Ctrl+Shift+C` fills and `Ctrl+Shift+V` reads.
    Clipboard,
    /// What selecting fills and the middle button pastes.
    Primary,
}

/// Where the platform half of the store stands.
enum Backend {
    /// There is a display. The handle is made on first use and then kept.
    Platform(Option<Box<Clipboard>>),
    /// There is not. The slots below are the whole store.
    Memory,
}

/// Both selections, and the platform handle behind them.
///
/// Every write lands in the in-memory slot as well as on the platform, so
/// `Ctrl+Shift+C` after a selection has something to copy without asking the
/// display for what this process just put there, and so a surface with no
/// display behaves the same way one slot at a time.
pub struct Store {
    backend: Backend,
    clipboard: Option<String>,
    primary: Option<String>,
}

impl Store {
    /// The store for a surface with a window on a display.
    pub fn platform() -> Self {
        Self {
            backend: Backend::Platform(None),
            clipboard: None,
            primary: None,
        }
    }

    /// The store for a surface with no display to reach.
    pub fn memory() -> Self {
        Self {
            backend: Backend::Memory,
            clipboard: None,
            primary: None,
        }
    }

    /// What this process last wrote to `target`. The record a test reads, and
    /// what `Ctrl+Shift+C` copies after a selection.
    pub fn last(&self, target: Target) -> Option<&str> {
        match target {
            Target::Clipboard => self.clipboard.as_deref(),
            Target::Primary => self.primary.as_deref(),
        }
    }

    pub fn set(&mut self, target: Target, text: &str) -> Result<(), ClipboardError> {
        match target {
            Target::Clipboard => self.clipboard = Some(text.to_owned()),
            Target::Primary => self.primary = Some(text.to_owned()),
        }
        let Some(handle) = self.handle() else {
            return Ok(());
        };
        match target {
            Target::Clipboard => handle.set_text(text.to_owned())?,
            Target::Primary => set_primary(handle, text)?,
        }
        Ok(())
    }

    pub fn get(&mut self, target: Target) -> Result<String, ClipboardError> {
        let remembered = self.last(target).map(str::to_owned);
        let Some(handle) = self.handle() else {
            return Ok(remembered.unwrap_or_default());
        };
        match target {
            Target::Clipboard => Ok(handle.get_text()?),
            // No PRIMARY on this platform: what this window last selected is
            // the answer, which is what the middle button is asking for.
            Target::Primary => match get_primary(handle) {
                Some(text) => Ok(text?),
                None => Ok(remembered.unwrap_or_default()),
            },
        }
    }

    /// The platform handle, made on first use. A display that cannot be
    /// reached leaves the store on its slots rather than failing the copy.
    fn handle(&mut self) -> Option<&mut Clipboard> {
        let Backend::Platform(held) = &mut self.backend else {
            return None;
        };
        if held.is_none() {
            match Clipboard::new() {
                Ok(clipboard) => *held = Some(Box::new(clipboard)),
                Err(e) => {
                    log::debug!("no clipboard on this display: {e}");
                    return None;
                }
            }
        }
        held.as_deref_mut()
    }
}

// -- PRIMARY, where the platform has one ----------------------------------
//
// The gate is `arboard`'s own: it offers the Linux extension traits on every
// unix that is not macOS, Android or emscripten, which is the set of targets
// whose window systems carry a second selection.

#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "android", target_os = "emscripten"))
))]
fn set_primary(handle: &mut Clipboard, text: &str) -> Result<(), arboard::Error> {
    use arboard::{LinuxClipboardKind, SetExtLinux};
    handle
        .set()
        .clipboard(LinuxClipboardKind::Primary)
        .text(text.to_owned())
}

#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "android", target_os = "emscripten"))
))]
fn get_primary(handle: &mut Clipboard) -> Option<Result<String, arboard::Error>> {
    use arboard::{GetExtLinux, LinuxClipboardKind};
    Some(handle.get().clipboard(LinuxClipboardKind::Primary).text())
}

/// macOS and Windows have one pasteboard, and writing a selection to it is
/// the behaviour this pair exists to avoid. The selection stays in the
/// store's own slot, where the middle button reads it.
#[cfg(not(all(
    unix,
    not(any(target_os = "macos", target_os = "android", target_os = "emscripten"))
)))]
fn set_primary(_handle: &mut Clipboard, _text: &str) -> Result<(), arboard::Error> {
    Ok(())
}

#[cfg(not(all(
    unix,
    not(any(target_os = "macos", target_os = "android", target_os = "emscripten"))
)))]
fn get_primary(_handle: &mut Clipboard) -> Option<Result<String, arboard::Error>> {
    None
}

/// Wrap `text` in DEC bracketed-paste markers (`\x1b[200~` ... `\x1b[201~`)
/// when the terminal has bracketed paste enabled, so the shell/program on
/// the other end can tell pasted text from typed text. Pure and unit
/// tested; a paste is expected to route its result through this before
/// writing to the pty.
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

    /// The two targets are two slots, and writing one leaves the other
    /// standing: that separation is the whole reason the pair exists.
    #[test]
    fn the_two_targets_do_not_write_over_each_other() {
        let mut store = Store::memory();
        store.set(Target::Clipboard, "copied").unwrap();
        store.set(Target::Primary, "selected").unwrap();

        assert_eq!(store.get(Target::Clipboard).unwrap(), "copied");
        assert_eq!(store.get(Target::Primary).unwrap(), "selected");
        assert_eq!(store.last(Target::Clipboard), Some("copied"));
    }

    /// Nothing written yet reads as nothing, rather than as the other slot.
    #[test]
    fn an_unwritten_target_is_empty() {
        let mut store = Store::memory();
        store.set(Target::Primary, "selected").unwrap();
        assert_eq!(store.get(Target::Clipboard).unwrap(), "");
        assert_eq!(store.last(Target::Clipboard), None);
    }
}
