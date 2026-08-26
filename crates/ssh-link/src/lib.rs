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
//! The auth surface is deliberately narrow while the operator interface is
//! built elsewhere (#14): the agent path lives here whole, and the prompted
//! methods (password, keyboard-interactive) enter as a step-drivable
//! exchange when there is an input surface to drive them.

mod channel;
#[cfg(feature = "test-server")]
pub mod test_server;
mod thread;

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
    fn verify(
        &mut self,
        host: &str,
        port: u16,
        key: &russh::keys::PublicKeyOrCertificate,
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
    pub fn connect(
        target: SshTarget,
        policy: Box<dyn HostPolicy>,
    ) -> std::io::Result<(Link, ChannelHandle)> {
        let (handle, wire) = thread::endpoints();
        let (cmd, link_cmd) = tokio::sync::mpsc::unbounded_channel();
        let thread = thread::spawn(target, policy, wire, link_cmd)?;
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
