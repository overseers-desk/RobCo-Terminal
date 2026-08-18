//! The DCS tap: the seam the tmux -CC gateway (`app::tmux`) stands on.
//!
//! The application owns the read loop itself rather than using rio-vt's
//! `Machine`. rio-vt's `Handler` trait has no hook/put/unhook, and
//! the `Performer` that does see DCS is private and discards anything it
//! does not recognise, so a `1000p` control-mode stream is invisible from
//! the `Crosswords` side. The public `Perform` trait *does* see it. The
//! price is that we cannot use rio-vt's `Machine`: the read loop is ours,
//! and it feeds every byte to two consumers, this tap and
//! `Processor::advance`.
//!
//! Feeding both is safe rather than duplicative. The VT parser consumes a
//! DCS block whole and hands `Handler` nothing from its body, so the body
//! never reaches the grid; text after `ESC \` does. The tap sees the body,
//! the grid sees the rest, and neither needs the other to have run.
//!
//! The tap the app installs is [`ControlModeTap`], which replaced
//! [`NoopTap`] as the shipped tap ([`NoopTap`] stays as the test double): it
//! recognises the tmux control-mode envelope, buffers the peeled body for
//! the gateway, and reports the two edges the host acts on: the
//! envelope opening (detection) and closing (`ST`). It still decides
//! nothing: what an attachment *means* is `app::channels`' and the
//! gateway's business.

use rio_vt::performer::parser::{Params, Parser, Perform};

/// What the read loop hands a DCS consumer.
///
/// Deliberately narrower than `Perform`: a tap has no business seeing
/// ordinary printable bytes or CSI sequences, which belong to the grid.
/// [`ControlModeTap`] implements this rather than `Perform`, so the
/// tmux layer never has to care that a VT parser is underneath.
pub trait DcsTap {
    /// A DCS block opened. `params`/`action` identify it: tmux control
    /// mode is params `[1000]` with action `'p'`.
    fn hook(&mut self, params: &[u16], action: char);

    /// One byte of the block's body.
    fn put(&mut self, byte: u8);

    /// The block closed (`ESC \` or `ST`).
    fn unhook(&mut self);

    /// Whether the envelope this tap is inside owns the write side of the
    /// session's PTY, so the session must not push its own queue at it.
    ///
    /// Only tmux control mode answers true. Its wire is shared with a second
    /// writer (`app::tmux::Gateway`, through `Session::writer_handle`) and the
    /// protocol on it is line-oriented, so a queued paste tail flushed
    /// underneath the gateway splices the user's half-line into the middle of
    /// a command. See [`Session::pump`](crate::Session::pump).
    fn owns_the_wire(&self) -> bool {
        false
    }
}

/// The no-op tap: sees everything, remembers nothing.
///
/// It counts, and only so the transcript test can prove the bytes
/// arrive. [`ControlModeTap`] replaced it as the shipped type (the
/// wiring never moved) and this stays as the test double.
#[derive(Default, Debug)]
pub struct NoopTap {
    pub hooks: usize,
    pub body_bytes: usize,
    pub unhooks: usize,
}

impl DcsTap for NoopTap {
    fn hook(&mut self, _params: &[u16], _action: char) {
        self.hooks += 1;
    }
    fn put(&mut self, _byte: u8) {
        self.body_bytes += 1;
    }
    fn unhook(&mut self) {
        self.unhooks += 1;
    }
}

/// The shipped tap: recognises `DCS 1000 p`, tmux's control-mode
/// envelope, and peels its body for the gateway.
///
/// Three things cross this seam, each drained by its own `take_`:
///
/// * **Detection** ([`ControlModeTap::take_detected`]): the envelope
///   opened. The host turns the local channel this session sits on into
///   an attachment (`app::channels::attach_gateway`) and raises a
///   gateway over this tap's body stream.
/// * **The body** ([`ControlModeTap::take_body`]): the bytes between
///   `ESC P 1000 p` and `ST`, exactly what the gateway's codec eats.
///   Buffered here rather than handed to a callback so the tap stays
///   free of any `tmux-cc` type: `term` knows there is an envelope, not
///   what is in it.
/// * **The close** ([`ControlModeTap::take_ended`]): `ST` arrived. With
///   a preceding `%exit` this is the ordinary end of a detach; without
///   one it marks that the gateway program died mid-protocol and the
///   host must collapse the page.
///
/// Any DCS that is not `1000p` is ignored whole: its body neither
/// buffers nor flips the flags, so a sixel or a `DECRQSS` reply cannot
/// impersonate an attachment.
#[derive(Default, Debug)]
pub struct ControlModeTap {
    /// Inside a `1000p` envelope right now.
    active: bool,
    detected: bool,
    ended: bool,
    body: Vec<u8>,
}

/// tmux control mode's DCS params and action: `ESC P 1000 p`.
///
/// Module-level rather than `ControlModeTap`'s own, because two things
/// need the same answer: the tap, deciding whether an envelope is an
/// attachment, and [`DcsParser`], deciding whether to hand the body to
/// the tap verbatim instead of through rio-vt's DCS string state.
const TMUX_PARAMS: &[u16] = &[1000];
const TMUX_ACTION: char = 'p';

impl ControlModeTap {
    /// The envelope opened since the last call. One edge per envelope.
    pub fn take_detected(&mut self) -> bool {
        std::mem::take(&mut self.detected)
    }

    /// The envelope closed (`ST`) since the last call.
    pub fn take_ended(&mut self) -> bool {
        std::mem::take(&mut self.ended)
    }

    /// Drain the peeled body bytes that arrived since the last call.
    pub fn take_body(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.body)
    }

    /// Whether the parser currently stands inside the envelope.
    pub fn in_control_mode(&self) -> bool {
        self.active
    }
}

impl DcsTap for ControlModeTap {
    fn hook(&mut self, params: &[u16], action: char) {
        self.active = params == TMUX_PARAMS && action == TMUX_ACTION;
        if self.active {
            self.detected = true;
            // A new envelope is a new conversation. Whatever a dead one left
            // undrained (a tail nobody pumped before its gateway went, its
            // close edge) must not reach the next gateway as its own.
            self.body.clear();
            self.ended = false;
        }
    }

    fn put(&mut self, byte: u8) {
        if self.active {
            self.body.push(byte);
        }
    }

    fn unhook(&mut self) {
        if self.active {
            self.active = false;
            self.ended = true;
        }
    }

    fn owns_the_wire(&self) -> bool {
        self.active
    }
}

/// Adapts a [`DcsTap`] to rio-vt's `Perform`, discarding everything that
/// is not DCS.
///
/// The empty methods are the point of the type: `Perform` is a wide
/// trait covering the whole VT vocabulary, and the grid is already
/// handling all of it through `Processor`. Only the three DCS callbacks
/// forward.
struct DcsOnly<'a, T: DcsTap> {
    tap: &'a mut T,
    /// Set when the block the parser just opened is tmux's `1000p`.
    /// [`DcsParser`] reads it back to decide whether to take the body
    /// over; nothing else in the file cares.
    tmux_hooked: bool,
}

impl<T: DcsTap> Perform for DcsOnly<'_, T> {
    fn hook(&mut self, params: &Params, _intermediates: &[u8], _ignore: bool, action: char) {
        let flat: Vec<u16> = params.iter().flat_map(|p| p.iter().copied()).collect();
        self.tmux_hooked = flat == TMUX_PARAMS && action == TMUX_ACTION;
        self.tap.hook(&flat, action);
    }

    fn put(&mut self, byte: u8) {
        self.tap.put(byte);
    }

    fn unhook(&mut self) {
        self.tap.unhook();
    }
}

/// Where the tap's byte scanner stands, which is not always where its VT
/// parser stands.
///
/// The scanner exists because a tmux control-mode body is *not* a VT DCS
/// string. rio-vt implements the string state ECMA-48 specifies, which
/// drops `0x7F`, drops `0x80..=0xFF`, and treats a raw `0x9C` as the
/// terminator; tmux emits all three inside `%output` as ordinary payload
/// (it escapes only `0x00..=0x1F` and the backslash, as octal), and a
/// `capture-pane -e` reply carries raw `ESC [ ... m` by design. Under the
/// standard state machine the high half of a `%output` line vanishes, a
/// stray `0x9C` closes the attachment early, and the first colour
/// sequence in a captured page unhooks it.
///
/// So for that one envelope the body bypasses the VT parser and reaches
/// the tap verbatim, terminated only by `ESC \` -- which is the rule the
/// transcript fixtures are peeled by (`tmux-cc/tests/support`'s
/// `control_stream`) and the rule tmux actually emits under. Every other
/// DCS keeps rio-vt's standard handling: the tap discards those bodies
/// anyway, and a sixel that ends on an 8-bit `ST` must still be able to
/// end.
#[derive(Debug)]
enum Mode {
    /// Outside any envelope. Bytes go to the VT parser in bulk, up to
    /// the next `ESC` -- the only byte that can open a DCS here, since
    /// this parser dispatches a bare C1 `0x90` as `execute` rather than
    /// as an introducer.
    Scan,
    /// Inside a DCS introducer. Fed one byte at a time so that the
    /// takeover can happen on exactly the byte the hook fires on, before
    /// the parser has swallowed any of the body. Persists across `feed`
    /// calls, because a read boundary can fall inside `ESC P 1000 p`.
    Intro { after_esc: bool },
    /// Inside a tmux envelope, where the VT parser is not involved.
    /// `esc` holds a pending `ESC` whose next byte decides whether it was
    /// the terminator or payload.
    Envelope { esc: bool },
}

/// The tap's own VT parser, running alongside the grid's.
///
/// Its own parser and not a shared one: `Processor` owns its parser
/// privately, and a DCS block's state spans reads, so the tap needs a
/// state machine that persists across `feed` calls the same way.
pub struct DcsParser<T: DcsTap> {
    parser: Parser,
    tap: T,
    mode: Mode,
}

impl<T: DcsTap> DcsParser<T> {
    pub fn new(tap: T) -> Self {
        Self {
            parser: Parser::default(),
            tap,
            mode: Mode::Scan,
        }
    }

    /// Hand bytes to the VT parser, reporting whether they opened tmux's
    /// envelope.
    fn vt(&mut self, bytes: &[u8]) -> bool {
        let mut perform = DcsOnly {
            tap: &mut self.tap,
            tmux_hooked: false,
        };
        self.parser.advance(&mut perform, bytes);
        perform.tmux_hooked
    }

    /// Feed the same bytes the grid is getting.
    pub fn feed(&mut self, bytes: &[u8]) {
        let mut i = 0;
        while i < bytes.len() {
            i += match self.mode {
                Mode::Scan => self.scan(&bytes[i..]),
                Mode::Intro { after_esc } => self.intro(bytes[i], after_esc),
                Mode::Envelope { esc } => self.envelope(bytes[i], esc),
            };
        }
    }

    /// Bulk-feed up to the next `ESC`, then arm the introducer matcher.
    fn scan(&mut self, rest: &[u8]) -> usize {
        let n = rest.iter().position(|&b| b == 0x1B).unwrap_or(rest.len());
        if n > 0 {
            self.vt(&rest[..n]);
        }
        if n == rest.len() {
            return n;
        }
        self.vt(&rest[n..n + 1]);
        self.mode = Mode::Intro { after_esc: true };
        n + 1
    }

    /// Track the DCS introducer grammar (`ESC P`, parameter and
    /// intermediate bytes, then a final in `0x40..=0x7E`) alongside the
    /// parser, so the byte that fires `hook` can be recognised as it is
    /// fed rather than after the body is gone.
    fn intro(&mut self, byte: u8, after_esc: bool) -> usize {
        let tmux = self.vt(&[byte]);
        self.mode = if after_esc {
            match byte {
                b'P' => Mode::Intro { after_esc: false },
                // `ESC ESC`: the second one is the introducer's.
                0x1B => Mode::Intro { after_esc: true },
                _ => Mode::Scan,
            }
        } else {
            match byte {
                // Parameters and intermediates: still in the introducer.
                0x20..=0x3F => Mode::Intro { after_esc: false },
                // The final byte. `hook` has just fired.
                0x40..=0x7E if tmux => Mode::Envelope { esc: false },
                _ => Mode::Scan,
            }
        };
        1
    }

    /// One byte of a tmux body, terminated only by `ESC \`.
    fn envelope(&mut self, byte: u8, esc: bool) -> usize {
        if esc {
            if byte == b'\\' {
                // Let the VT parser close the block it opened, so it
                // returns to ground and the tap hears its `unhook`
                // through the same path every other DCS uses.
                self.vt(b"\x1b\\");
                self.mode = Mode::Scan;
                return 1;
            }
            // Not the terminator, so the `ESC` was payload: a colour
            // sequence in a `capture-pane -e` reply, most often.
            self.tap.put(0x1B);
        }
        if byte == 0x1B {
            self.mode = Mode::Envelope { esc: true };
        } else {
            self.tap.put(byte);
            self.mode = Mode::Envelope { esc: false };
        }
        1
    }

    pub fn tap(&self) -> &T {
        &self.tap
    }

    pub fn tap_mut(&mut self) -> &mut T {
        &mut self.tap
    }
}

impl<T: DcsTap + Default> Default for DcsParser<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tap that reconstructs the body, so the test can assert on
    /// content rather than only on counts.
    #[derive(Default)]
    struct Recording {
        opened: Option<(Vec<u16>, char)>,
        body: Vec<u8>,
        closed: bool,
    }

    impl DcsTap for Recording {
        fn hook(&mut self, params: &[u16], action: char) {
            self.opened = Some((params.to_vec(), action));
        }
        fn put(&mut self, byte: u8) {
            self.body.push(byte);
        }
        fn unhook(&mut self) {
            self.closed = true;
        }
    }

    const TMUX_DCS: &[u8] = b"\x1bP1000p%begin 1 1 1\r\n%output %1 hi\r\n%end 1 1 1\x1b\\AFTER-DCS";

    #[test]
    fn a_tmux_control_block_reaches_the_tap_whole() {
        let mut p = DcsParser::new(Recording::default());
        p.feed(TMUX_DCS);
        let t = p.tap();
        assert_eq!(t.opened, Some((vec![1000], 'p')));
        assert!(t.closed);
        assert_eq!(
            String::from_utf8_lossy(&t.body),
            "%begin 1 1 1\r\n%output %1 hi\r\n%end 1 1 1"
        );
    }

    #[test]
    fn a_block_split_across_reads_is_still_one_block() {
        // The read loop hands over whatever the PTY gave it, which has
        // no relationship to escape-sequence boundaries. Splitting mid
        // body must not close or reopen the block.
        let mut p = DcsParser::new(Recording::default());
        let (a, b) = TMUX_DCS.split_at(20);
        p.feed(a);
        assert!(p.tap().opened.is_some());
        assert!(!p.tap().closed, "block closed early at a read boundary");
        p.feed(b);
        let t = p.tap();
        assert!(t.closed);
        assert_eq!(
            String::from_utf8_lossy(&t.body),
            "%begin 1 1 1\r\n%output %1 hi\r\n%end 1 1 1"
        );
    }

    #[test]
    fn ordinary_text_never_reaches_the_tap() {
        let mut p = DcsParser::new(Recording::default());
        p.feed(b"plain text\r\n\x1b[31mred\x1b[0m");
        let t = p.tap();
        assert!(t.opened.is_none());
        assert!(t.body.is_empty());
    }

    #[test]
    fn the_control_tap_detects_peels_and_reports_the_close() {
        let mut p = DcsParser::new(ControlModeTap::default());
        p.feed(TMUX_DCS);
        let tap = p.tap_mut();
        assert!(tap.take_detected());
        assert!(!tap.take_detected(), "one edge per envelope");
        assert_eq!(
            String::from_utf8_lossy(&tap.take_body()),
            "%begin 1 1 1\r\n%output %1 hi\r\n%end 1 1 1"
        );
        assert!(tap.take_body().is_empty(), "a drain drains");
        assert!(tap.take_ended());
        assert!(!tap.in_control_mode());
    }

    #[test]
    fn a_foreign_dcs_is_no_attachment() {
        // A DECRQSS reply: DCS 1 $ r ... ST. Wrong params, wrong action.
        let mut p = DcsParser::new(ControlModeTap::default());
        p.feed(b"\x1bP1$r0m\x1b\\");
        let tap = p.tap_mut();
        assert!(!tap.take_detected());
        assert!(tap.take_body().is_empty());
        assert!(!tap.take_ended(), "a foreign close is not tmux's close");
    }

    /// A recorded tmux 3.5a attach whose panes print every byte there is.
    ///
    /// tmux escapes `0x00..=0x1F` and the backslash as octal inside
    /// `%output` and nothing else, so `0x7F` and the whole `0x80..=0xFF`
    /// run -- `0x9C` among them -- ride the envelope raw. Under rio-vt's
    /// DCS string state those are, in order, dropped, dropped, and read
    /// as the terminator.
    const OCTAL_FIXTURE: &[u8] =
        include_bytes!("../../tmux-cc/tests/transcripts/04-output-octal.txt");

    fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack.windows(needle.len()).position(|w| w == needle)
    }

    /// Feed `wire` at several chunk sizes, returning the body each time.
    ///
    /// The chunk sizes matter as much as the bytes: the read loop hands
    /// over whatever the PTY gave it, so a boundary can fall between an
    /// `ESC` and the byte that decides whether it was a terminator.
    fn peel_at_every_boundary(wire: &[u8]) -> Vec<Vec<u8>> {
        [1usize, 3, 7, 64, wire.len()]
            .into_iter()
            .map(|chunk| {
                let mut p = DcsParser::new(ControlModeTap::default());
                let mut body = Vec::new();
                for part in wire.chunks(chunk) {
                    p.feed(part);
                    body.extend(p.tap_mut().take_body());
                }
                assert!(
                    p.tap_mut().take_ended(),
                    "chunked by {chunk}: the envelope never closed"
                );
                body
            })
            .collect()
    }

    #[test]
    fn every_byte_of_a_recorded_envelope_reaches_the_tap_unchanged() {
        // The canonical peel: from after `ESC P 1000 p` to the `ESC \`,
        // which is how `tmux-cc`'s transcript support reads the same
        // fixture and how tmux emits it.
        let start = find(OCTAL_FIXTURE, b"\x1bP1000p").expect("an envelope") + 7;
        let end = start + find(&OCTAL_FIXTURE[start..], b"\x1b\\").expect("an ST");
        let expected = &OCTAL_FIXTURE[start..end];

        // Not a vacuous assertion: these are the three bytes the VT DCS
        // state would have eaten, and they are really in the fixture.
        assert!(expected.contains(&0x7F), "no raw 0x7F in the fixture");
        assert!(expected.contains(&0x9C), "no raw 0x9C in the fixture");
        assert!(expected.contains(&0xFF), "no raw 0xFF in the fixture");

        for (body, chunk) in peel_at_every_boundary(OCTAL_FIXTURE).into_iter().zip([
            1usize,
            3,
            7,
            64,
            OCTAL_FIXTURE.len(),
        ]) {
            assert_eq!(
                body.len(),
                expected.len(),
                "chunked by {chunk}: {} body bytes for {} on the wire",
                body.len(),
                expected.len()
            );
            assert_eq!(body, expected, "chunked by {chunk}: a byte changed");
        }
    }

    #[test]
    fn a_capture_reply_keeps_the_escape_sequences_it_is_made_of() {
        // `capture-pane -e` answers with the page's own escape sequences,
        // so the first `ESC [ 31 m` is payload. Only `ESC \` ends the
        // envelope, and a coloured prompt is full of the other kind.
        let body: &[u8] =
            b"%begin 1 1 1\r\n\x1b[31mred\x1b[0m $ \x1b[1;32mprompt\x1b[m\r\n%end 1 1 1";
        let mut wire = Vec::from(&b"\x1bP1000p"[..]);
        wire.extend_from_slice(body);
        wire.extend_from_slice(b"\x1b\\");

        for (peeled, chunk) in
            peel_at_every_boundary(&wire)
                .into_iter()
                .zip([1usize, 3, 7, 64, wire.len()])
        {
            assert_eq!(peeled, body, "chunked by {chunk}: the reply was cut");
        }

        // And the tap is back on the ground afterwards, ready for the
        // text that follows the envelope.
        let mut p = DcsParser::new(ControlModeTap::default());
        p.feed(&wire);
        assert!(!p.tap().in_control_mode());
    }

    #[test]
    fn a_foreign_dcs_still_ends_the_way_its_own_standard_says() {
        // The takeover is the tmux envelope's alone. A sixel that closes
        // on an 8-bit `ST` must still close, or the tap would swallow
        // every attachment that came after it.
        let mut p = DcsParser::new(ControlModeTap::default());
        p.feed(b"\x1bPq#0;2;0;0;0\x9c");
        assert!(!p.tap().in_control_mode());

        // A real attachment right behind it is still seen.
        p.feed(TMUX_DCS);
        let tap = p.tap_mut();
        assert!(tap.take_detected(), "the sixel ate the attachment");
        assert_eq!(
            String::from_utf8_lossy(&tap.take_body()),
            "%begin 1 1 1\r\n%output %1 hi\r\n%end 1 1 1"
        );
    }

    #[test]
    fn the_body_survives_read_boundaries_and_drains_between_them() {
        let mut p = DcsParser::new(ControlModeTap::default());
        let (a, b) = TMUX_DCS.split_at(20);
        p.feed(a);
        assert!(p.tap_mut().take_detected());
        let first = p.tap_mut().take_body();
        p.feed(b);
        let second = p.tap_mut().take_body();
        let whole = [first, second].concat();
        assert_eq!(
            String::from_utf8_lossy(&whole),
            "%begin 1 1 1\r\n%output %1 hi\r\n%end 1 1 1"
        );
        assert!(p.tap_mut().take_ended());
    }
}
