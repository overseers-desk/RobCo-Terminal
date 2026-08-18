//! The tmux control-mode protocol, and nothing else.
//!
//! `tmux -CC` is a line protocol: the client writes tmux commands on its
//! standard input, tmux writes back reply blocks and notifications, and the
//! whole conversation travels inside one DCS string (`ESC P 1000 p` ... `ESC \`)
//! on the gateway terminal's own output. This crate turns typed
//! [`Command`]s into the lines that go out and the bytes that come back into
//! typed [`Event`]s. It does not decide what any of it means.
//!
//! # The seam D1b consumes
//!
//! One type, [`Codec`], holds both directions, because they are one
//! conversation: tmux answers a client's commands in the order it received
//! them, so the queue of commands sent is what names the reply that arrives.
//!
//! ```text
//!   host                        Codec                        tmux
//!   ----                        -----                        ----
//!   codec.send(&Command::...) --> wire line, CommandId  --->  stdin
//!   write(sent.wire)
//!                                                             stdout
//!   codec.feed(bytes)         <-- Vec<Event>            <---  (DCS body)
//! ```
//!
//! What crosses the seam, and what deliberately does not:
//!
//! * **Out**: [`Codec::send`] takes a [`Command`] and returns a [`Sent`] --
//!   the exact line to write (newline included) and a [`CommandId`]. The host
//!   writes the bytes; the codec neither owns nor knows about the transport.
//! * **In**: [`Codec::feed`] takes whatever bytes arrived, in whatever sized
//!   chunks the transport gave them, and returns the [`Event`]s they
//!   completed. Partial lines and half-arrived blocks are held.
//! * **Pairing**: a reply block comes back as [`Event::Reply`] carrying the
//!   [`CommandId`] of the command it answers. The host keeps its own map from
//!   id to intent. The codec has no idea that `list-panes` lists panes; it
//!   hands over the body's lines as bytes.
//! * **Not here**: the DCS envelope (the gateway's VT parser strips it --
//!   `term::dcs`), the PTY, the session and window model, the pane-to-channel
//!   routing, the bootstrap sequence, the resize debounce, the capture-and-
//!   cursor gate. Those are D1b's, and each of them is a policy. This crate
//!   holds no policy at all: it has no timers, no state beyond the half-read
//!   line and the pending queue, and no opinion about which commands to send.
//!
//! Bodies and payloads are `Vec<u8>`, not `String`, and that is load-bearing:
//! tmux escapes only the C0 bytes and the backslash in a `%output` payload
//! (see [`escape`]), so a payload carries raw bytes 0x7f..=0xff and is not
//! valid UTF-8 in general. Names and other text fields, which tmux does not
//! escape either, are decoded lossily and say so at their definition.
//!
//! # Where the grammar comes from
//!
//! Two sources, and where they disagree the observed bytes win:
//!
//! * `man tmux`, CONTROL MODE, for the vocabulary of notifications.
//! * tmux 3.5a itself, recorded through a PTY into
//!   `tests/transcripts/`. Every divergence found is recorded at the code that
//!   handles it: the two escaping schemes and which fields take which
//!   ([`escape`]), the `%begin` flags field ([`Block::solicited`]),
//!   `%window-close` never appearing ([`Notification::WindowClose`]),
//!   `%session-renamed` carrying a session id ([`Notification::SessionRenamed`]),
//!   `%message` arriving inside a block ([`Block`]), and `#` starting a comment
//!   in the command lexer ([`command`]).

pub mod codec;
pub mod command;
pub mod escape;
pub mod event;
pub mod ids;

pub use codec::{Codec, CommandId, Sent};
pub use command::Command;
pub use event::{Block, Event, Notification};
pub use ids::{PaneId, SessionId, WindowId};
