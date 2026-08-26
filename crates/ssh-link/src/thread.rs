//! The connection's own thread: a current-thread runtime driving russh,
//! feeding the loop-side endpoints in `channel`. The comments here and in
//! `channel` call this thread's [`run`] loop the supervisor: it connects,
//! authenticates, opens channels, and outlives every channel task it
//! spawns.
//!
//! One thread per connection, named after the host the way the
//! single-instance listener is named for its job. The thread ends when the
//! connection does, whichever side ended it; every channel hears `Eof`
//! first, so no row outlives its wire silently.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use russh::client;
use russh::keys::agent::client::AgentClient;
use russh::keys::agent::AgentIdentity;
use russh::keys::PublicKeyOrCertificate;
use russh::client::AuthResult;
use russh::ChannelMsg;
use tokio::sync::mpsc;

use crate::channel::{ChannelCmd, WireEvent, EVENT_QUEUE};
use crate::{ChannelHandle, HostPolicy, SshTarget};

/// Commands from the loop side to the connection itself.
pub(crate) enum LinkCmd {
    /// Another channel on this connection: shell it, then drive its wire.
    Open { wire: ChannelWire, size: (u16, u16, u16, u16) },
}

/// The supervisor's grip on one channel's loop-side endpoints.
pub(crate) struct ChannelWire {
    pub events: mpsc::Sender<WireEvent>,
    pub cmd: mpsc::UnboundedReceiver<ChannelCmd>,
    pub queued: Arc<std::sync::atomic::AtomicUsize>,
}

pub(crate) fn endpoints() -> (ChannelHandle, ChannelWire) {
    let (event_tx, event_rx) = mpsc::channel(EVENT_QUEUE);
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let queued = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let sheds = Arc::new(std::sync::atomic::AtomicU64::new(0));
    (
        ChannelHandle { events: event_rx, cmd: cmd_tx, queued: queued.clone(), sheds },
        ChannelWire { events: event_tx, cmd: cmd_rx, queued },
    )
}

/// Spawn the connection thread. The wire is the first (and in stage 1 only)
/// channel's; its `Eof` is how the loop side learns the connection is over.
pub(crate) fn spawn(
    target: SshTarget,
    policy: Box<dyn HostPolicy>,
    wire: ChannelWire,
    link_cmd: mpsc::UnboundedReceiver<LinkCmd>,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name(format!("ssh-{}", target.host))
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .enable_time()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    // No runtime, no wire pump: the blocking sender is safe
                    // here because nothing async exists yet.
                    let _ = wire.events.blocking_send(WireEvent::Notice(format!(
                        "runtime failed to start: {e}"
                    )));
                    let _ = wire.events.blocking_send(WireEvent::Eof);
                    return;
                }
            };
            rt.block_on(run(target, policy, wire, link_cmd));
        })
}

/// What `check_server_key` leaves behind for the error path: the policy's
/// refusal text, which russh's own error does not carry.
struct Verdicts {
    refusal: Mutex<Option<String>>,
}

struct Handler {
    policy: Box<dyn HostPolicy>,
    host: String,
    port: u16,
    verdicts: Arc<Verdicts>,
}

impl client::Handler for Handler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        match self.policy.verify(&self.host, self.port, key) {
            Ok(()) => Ok(true),
            Err(text) => {
                *self.verdicts.refusal.lock().unwrap() = Some(text);
                Ok(false)
            }
        }
    }
}

async fn notice(wire: &ChannelWire, text: impl Into<String>) {
    let _ = wire.events.send(WireEvent::Notice(text.into())).await;
}

async fn run(
    target: SshTarget,
    mut policy: Box<dyn HostPolicy>,
    wire: ChannelWire,
    mut link_cmd: mpsc::UnboundedReceiver<LinkCmd>,
) {
    let verdicts = Arc::new(Verdicts { refusal: Mutex::new(None) });

    notice(&wire, format!("connecting to {}:{}", target.host, target.port)).await;

    // The recorded algorithms lead the preference list, or a host recorded
    // under one algorithm reads as unknown when the server also holds a
    // preferred one.
    let mut config = client::Config::default();
    if let Some(order) = policy.key_order(&target.host, target.port) {
        if !order.is_empty() {
            let mut key: Vec<_> = order;
            for alg in config.preferred.key.iter() {
                if !key.contains(alg) {
                    key.push(alg.clone());
                }
            }
            config.preferred.key = key.into();
        }
    }

    let handler = Handler {
        policy,
        host: target.host.clone(),
        port: target.port,
        verdicts: verdicts.clone(),
    };

    let mut handle = match client::connect(
        Arc::new(config),
        (target.host.as_str(), target.port),
        handler,
    )
    .await
    {
        Ok(handle) => handle,
        Err(e) => {
            // The policy's own words beat russh's, when it spoke.
            let text = match verdicts.refusal.lock().unwrap().take() {
                Some(refusal) => refusal,
                None => format!("connection failed: {e}"),
            };
            notice(&wire, text).await;
            let _ = wire.events.send(WireEvent::Eof).await;
            return;
        }
    };

    if !authenticate(&mut handle, &target, &wire).await {
        let _ = wire.events.send(WireEvent::Eof).await;
        return;
    }

    // The first channel, then any the loop side asks for: opened here (the
    // handle is the supervisor's alone), each then driven on a task of its
    // own -- this runtime is one thread, so a task is concurrency, not
    // parallelism. The supervisor ends when the loop side drops the Link,
    // and the disconnect then ends every channel task through its Eof.
    raise(&handle, &target.term, target.size, wire).await;
    while let Some(LinkCmd::Open { wire, size }) = link_cmd.recv().await {
        raise(&handle, &target.term, size, wire).await;
    }
    let _ = handle
        .disconnect(russh::Disconnect::ByApplication, "", "en")
        .await;
}

/// Open a session channel with a pty and shell on it, and put its pump on
/// a task. A refusal at any step lands on the wire and ends with `Eof`, so
/// the row asking hears why.
async fn raise(
    handle: &client::Handle<Handler>,
    term: &str,
    size: (u16, u16, u16, u16),
    wire: ChannelWire,
) {
    let channel = match handle.channel_open_session().await {
        Ok(c) => c,
        Err(e) => {
            notice(&wire, format!("channel refused: {e}")).await;
            let _ = wire.events.send(WireEvent::Eof).await;
            return;
        }
    };
    let (cols, rows, pix_w, pix_h) = size;
    let pty = channel
        .request_pty(
            true,
            term,
            u32::from(cols),
            u32::from(rows),
            u32::from(pix_w),
            u32::from(pix_h),
            &[],
        )
        .await;
    let shell = match pty {
        Ok(()) => channel.request_shell(true).await,
        Err(e) => Err(e),
    };
    if let Err(e) = shell {
        notice(&wire, format!("shell refused: {e}")).await;
        let _ = wire.events.send(WireEvent::Eof).await;
        return;
    }
    tokio::spawn(drive(channel, wire));
}

/// Pump one raised channel against its wire until either side ends it.
async fn drive(channel: russh::Channel<client::Msg>, mut wire: ChannelWire) {
    let mut channel = channel;
    loop {
        tokio::select! {
            cmd = wire.cmd.recv() => match cmd {
                Some(ChannelCmd::Data(bytes)) => {
                    let len = bytes.len();
                    let sent = channel.data(&bytes[..]).await;
                    wire.queued.fetch_sub(len, Ordering::Relaxed);
                    if sent.is_err() {
                        break;
                    }
                }
                Some(ChannelCmd::WindowChange { cols, rows, pix_w, pix_h }) => {
                    let _ = channel
                        .window_change(
                            u32::from(cols),
                            u32::from(rows),
                            u32::from(pix_w),
                            u32::from(pix_h),
                        )
                        .await;
                }
                // Close, or the loop side dropped the handle: this channel
                // is done, and the connection's own end is the Link's.
                Some(ChannelCmd::Close) | None => break,
            },
            msg = channel.wait() => match msg {
                Some(ChannelMsg::Data { data }) => {
                    if wire.events.send(WireEvent::Data(data.to_vec())).await.is_err() {
                        break;
                    }
                }
                // A pty merges stderr into the stream server-side; extended
                // data still arrives for a session without one, and the
                // glass is where it belongs either way.
                Some(ChannelMsg::ExtendedData { data, .. }) => {
                    if wire.events.send(WireEvent::Data(data.to_vec())).await.is_err() {
                        break;
                    }
                }
                Some(ChannelMsg::ExitStatus { exit_status }) => {
                    let _ = wire.events.send(WireEvent::ExitStatus(exit_status)).await;
                }
                Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
                Some(_) => {}
            },
        }
    }

    let _ = channel.eof().await;
    let _ = wire.events.send(WireEvent::Eof).await;
}

/// Agent-backed publickey auth, the one method this build speaks. Returns
/// whether the session is authenticated; on failure the refusal the user
/// can act on is already on the wire.
async fn authenticate(
    handle: &mut client::Handle<Handler>,
    target: &SshTarget,
    wire: &ChannelWire,
) -> bool {
    // Not an auth attempt that could succeed: the reply's method list is
    // what makes every later refusal name what the server would take.
    let offered = match handle.authenticate_none(target.user.clone()).await {
        Ok(AuthResult::Success) => return true,
        Ok(AuthResult::Failure { remaining_methods, .. }) => format!("{remaining_methods:?}"),
        Err(e) => {
            notice(wire, format!("authentication failed: {e}")).await;
            return false;
        }
    };
    agent_auth(handle, target, wire, &offered).await
}

/// This platform's agent: whatever `SSH_AUTH_SOCK` names.
#[cfg(unix)]
async fn agent_auth(
    handle: &mut client::Handle<Handler>,
    target: &SshTarget,
    wire: &ChannelWire,
    offered: &str,
) -> bool {
    let agent = match AgentClient::connect_env().await {
        Ok(agent) => agent,
        Err(_) => {
            notice(wire, "SSH_AUTH_SOCK is unset or dead: run ssh-add, then retry").await;
            notice(wire, format!("(this build authenticates with the agent only; the server offers {offered})")).await;
            return false;
        }
    };
    sign_with(handle, target, wire, offered, agent).await
}

/// This platform's agents: the OpenSSH Authentication Agent service on its
/// fixed pipe name, then Pageant. Tried in that order because the service
/// ships with Windows and Pageant is the add-on.
#[cfg(windows)]
async fn agent_auth(
    handle: &mut client::Handle<Handler>,
    target: &SshTarget,
    wire: &ChannelWire,
    offered: &str,
) -> bool {
    const OPENSSH_AGENT_PIPE: &str = r"\\.\pipe\openssh-ssh-agent";
    if let Ok(agent) = AgentClient::connect_named_pipe(OPENSSH_AGENT_PIPE).await {
        return sign_with(handle, target, wire, offered, agent).await;
    }
    match AgentClient::connect_pageant().await {
        Ok(agent) => sign_with(handle, target, wire, offered, agent).await,
        Err(_) => {
            notice(
                wire,
                "no agent answered: neither the OpenSSH Authentication Agent service nor Pageant is running",
            )
            .await;
            notice(wire, format!("(this build authenticates with the agent only; the server offers {offered})")).await;
            false
        }
    }
}

/// The identities loop, blind to which stream the agent answers on.
async fn sign_with<S>(
    handle: &mut client::Handle<Handler>,
    target: &SshTarget,
    wire: &ChannelWire,
    offered: &str,
    mut agent: AgentClient<S>,
) -> bool
where
    S: russh::keys::agent::client::AgentStream + Send + Unpin,
{
    let identities = match agent.request_identities().await {
        Ok(ids) => ids,
        Err(e) => {
            notice(wire, format!("the agent refused to list identities: {e}")).await;
            return false;
        }
    };
    if identities.is_empty() {
        notice(wire, "the agent holds no identities: run ssh-add, then retry").await;
        return false;
    }

    for identity in identities {
        let AgentIdentity::PublicKey { key, comment } = identity else {
            // A certificate identity waits on the trust design that #14
            // parks with the interface; skipping it is the legible choice.
            continue;
        };
        let hash_alg = if key.algorithm().is_rsa() {
            match handle.best_supported_rsa_hash().await {
                Ok(Some(alg)) => alg,
                _ => None,
            }
        } else {
            None
        };
        match handle
            .authenticate_publickey_with(target.user.clone(), key, hash_alg, &mut agent)
            .await
        {
            Ok(AuthResult::Success) => {
                notice(wire, format!("authenticated as {} ({comment})", target.user)).await;
                return true;
            }
            Ok(AuthResult::Failure { .. }) => continue,
            Err(e) => {
                notice(wire, format!("agent signing failed: {e}")).await;
                return false;
            }
        }
    }

    notice(wire, format!("no identity in the agent was accepted; the server offers {offered}")).await;
    false
}
