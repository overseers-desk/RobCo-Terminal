//! The session variant a tmux window feeds: `%output` in, `send-keys` out.
//!
//! [`super::session::Session`] is PTY-only: its read loop, its EOF law and
//! its resize all assume a child on a master fd. A remote channel has none of
//! that: its bytes arrive as `%output` payloads peeled off the gateway's
//! control stream, and its keystrokes leave as `send-keys` commands on that
//! same stream. Bending `app::channels` around that difference was a design
//! that was ruled out; the variant lives here instead, beside the
//! PTY session, and [`ChannelSession`] is the one type a channel slot holds.
//!
//! What a [`RemoteSession`] deliberately does not have:
//!
//! * **A DCS tap.** The envelope lives on the gateway's own PTY; a pane's
//!   payload is already inside it. A `tmux -CC` run *inside* a remote window
//!   is tmux's own nesting problem, refused by tmux itself.
//! * **A transport.** Keystrokes buffer in the session and the host drains
//!   them to the gateway ([`RemoteSession::take_input`]); the session cannot
//!   name the pane it is, because the pane id is the channel row's
//!   (`app::channels::Row::pane_id`) and panes move under a window
//!   (`%window-pane-changed`) without the session noticing.
//! * **An EOF.** A remote window ends when tmux says `%window-close`, a model
//!   transition, not a session state.

use std::time::Instant;

use rio_vt::ansi::CursorShape;
use rio_vt::crosswords::Crosswords;
use rio_vt::event::{VoidListener, WindowId};
use rio_vt::performer::handler::Processor;

use crate::dcs::DcsTap;
use crate::session::{Pumped, Session, Term};
use crate::size::TermSize;

/// A channel fed by a tmux pane rather than a PTY.
pub struct RemoteSession {
    term: Term,
    processor: Processor,
    size: TermSize,
    /// Keystrokes waiting to become `send-keys`. See [`Self::take_input`].
    input: Vec<u8>,
}

impl RemoteSession {
    /// An empty screen of the glass's geometry.
    ///
    /// The geometry is the glass's and not the pane's, deliberately: every
    /// channel is the same rectangle of glass, and tmux is told that size
    /// once per client (`refresh-client -C`, the client-size law in
    /// `app::channels`' module doc), so pane and glass agree except for
    /// split windows, which the channel model does not draw.
    pub fn new(size: TermSize, scrollback: usize) -> Self {
        Self {
            term: Crosswords::new(
                size,
                CursorShape::Block,
                VoidListener {},
                WindowId::from(0u64),
                0,
                scrollback,
            ),
            processor: Processor::default(),
            size,
            input: Vec::new(),
        }
    }

    /// Apply bytes the gateway routed here: a `%output` payload, or the
    /// capture-and-cursor bootstrap the gateway synthesises on attach.
    pub fn feed(&mut self, bytes: &[u8]) {
        self.processor.advance(&mut self.term, bytes);
    }

    /// Queue keystrokes for the pane. The write side of the diversion: the
    /// host drains this to `Gateway::send_keys` on its next pump.
    pub fn write(&mut self, bytes: &[u8]) {
        self.input.extend_from_slice(bytes);
    }

    /// Drain what [`Self::write`] queued.
    pub fn take_input(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.input)
    }

    /// Reflow the grid. There is no `TIOCSWINSZ` half: the program behind
    /// this screen sits on tmux's PTY, and tmux resizes it from the client
    /// size the host publishes.
    pub fn resize(&mut self, size: TermSize) {
        if size == self.size {
            return;
        }
        self.size = size;
        self.term.resize(size);
    }

    pub fn size(&self) -> TermSize {
        self.size
    }

    pub fn term(&self) -> &Term {
        &self.term
    }

    pub fn term_mut(&mut self) -> &mut Term {
        &mut self.term
    }
}

/// What a channel slot holds: a shell of this machine's, or a tmux window.
///
/// The methods are the surface `app::window` already drove when every slot
/// was a [`Session`]; each match arm says what the operation means on the
/// side that has no native one.
pub enum ChannelSession<T: DcsTap> {
    /// A PTY-backed session: a local shell, or the anchor whose PTY carries
    /// the control stream.
    Local(Session<T>),
    /// A tmux window's screen, fed by the gateway.
    Remote(RemoteSession),
}

impl<T: DcsTap> ChannelSession<T> {
    /// Drain the PTY. A remote session has none; its bytes arrive through
    /// [`RemoteSession::feed`] on the gateway's pump, so this is idle.
    pub fn pump(&mut self) -> Pumped {
        match self {
            ChannelSession::Local(s) => s.pump(),
            ChannelSession::Remote(_) => Pumped::default(),
        }
    }

    /// Bytes from the keyboard (or a paste, or a mouse report). Local goes
    /// to the child; remote queues for `send-keys`.
    pub fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        match self {
            ChannelSession::Local(s) => s.write(bytes),
            ChannelSession::Remote(s) => {
                s.write(bytes);
                Ok(())
            }
        }
    }

    pub fn resize(&mut self, size: TermSize) -> std::io::Result<()> {
        match self {
            ChannelSession::Local(s) => s.resize(size),
            ChannelSession::Remote(s) => {
                s.resize(size);
                Ok(())
            }
        }
    }

    pub fn term(&self) -> &Term {
        match self {
            ChannelSession::Local(s) => s.term(),
            ChannelSession::Remote(s) => s.term(),
        }
    }

    pub fn term_mut(&mut self) -> &mut Term {
        match self {
            ChannelSession::Local(s) => s.term_mut(),
            ChannelSession::Remote(s) => s.term_mut(),
        }
    }

    /// When the pending synchronized update expires. Only a PTY session
    /// holds one: a remote screen's BSU/ESU pass through tmux, which runs
    /// its own timeout server-side.
    pub fn sync_deadline(&self) -> Option<Instant> {
        match self {
            ChannelSession::Local(s) => s.sync_deadline(),
            ChannelSession::Remote(_) => None,
        }
    }

    /// How many writes this slot has thrown away for want of queue
    /// ([`Session::sheds`]).
    ///
    /// Zero on a remote window, and not because nobody counted: a remote
    /// session's input queue is drained whole by the host on every pump
    /// ([`RemoteSession::take_input`]), so it holds one pump's typing and has no
    /// cap to shed against. What can shed on that path is the gateway's own
    /// command queue, one level up, and the gateway counts that itself.
    pub fn sheds(&self) -> u64 {
        match self {
            ChannelSession::Local(s) => s.sheds(),
            ChannelSession::Remote(_) => 0,
        }
    }

    pub fn local_mut(&mut self) -> Option<&mut Session<T>> {
        match self {
            ChannelSession::Local(s) => Some(s),
            ChannelSession::Remote(_) => None,
        }
    }

    pub fn remote_mut(&mut self) -> Option<&mut RemoteSession> {
        match self {
            ChannelSession::Local(_) => None,
            ChannelSession::Remote(s) => Some(s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewport_text;
    use rio_vt::crosswords::grid::Dimensions;

    fn size() -> TermSize {
        TermSize::new(20, 5, 9, 18)
    }

    #[test]
    fn fed_bytes_land_on_the_grid_and_input_queues_until_drained() {
        let mut s = RemoteSession::new(size(), 100);
        s.feed(b"hello \x1b[1mworld\x1b[0m");
        assert!(viewport_text(s.term())[0].starts_with("hello world"));

        s.write(b"ls");
        s.write(b"\r");
        assert_eq!(s.take_input(), b"ls\r");
        assert!(s.take_input().is_empty(), "a drain drains");
    }

    #[test]
    fn a_resize_reflows_the_grid_alone() {
        let mut s = RemoteSession::new(size(), 100);
        s.feed(b"abc");
        s.resize(TermSize::new(40, 10, 9, 18));
        assert_eq!(s.term().grid.columns(), 40);
        assert!(viewport_text(s.term())[0].starts_with("abc"));
    }
}
