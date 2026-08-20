//! A terminal session: a PTY, the VT state, and the read loop joining them.
//!
//! The read loop is hand-rolled rather than rio-vt's `Machine`, because
//! `Machine` owns the bytes and hands them only to
//! `Crosswords`, which cannot see DCS. Owning the loop is the price of
//! the tmux tap, and it buys something else worth having: the loop is
//! synchronous and driven by whoever calls [`Session::pump`], so the
//! whole session runs headless in a test with no threads and no window.
//!
//! The PTY master is opened non-blocking by `teletypewriter`, so `pump`
//! drains what is there and returns; it never waits on the child.

use std::io::{Read, Write};
use std::time::Instant;

use rio_vt::ansi::CursorShape;
use rio_vt::crosswords::grid::Dimensions;
use rio_vt::crosswords::Crosswords;
use rio_vt::event::{VoidListener, WindowId};
use rio_vt::performer::handler::Processor;
use rio_vt::teletypewriter::{create_pty_with_spawn, ProcessReadWrite, Pty, WinsizeBuilder};

use crate::dcs::{DcsParser, DcsTap};
use crate::size::TermSize;

/// The VT state machine and grid in one type, as rio-vt models it.
pub type Term = Crosswords<VoidListener>;

/// Read buffer size. Matches the order of magnitude a PTY hands over per
/// wakeup; larger buffers stop helping once the kernel's own buffer is
/// the limit.
const READ_BUF: usize = 65_536;

/// How much unwritten input may wait for a child that is not reading before
/// this session starts refusing more.
///
/// The queue is there to hold the tail of one write the master would not take
/// whole (see [`Session::write`]), and that is all it is there for. A child
/// that has stopped reading its tty -- stopped, wedged, or simply busy for a
/// long time -- makes every flush a no-op, and every keystroke, paste and
/// terminal report aimed at it then accumulates with nothing to drain it.
///
/// The shed is on the *newest* write and never on the queue, and a write is
/// always taken whole or not at all: splitting one would put a hole in the
/// middle of a paste, which is the exact failure the queue was built to
/// prevent. So the ceiling is this cap plus one write, and one write is
/// bounded by the paste the user actually made.
pub const INPUT_CAP: usize = 4 << 20;

/// How a session is started.
#[derive(Clone, Debug)]
pub struct SessionConfig {
    /// Program to run. `None` means the user's login shell, which is the
    /// production default; tests name `/bin/sh` for determinism.
    pub program: Option<String>,
    pub args: Vec<String>,
    pub working_directory: Option<String>,
    /// Applied on top of the inherited environment.
    pub env: Vec<(String, String)>,
    /// Scrollback lines retained above the screen.
    pub scrollback: usize,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            program: None,
            args: Vec::new(),
            working_directory: None,
            env: vec![("TERM".to_string(), "xterm-256color".to_string())],
            scrollback: 10_000,
        }
    }
}

/// What one `pump` did, so a caller can decide whether to redraw.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pumped {
    /// Bytes read from the child this call.
    pub bytes: usize,
    /// The child's end closed: the session is over.
    pub eof: bool,
}

impl Pumped {
    pub fn is_idle(&self) -> bool {
        self.bytes == 0 && !self.eof
    }
}

/// A live terminal session.
pub struct Session<T: DcsTap> {
    term: Term,
    pty: Pty,
    processor: Processor,
    dcs: DcsParser<T>,
    size: TermSize,
    buf: Box<[u8; READ_BUF]>,
    /// Bytes on their way to the child that the master would not take yet.
    /// See [`Session::write`].
    input: Vec<u8>,
    /// How many writes have been refused because the queue was full.
    /// See [`Session::sheds`].
    sheds: u64,
    eof: bool,
}

impl<T: DcsTap> Session<T> {
    /// Spawn the child and build the session around it.
    pub fn spawn(config: &SessionConfig, size: TermSize, tap: T) -> std::io::Result<Self> {
        let (px_w, px_h) = size.pixel_size();
        let pty = create_pty_with_spawn(
            config.program.as_deref(),
            config.args.clone(),
            &config.working_directory,
            Some(config.env.clone()),
            size.cols() as u16,
            size.rows() as u16,
            px_w,
            px_h,
        )?;

        let term = Crosswords::new(
            size,
            CursorShape::Block,
            VoidListener {},
            WindowId::from(0u64),
            0,
            config.scrollback,
        );

        Ok(Self {
            term,
            pty,
            processor: Processor::default(),
            dcs: DcsParser::new(tap),
            size,
            buf: Box::new([0u8; READ_BUF]),
            input: Vec::new(),
            sheds: 0,
            eof: false,
        })
    }

    /// Drain whatever the child has written and apply it.
    ///
    /// Every chunk goes to two consumers: the DCS tap and the grid. See
    /// `dcs.rs` for why feeding both is correct rather than double work.
    pub fn pump(&mut self) -> Pumped {
        let mut out = Pumped::default();
        if self.eof {
            out.eof = true;
            return out;
        }

        // The other direction first: whatever a paste left queued when the
        // tty's buffer filled ([`Session::write`]). A pump is the only thing
        // that comes back to it.
        self.drop_input_in_control_mode();
        if let Err(e) = self.flush_input() {
            log::warn!("could not write to the pty: {e}");
        }

        loop {
            match self.pty.read(&mut self.buf[..]) {
                // A zero-length read means no slave fd is open. Usually
                // that is the child having closed its end, but not
                // always: see `child_gone`.
                Ok(0) => {
                    if self.child_gone() {
                        self.eof = true;
                        out.eof = true;
                    }
                    break;
                }
                Ok(n) => {
                    out.bytes += n;
                    // Copy out so the two consumers can each take
                    // `&mut self`-adjacent borrows without fighting the
                    // borrow checker over `self.buf`.
                    let chunk: Vec<u8> = self.buf[..n].to_vec();
                    self.dcs.feed(&chunk);
                    self.processor.advance(&mut self.term, &chunk);
                    // A short read means the buffer was not filled, so
                    // there is nothing queued behind it.
                    if n < READ_BUF {
                        break;
                    }
                }
                Err(e) => {
                    match e.kind() {
                        // Nothing pending: the normal exit from this loop.
                        std::io::ErrorKind::WouldBlock => {}
                        std::io::ErrorKind::Interrupted => continue,
                        // Everything else is effectively EIO: no slave
                        // fd is open. Only the child can tell us whether
                        // that is fatal.
                        _ => {
                            if self.child_gone() {
                                self.eof = true;
                                out.eof = true;
                            }
                        }
                    }
                    break;
                }
            }
        }

        // And again on the way out, because the envelope may have opened in
        // the loop just above: this is the one call that stands between the
        // control mode starting and the gateway's first write, which the host
        // makes later in this same pump.
        self.drop_input_in_control_mode();

        self.expire_sync();
        out
    }

    /// Give up any input still queued for a shell that has become a tmux
    /// control-mode wire.
    ///
    /// The gateway writes that fd through its own `dup` while this queue
    /// writes it here, and the module doc above [`Session::control_mode_writer`]
    /// rests on nothing but the gateway writing it while control mode is
    /// active. A tail left over from before the envelope opened -- the rest of
    /// a paste the tty's buffer refused, which is exactly the case this queue
    /// exists for -- would otherwise flush underneath the gateway and splice
    /// the user's half-line into the middle of a command line.
    ///
    /// Dropped rather than held: an anchor swallows every byte typed, pasted
    /// or reported at it (`app::window::TerminalSurface::write`), and these
    /// are bytes aimed at a shell that is not there any more. Holding them
    /// would only deliver them at a detach, to a shell that never asked.
    fn drop_input_in_control_mode(&mut self) {
        if self.input.is_empty() || !self.dcs.tap().in_control_mode() {
            return;
        }
        log::warn!(
            "tmux control mode opened with {} byte(s) still queued for the shell; \
             the anchor swallows them rather than splicing them into the gateway's wire",
            self.input.len()
        );
        self.input.clear();
    }

    /// Has the child actually exited?
    ///
    /// Reading a PTY master reports EIO (and sometimes a zero-length
    /// read) whenever no process holds the slave open. That is the right
    /// signal for "the shell exited", but it is *also* true for a moment
    /// at startup: `openpty` returns, the parent closes its copy of the
    /// slave fd, and until the forked child reaches `exec` and opens it,
    /// nobody holds it. A first `pump` landing in that window would
    /// otherwise mark the session finished before the shell ever ran,
    /// a race that only shows up on a loaded machine, where the child
    /// takes longer to get going.
    ///
    /// `waitpid(WNOHANG)` is the discriminator: still running means the
    /// error was transient and the next pump will find the child.
    fn child_gone(&self) -> bool {
        match self.pty.waitpid() {
            // Alive: whatever we just saw was the startup window.
            Ok(None) => false,
            Ok(Some(_)) => true,
            // We cannot ask, so we cannot claim it is still running.
            Err(_) => true,
        }
    }

    /// Flush a synchronized update whose deadline has passed.
    ///
    /// A `BSU` (`ESC P = 1 s`) tells the terminal to hold rendering until
    /// `ESU`. If the application dies mid-update the screen would freeze
    /// forever, so the sequence carries a timeout. `Processor` only
    /// enforces it lazily, on the next `advance`; an embedder that stops
    /// receiving bytes never reaches that call. Honouring it here, on
    /// every pump, is the price of owning the read loop: nothing else calls
    /// `sync_timeout()` for us.
    fn expire_sync(&mut self) {
        if let Some(deadline) = self.processor.sync_timeout().sync_timeout() {
            if deadline <= Instant::now() {
                self.processor.stop_sync(&mut self.term);
            }
        }
    }

    /// When the pending synchronized update expires, if one is pending.
    ///
    /// The app's event loop uses this as a wakeup deadline: with no PTY
    /// traffic it would otherwise sleep past the timeout and leave the
    /// screen held.
    pub fn sync_deadline(&self) -> Option<Instant> {
        self.processor.sync_timeout().sync_timeout()
    }

    /// Send bytes to the child (keystrokes, paste, responses).
    ///
    /// Queued, then pushed as far as the master will take them. The master is
    /// `O_NONBLOCK` (see this module's opening note), so a write bigger than
    /// the free space in the tty's input buffer takes a prefix and refuses the
    /// rest, and a paste is exactly that write. `write_all` would report the
    /// refusal as an error *after* the prefix had gone, and the tail would be
    /// gone with it: a paste arriving at the shell with a hole in the middle.
    /// So the tail waits here and [`Session::pump`] pushes it again, on every
    /// pump, until the child has read enough to make room.
    ///
    /// A queue past [`INPUT_CAP`] means the child has stopped reading, and
    /// this refuses rather than grows: see that constant for which end sheds
    /// and why a write is taken whole or not at all. A refusal is not an
    /// error -- nothing went wrong on the wire, and there is nothing for the
    /// caller to retry -- so it is logged and swallowed, the same shape the
    /// gateway's own shed has.
    ///
    /// The error this can still answer is a real one (the master is gone)
    /// and the queue is dropped with it, because the child that would have
    /// read it is what just died.
    pub fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        // While control mode is active this fd is the gateway's, and an anchor
        // swallows every byte typed, pasted *or reported* at it. The host
        // swallows what it knows about (`app::window::TerminalSurface::write`,
        // gated on the row already being an anchor); this is the same rule at
        // the one place that can see the envelope rather than the row, which
        // is what also catches a report the grid generates from inside the
        // pump. See [`Self::drop_input_in_control_mode`].
        if self.dcs.tap().in_control_mode() {
            return Ok(());
        }
        if self.input.len() >= INPUT_CAP {
            self.sheds += 1;
            log::warn!(
                "the child is not reading its tty and {} byte(s) are queued for it; \
                 dropping {} more rather than growing the queue",
                self.input.len(),
                bytes.len()
            );
            return Ok(());
        }
        self.input.extend_from_slice(bytes);
        self.flush_input()
    }

    /// How many bytes are queued for a child that has not taken them yet.
    ///
    /// The instrument behind [`INPUT_CAP`]: a test measures the high-water
    /// mark through this, and it is the reading a host would want if it ever
    /// grows a way to tell the user their shell has stopped listening.
    pub fn queued_input(&self) -> usize {
        self.input.len()
    }

    /// How many writes this session has thrown away because the queue was at
    /// [`INPUT_CAP`].
    ///
    /// The log line at the shed says what was lost, and a log is not on the
    /// glass: this counter is how a host tells the user instead. It only ever
    /// grows, so a host that remembers the last reading learns "something was
    /// dropped since you last looked" by comparing, without this type having to
    /// know what a host would do about it (`app::window` raises a badge).
    pub fn sheds(&self) -> u64 {
        self.sheds
    }

    /// Push the input queue at the master, keeping what it refuses.
    fn flush_input(&mut self) -> std::io::Result<()> {
        while !self.input.is_empty() {
            match self.pty.write(&self.input) {
                // Nothing taken and nothing wrong: backpressure, wearing a
                // success. Stop rather than spin.
                Ok(0) => break,
                Ok(n) => {
                    self.input.drain(..n);
                }
                Err(e) => match e.kind() {
                    std::io::ErrorKind::Interrupted => continue,
                    // The buffer is full; the child is what empties it.
                    std::io::ErrorKind::WouldBlock => break,
                    _ => {
                        self.input.clear();
                        return Err(e);
                    }
                },
            }
        }
        Ok(())
    }

    /// A second, independent handle onto the PTY master's write side.
    ///
    /// The tmux gateway owns its transport (its commands must go out the
    /// moment it decides to send them, not on the next time the host
    /// happens to hold this session), while this session keeps owning the
    /// read side and the loop. A `dup` of the master fd gives both their
    /// own handle onto one file description.
    ///
    /// Two handles onto one fd is only safe because of what the host does
    /// with them, and not because of anything the fd guarantees. This is a
    /// tty, so there is no `PIPE_BUF` atomicity to lean on: a write here can
    /// take a prefix and refuse the rest, which is why the gateway queues
    /// (`app::tmux::Gateway::flush`) rather than assuming a whole line left.
    /// What keeps the wire clean is that the host is single-threaded (one
    /// pump, one writer running at a time, never two mid-line) and that
    /// while control mode is active nothing writes this fd but the gateway:
    /// the channel holding this session is an anchor, and an anchor swallows
    /// every byte typed, pasted or reported at it
    /// (`app::window::TerminalSurface::write`).
    pub fn control_mode_writer(&mut self) -> std::io::Result<std::fs::File> {
        self.pty.writer().try_clone()
    }

    /// Force both parsers out of a DCS string that will never close.
    ///
    /// When the control client is killed without closing its device
    /// string, no `ST` is ever coming,
    /// and until one arrives the grid's parser swallows everything the
    /// shell prints as DCS body. Feeding a synthetic `ST` to both
    /// consumers returns them to ground; at ground it is a no-op, so
    /// calling it on a healthy session costs nothing.
    pub fn leave_control_mode(&mut self) {
        const ST: &[u8] = b"\x1b\\";
        self.dcs.feed(ST);
        self.processor.advance(&mut self.term, ST);
    }

    /// Apply a new geometry to both halves.
    ///
    /// Both, and in this order. The grid is reflowed first so a redraw
    /// racing the child's response finds a consistent grid; the child is
    /// then told through `TIOCSWINSZ`, which is the only way it learns
    /// its size (and what makes it send `SIGWINCH` to its foreground
    /// job). Skipping either half is the classic half-resized terminal.
    pub fn resize(&mut self, size: TermSize) -> std::io::Result<()> {
        if size == self.size {
            return Ok(());
        }
        self.size = size;
        self.term.resize(size);
        let (width, height) = size.pixel_size();
        self.pty.set_winsize(WinsizeBuilder {
            rows: size.rows() as u16,
            cols: size.cols() as u16,
            width,
            height,
        })
    }

    /// The geometry last applied.
    pub fn size(&self) -> TermSize {
        self.size
    }

    /// Columns as the *grid itself* reports them.
    ///
    /// Deliberately read from `Crosswords` rather than from the cached
    /// `size`, so a caller can tell whether a resize actually reached
    /// the grid instead of only being recorded here.
    pub fn grid_cols(&self) -> usize {
        self.term.grid.columns()
    }

    /// Screen rows as the grid reports them. See [`Self::grid_cols`].
    pub fn grid_rows(&self) -> usize {
        self.term.grid.screen_lines()
    }

    /// Lines retained above the screen.
    pub fn history_size(&self) -> usize {
        self.term.history_size()
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

    /// The tap, mutably. The shipped tap ([`crate::tmux_cc::ControlModeTap`])
    /// hands its edges and its peeled body over through draining `take_`
    /// methods, so reaching it through `&self` is not enough.
    pub fn tap_mut(&mut self) -> &mut T {
        self.dcs.tap_mut()
    }

    /// True once the child's end has closed.
    pub fn is_finished(&self) -> bool {
        self.eof
    }
}
