//! The session variant an SSH channel feeds: remote bytes in, keystrokes
//! straight to the wire.
//!
//! Where [`super::session::Session`] owns a PTY and [`super::tmux_pane::TmuxPane`]
//! owns nothing but a screen, this variant owns a wire it can speak to.
//! That puts it on the PTY session's side of every seam the two differ on:
//! its bytes are its own (drained here, not routed by a gateway), its
//! writes name their destination (no queue-and-drain), its resize has a far
//! half (`window_change` where the PTY does `TIOCSWINSZ`), and its end is
//! session state (the wire's `Eof`, where a pane waits on a model
//! transition).
//!
//! The wire itself stays behind [`SshWire`], a trait with no SSH types in
//! it, so this crate compiles with no transport, no runtime and no crypto
//! under it: tests feed a `Vec`-backed fake, the app hands in the real
//! adapter, and the parser cannot tell them apart.
//!
//! The DCS tap is here from the start, not deferred: a `tmux -CC` typed on
//! a remote shell must be detectable exactly as on a local one, and the
//! tap at ground state is a byte-scan no-op.

use std::time::Instant;

use rio_vt::ansi::CursorShape;
use rio_vt::crosswords::Crosswords;
use rio_vt::event::{VoidListener, WindowId};
use rio_vt::performer::handler::Processor;

use crate::dcs::{DcsParser, DcsTap};
use crate::session::{Pumped, Term};
use crate::size::TermSize;

/// What the wire has for the grid. The transport's progress and failure
/// text arrives already rendered as `Data`: how a notice looks on the
/// glass is the adapter's decision, not this crate's.
#[derive(Debug)]
pub enum SshEvent {
    /// Bytes for the parser: remote output, or a rendered notice.
    Data(Vec<u8>),
    /// The remote command's exit status. Carried for a consumer that
    /// wants it; not printed, for parity with a local shell's exit.
    ExitStatus(u32),
    /// The channel is over: remote close, or the connection died.
    Eof,
}

/// The transport as this crate is allowed to see it.
pub trait SshWire: Send {
    /// The next event, if one is waiting. Never blocks.
    fn try_event(&mut self) -> Option<SshEvent>;
    /// Keystrokes, paste, reports: whole or not at all, the writer sheds
    /// and counts past its budget.
    fn send(&mut self, bytes: &[u8]);
    /// Tell the remote pty the glass changed shape.
    fn window_change(&mut self, cols: u16, rows: u16, pix_w: u16, pix_h: u16);
    /// Whole writes the wire refused for want of budget.
    fn sheds(&self) -> u64;
}

/// A channel fed by an SSH connection rather than a PTY.
pub struct SshChannel<T: DcsTap> {
    term: Term,
    processor: Processor,
    dcs: DcsParser<T>,
    size: TermSize,
    wire: Box<dyn SshWire>,
    eof: bool,
}

impl<T: DcsTap> SshChannel<T> {
    /// An empty screen of the glass's geometry, listening on the wire.
    pub fn new(size: TermSize, scrollback: usize, tap: T, wire: Box<dyn SshWire>) -> Self {
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
            dcs: DcsParser::new(tap),
            size,
            wire,
            eof: false,
        }
    }

    /// Drain what the connection thread handed over and apply it.
    ///
    /// Every chunk goes to both consumers, tap then grid, exactly as the
    /// PTY pump does; the sync timeout is honoured here for the same
    /// reason it is there: owning the loop means nothing else will.
    pub fn pump(&mut self) -> Pumped {
        let mut out = Pumped::default();
        if self.eof {
            out.eof = true;
            return out;
        }
        while let Some(event) = self.wire.try_event() {
            match event {
                SshEvent::Data(chunk) => {
                    out.bytes += chunk.len();
                    self.dcs.feed(&chunk);
                    self.processor.advance(&mut self.term, &chunk);
                }
                SshEvent::ExitStatus(_) => {}
                SshEvent::Eof => {
                    self.eof = true;
                    out.eof = true;
                    break;
                }
            }
        }
        if let Some(deadline) = self.processor.sync_timeout().sync_timeout() {
            if deadline <= Instant::now() {
                self.processor.stop_sync(&mut self.term);
            }
        }
        out
    }

    /// Send bytes to the remote side.
    ///
    /// The same swallow the PTY session has: while the tap says control
    /// mode is open, this wire belongs to a gateway and every byte typed,
    /// pasted or reported at it is swallowed rather than spliced.
    pub fn write(&mut self, bytes: &[u8]) {
        if self.dcs.tap().in_control_mode() {
            return;
        }
        self.wire.send(bytes);
    }

    /// Apply a new geometry to both halves: the grid first, so a redraw
    /// racing the remote response finds a consistent grid, then the far
    /// side through `window_change`, which is where its `SIGWINCH` comes
    /// from.
    pub fn resize(&mut self, size: TermSize) {
        if size == self.size {
            return;
        }
        self.size = size;
        self.term.resize(size);
        let (width, height) = size.pixel_size();
        self.wire
            .window_change(size.cols() as u16, size.rows() as u16, width, height);
    }

    /// When the pending synchronized update expires, if one is pending.
    pub fn sync_deadline(&self) -> Option<Instant> {
        self.processor.sync_timeout().sync_timeout()
    }

    /// Whole writes the wire refused. Real here, unlike a pane's zero:
    /// this session owns its writer and the writer has a budget.
    pub fn sheds(&self) -> u64 {
        self.wire.sheds()
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

    pub fn tap(&self) -> &T {
        self.dcs.tap()
    }

    pub fn tap_mut(&mut self) -> &mut T {
        self.dcs.tap_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dcs::NoopTap;
    use crate::viewport_text;
    use rio_vt::crosswords::grid::Dimensions;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    /// A wire whose far side is a script: events queue in, writes and
    /// resizes are recorded for the assertions.
    #[derive(Clone, Default)]
    struct FakeWire {
        events: Arc<Mutex<VecDeque<SshEvent>>>,
        sent: Arc<Mutex<Vec<u8>>>,
        resizes: Arc<Mutex<Vec<(u16, u16)>>>,
    }

    impl SshWire for FakeWire {
        fn try_event(&mut self) -> Option<SshEvent> {
            self.events.lock().unwrap().pop_front()
        }
        fn send(&mut self, bytes: &[u8]) {
            self.sent.lock().unwrap().extend_from_slice(bytes);
        }
        fn window_change(&mut self, cols: u16, rows: u16, _pix_w: u16, _pix_h: u16) {
            self.resizes.lock().unwrap().push((cols, rows));
        }
        fn sheds(&self) -> u64 {
            0
        }
    }

    fn size() -> TermSize {
        TermSize::new(20, 5, 9, 18)
    }

    #[test]
    fn wire_bytes_land_on_the_grid_and_writes_reach_the_wire() {
        let wire = FakeWire::default();
        wire.events
            .lock()
            .unwrap()
            .push_back(SshEvent::Data(b"hello \x1b[1mworld\x1b[0m".to_vec()));
        let mut s = SshChannel::new(size(), 100, NoopTap::default(), Box::new(wire.clone()));

        let pumped = s.pump();
        assert_eq!(pumped.bytes, 19);
        assert!(!pumped.eof);
        assert!(viewport_text(s.term())[0].starts_with("hello world"));

        s.write(b"ls\r");
        assert_eq!(*wire.sent.lock().unwrap(), b"ls\r");
    }

    #[test]
    fn a_resize_reflows_the_grid_and_tells_the_far_side() {
        let wire = FakeWire::default();
        let mut s = SshChannel::new(size(), 100, NoopTap::default(), Box::new(wire.clone()));
        s.resize(TermSize::new(40, 10, 9, 18));
        assert_eq!(s.term().grid.columns(), 40);
        assert_eq!(*wire.resizes.lock().unwrap(), vec![(40, 10)]);
        // The same geometry again is not a second far-side call.
        s.resize(TermSize::new(40, 10, 9, 18));
        assert_eq!(wire.resizes.lock().unwrap().len(), 1);
    }

    #[test]
    fn eof_ends_the_session_and_stays_ended() {
        let wire = FakeWire::default();
        {
            let mut q = wire.events.lock().unwrap();
            q.push_back(SshEvent::Data(b"bye".to_vec()));
            q.push_back(SshEvent::ExitStatus(1));
            q.push_back(SshEvent::Eof);
        }
        let mut s = SshChannel::new(size(), 100, NoopTap::default(), Box::new(wire.clone()));
        let pumped = s.pump();
        assert!(pumped.eof);
        assert_eq!(pumped.bytes, 3);
        assert!(s.pump().eof, "a dead wire reports eof on every pump");
        assert_eq!(s.pump().bytes, 0);
    }
}
