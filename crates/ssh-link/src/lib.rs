//! The SSH transport: one thread per connection, bytes to the event loop.
//!
//! The terminal is synchronous and polls; SSH wants an async runtime. The
//! boundary is a thread: each connection owns a current-thread tokio
//! runtime on an OS thread of its own, and the event loop sees only the
//! endpoints in [`ChannelHandle`], drained on its pump the way every other
//! session is. Nothing tokio crosses this crate's API except the channel
//! primitives inside the handle, and nothing here knows a grid, a window
//! or a key exists.
//!
//! Host-key trust is the caller's, through [`HostPolicy`]: this crate
//! carries the verdict to the wire but never decides one, because trust
//! policy is security-critical code that belongs beside its documentation
//! and its fixtures, not inside a transport.
//!
//! Everything a connection needs a human for -- a trust decision, a key's
//! passphrase, a password, a server's own challenge -- it gets through
//! [`Asker`], and it blocks its own thread until the answer arrives. This
//! crate never learns where the answer was typed, and does not need to:
//! the caller holds the [`AskDesk`], paints the question wherever it paints
//! everything else, and hands the answer back. That is what keeps the
//! whole prompted half of authentication out of a transport, exactly as
//! trust is. See [`ask`] for why the channel underneath is std's and not
//! tokio's, and for the ordering that keeps a question behind the notice
//! explaining it.
//!
//! The auth sequence in `thread` is `ssh`'s: public keys first (the named
//! keys, the agent, the default files), then keyboard-interactive, then
//! password. Which of them are tried at all is the server's to say. The
//! opening `none` probe brings back the method list, every rejection
//! refreshes it, and a method the server does not offer is never
//! attempted, so a key-only server produces no questions at all.

pub mod ask;
mod channel;
#[cfg(feature = "test-server")]
pub mod test_server;
mod thread;

pub use ask::{Ask, AskDesk, Answer, Asker, Question};
pub use channel::{ChannelHandle, WireEvent};
/// The one russh everything downstream compiles against: re-exported so the
/// policy implementation and this crate cannot drift apart on a version.
pub use russh;

/// Where and who: everything a connection needs before trust and auth.
#[derive(Debug, Clone)]
pub struct SshTarget {
    pub user: String,
    pub host: String,
    pub port: u16,
    /// What `TERM` the remote pty advertises: the same name the local
    /// sessions run under, because the remote programs face the same glass.
    pub term: String,
    /// Initial pty geometry: cols, rows, pixel width, pixel height.
    pub size: (u16, u16, u16, u16),
    /// The private keys the caller names for this destination, tried in
    /// this order and ahead of the agent. A list rather than one file
    /// because that is what names a key here: a `[[ssh.host]]` row's
    /// single `key`, or the whole `IdentityFile` sequence `~/.ssh/config`
    /// gives a host. Empty tries the agent and then the default key files
    /// `ssh` itself would (see `thread::default_key_files`).
    pub key_files: Vec<std::path::PathBuf>,
}

/// The caller's host-key trust policy, consulted on the connection thread.
pub trait HostPolicy: Send + 'static {
    /// Host-key algorithms recorded for this host, most preferred first;
    /// `None` leaves russh's default order. Consulted before connecting,
    /// because negotiation happens before the key can be checked: a host
    /// recorded only under RSA reads as unknown when the server also holds
    /// an Ed25519 key the default order prefers.
    fn key_order(&mut self, host: &str, port: u16) -> Option<Vec<russh::keys::Algorithm>>;

    /// Accept or refuse the presented key. `Err` carries the refusal text
    /// for the user's glass, which then outranks the library's own error.
    ///
    /// The `ask` is how a policy puts the decision to the person in front
    /// of the glass, and it blocks this thread until they answer. That is
    /// safe here and nowhere near safe in general: see [`ask`] for the
    /// stack this is called on and why std channels are the only ones that
    /// may be used from it.
    fn verify(
        &mut self,
        host: &str,
        port: u16,
        key: &russh::keys::PublicKeyOrCertificate,
        ask: &Asker,
    ) -> Result<(), String>;
}

/// One SSH connection: the thread's lifetime, held per bank by the surface.
/// Dropping it is the disconnect.
pub struct Link {
    cmd: tokio::sync::mpsc::UnboundedSender<thread::LinkCmd>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Link {
    /// Start the connection and hand back its first channel's endpoints.
    ///
    /// The endpoints exist before any byte moves, so the caller's row and
    /// grid exist from the first frame; progress, refusals and the shell's
    /// own bytes all arrive through the same handle, and the connection's
    /// death is the handle's `Eof`.
    ///
    /// The `asker` is the connection's line to the person at the glass;
    /// [`Asker::closed`] is a caller saying there is nobody, which turns
    /// every question into a refusal rather than a wait.
    pub fn connect(
        target: SshTarget,
        policy: Box<dyn HostPolicy>,
        asker: Asker,
    ) -> std::io::Result<(Link, ChannelHandle)> {
        let (handle, wire) = thread::endpoints();
        let (cmd, link_cmd) = tokio::sync::mpsc::unbounded_channel();
        let thread = thread::spawn(target, policy, asker, wire, link_cmd)?;
        Ok((Link { cmd, thread: Some(thread) }, handle))
    }

    /// Another channel on this connection, with its own remote pty of the
    /// given geometry. The endpoints come back at once; the shell's
    /// greeting, or the refusal, arrives through them. `Err` means the
    /// connection is already over.
    pub fn open_channel(
        &self,
        size: (u16, u16, u16, u16),
    ) -> Result<ChannelHandle, &'static str> {
        let (handle, wire) = thread::endpoints();
        self.cmd
            .send(thread::LinkCmd::Open { wire, size })
            .map_err(|_| "the connection is over")?;
        Ok(handle)
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        // The thread ends when the channels close; dropping the handles is
        // the disconnect. Joining here would stall the event loop behind a
        // network timeout, so the thread is detached by design.
        let _ = self.thread.take();
    }
}
