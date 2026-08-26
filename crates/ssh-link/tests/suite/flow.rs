//! Client against an in-process russh server on loopback, and an in-process
//! agent on a Unix socket. No sshd runs here and none is needed: the server
//! side of russh compiles unconditionally, which is what lets the trust and
//! auth paths be proven on every machine the suite runs on.
//!
//! Everything environmental (SSH_AUTH_SOCK) lives in the one test that owns
//! it, sequenced inside that test, because the test binary's threads share
//! the process environment.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use ssh_link::russh::keys::ssh_key;
use ssh_link::russh::keys::{Algorithm, PrivateKey, PublicKeyOrCertificate};
use ssh_link::russh::server::{self, Auth, Msg, Session};
use ssh_link::russh::{Channel, ChannelId, MethodSet};
use ssh_link::{ChannelHandle, HostPolicy, Link, SshTarget, WireEvent};

/// What the test server saw, for the assertions that need the far side.
#[derive(Default)]
struct Seen {
    resizes: Vec<(u32, u32)>,
}

struct Server {
    /// The one user key the server accepts.
    authorized: ssh_key::PublicKey,
    seen: Arc<Mutex<Seen>>,
}

impl server::Handler for Server {
    type Error = ssh_link::russh::Error;

    async fn auth_publickey(
        &mut self,
        _user: &str,
        key: &ssh_key::PublicKey,
    ) -> Result<Auth, Self::Error> {
        if *key == self.authorized {
            Ok(Auth::Accept)
        } else {
            Ok(Auth::Reject { proceed_with_methods: Some(MethodSet::empty()), partial_success: false })
        }
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: ssh_link::russh::server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        // The channel object pumps itself; dropping it here would close it.
        std::mem::forget(channel);
        Ok(())
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _term: &str,
        _col_width: u32,
        _row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _modes: &[(ssh_link::russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        session.data(channel, &b"ready\r\n"[..])?;
        Ok(())
    }

    /// The shell: echo, and a one-byte exit command so a test can ask for
    /// a clean remote end.
    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if data == b"\x04" {
            session.exit_status_request(channel, 7)?;
            session.eof(channel)?;
            session.close(channel)?;
        } else {
            session.data(channel, data.to_vec())?;
        }
        Ok(())
    }

    async fn window_change_request(
        &mut self,
        _channel: ChannelId,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.seen.lock().unwrap().resizes.push((col_width, row_height));
        Ok(())
    }
}

/// Bind loopback, serve every connection with [`Server`], return the port.
async fn start_server(
    host_key: PrivateKey,
    authorized: ssh_key::PublicKey,
    seen: Arc<Mutex<Seen>>,
) -> u16 {
    let config = Arc::new(server::Config {
        keys: vec![host_key],
        ..Default::default()
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else { break };
            let handler = Server { authorized: authorized.clone(), seen: seen.clone() };
            let config = config.clone();
            tokio::spawn(async move {
                let _ = server::run_stream(config, stream, handler).await;
            });
        }
    });
    port
}

/// An agent on a Unix socket holding one freshly-minted identity.
async fn start_agent(dir: &std::path::Path, identity: &PrivateKey) -> std::path::PathBuf {
    let sock = dir.join("agent.sock");
    let listener = tokio::net::UnixListener::bind(&sock).unwrap();
    tokio::spawn(ssh_link::russh::keys::agent::server::serve(
        tokio_stream::wrappers::UnixListenerStream::new(listener),
        (),
    ));
    let mut client =
        ssh_link::russh::keys::agent::client::AgentClient::connect_uds(&sock).await.unwrap();
    client.add_identity(identity, &[]).await.unwrap();
    sock
}

/// A policy for the transport tests: scripted verdict, recorded evidence.
/// The real known_hosts policy lives in the app crate with its fixtures;
/// what this side proves is that the transport obeys whatever the policy
/// says and carries its words to the wire.
struct Scripted {
    verdict: Result<(), String>,
    order: Option<Vec<Algorithm>>,
    saw: Arc<Mutex<Option<Algorithm>>>,
}

impl HostPolicy for Scripted {
    fn key_order(&mut self, _host: &str, _port: u16) -> Option<Vec<Algorithm>> {
        self.order.clone()
    }

    fn verify(
        &mut self,
        _host: &str,
        _port: u16,
        key: &PublicKeyOrCertificate,
    ) -> Result<(), String> {
        if let PublicKeyOrCertificate::PublicKey { key, .. } = key {
            *self.saw.lock().unwrap() = Some(key.algorithm());
        }
        self.verdict.clone()
    }
}

fn target(port: u16) -> SshTarget {
    SshTarget {
        user: "overseer".into(),
        host: "127.0.0.1".into(),
        port,
        term: "xterm-256color".into(),
        size: (80, 24, 720, 432),
    }
}

/// Drain events until the predicate hits or the deadline passes; the pump
/// here is the test's stand-in for the surface's 8ms poll.
fn wait_for(handle: &mut ChannelHandle, mut hit: impl FnMut(&WireEvent) -> bool) -> Vec<WireEvent> {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut log = Vec::new();
    while std::time::Instant::now() < deadline {
        while let Some(event) = handle.try_event() {
            let done = hit(&event);
            log.push(event);
            if done {
                return log;
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("deadline with no matching event; saw: {log:?}");
}

fn ed25519() -> PrivateKey {
    PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).unwrap()
}

fn text_of(log: &[WireEvent]) -> String {
    log.iter()
        .filter_map(|e| match e {
            WireEvent::Notice(text) => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The environment-touching cases in one test, sequenced: the process env
/// is shared across the binary's threads, so exactly one test owns it.
#[tokio::test(flavor = "multi_thread")]
async fn a_connection_lives_and_dies_on_the_glass() {
    let dir = tempfile::tempdir().unwrap();
    let seen = Arc::new(Mutex::new(Seen::default()));
    let identity = ed25519();
    let port = start_server(ed25519(), identity.public_key().clone(), seen.clone()).await;

    // No agent: the failure names the remedy and the row ends.
    std::env::remove_var("SSH_AUTH_SOCK");
    let saw = Arc::new(Mutex::new(None));
    let policy = Scripted { verdict: Ok(()), order: None, saw: saw.clone() };
    let (_link, mut handle) = Link::connect(target(port), Box::new(policy)).unwrap();
    let log = tokio::task::spawn_blocking(move || {
        wait_for(&mut handle, |e| matches!(e, WireEvent::Eof))
    })
    .await
    .unwrap();
    assert!(text_of(&log).contains("ssh-add"), "no remedy named: {log:?}");

    // Agent up: connect, shell, echo, resize, remote exit — the whole life.
    let sock = start_agent(dir.path(), &identity).await;
    std::env::set_var("SSH_AUTH_SOCK", &sock);
    let saw = Arc::new(Mutex::new(None));
    let policy = Scripted { verdict: Ok(()), order: None, saw: saw.clone() };
    let (_link, mut handle) = Link::connect(target(port), Box::new(policy)).unwrap();
    let seen_far = seen.clone();
    let log = tokio::task::spawn_blocking(move || {
        let mut log = wait_for(&mut handle, |e| {
            matches!(e, WireEvent::Data(d) if d.windows(5).any(|w| w == b"ready"))
        });
        handle.send(b"hello");
        log.extend(wait_for(&mut handle, |e| {
            matches!(e, WireEvent::Data(d) if d.windows(5).any(|w| w == b"hello"))
        }));
        handle.window_change(132, 43, 1188, 774);
        let resized = std::time::Instant::now() + Duration::from_secs(10);
        while seen_far.lock().unwrap().resizes.is_empty() {
            assert!(std::time::Instant::now() < resized, "no resize reached the server");
            std::thread::sleep(Duration::from_millis(5));
        }
        handle.send(b"\x04");
        log.extend(wait_for(&mut handle, |e| matches!(e, WireEvent::Eof)));
        log
    })
    .await
    .unwrap();
    assert_eq!(seen.lock().unwrap().resizes, vec![(132, 43)]);
    assert!(
        log.iter().any(|e| matches!(e, WireEvent::ExitStatus(7))),
        "exit status lost: {log:?}"
    );
    assert!(text_of(&log).contains("authenticated as overseer"));
    assert_eq!(*saw.lock().unwrap(), Some(Algorithm::Ed25519));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_host_key_speaks_the_policy_and_ends_the_row() {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let identity = ed25519();
    let port = start_server(ed25519(), identity.public_key().clone(), seen).await;

    let policy = Scripted {
        verdict: Err("the vault door stays shut: unknown host key".into()),
        order: None,
        saw: Arc::new(Mutex::new(None)),
    };
    let (_link, mut handle) = Link::connect(target(port), Box::new(policy)).unwrap();
    let log = tokio::task::spawn_blocking(move || {
        wait_for(&mut handle, |e| matches!(e, WireEvent::Eof))
    })
    .await
    .unwrap();
    // The policy's words, not the library's, and no auth was attempted
    // (nothing here read SSH_AUTH_SOCK: refusal precedes auth).
    assert!(text_of(&log).contains("the vault door stays shut"), "{log:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_recorded_algorithm_leads_the_negotiation() {
    // The server holds Ed25519 and P-256 and follows the client's
    // preference; a policy that recorded the host under P-256 must see a
    // P-256 key presented, or a real known_hosts file full of older
    // entries reads as a wall of unknown hosts.
    let seen = Arc::new(Mutex::new(Seen::default()));
    let identity = ed25519();
    let host_ed = ed25519();
    let host_p256 =
        PrivateKey::random(&mut rand::rng(), Algorithm::Ecdsa { curve: ssh_key::EcdsaCurve::NistP256 })
            .unwrap();
    let config = Arc::new(server::Config {
        keys: vec![host_ed, host_p256],
        ..Default::default()
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let authorized = identity.public_key().clone();
    tokio::spawn(async move {
        let Ok((stream, _)) = listener.accept().await else { return };
        let handler = Server { authorized, seen };
        let _ = server::run_stream(config, stream, handler).await;
    });

    let saw = Arc::new(Mutex::new(None));
    let policy = Scripted {
        verdict: Err("recorded under P-256; stopping here proves the order".into()),
        order: Some(vec![Algorithm::Ecdsa { curve: ssh_key::EcdsaCurve::NistP256 }]),
        saw: saw.clone(),
    };
    let (_link, mut handle) = Link::connect(target(port), Box::new(policy)).unwrap();
    tokio::task::spawn_blocking(move || wait_for(&mut handle, |e| matches!(e, WireEvent::Eof)))
        .await
        .unwrap();
    assert_eq!(
        *saw.lock().unwrap(),
        Some(Algorithm::Ecdsa { curve: ssh_key::EcdsaCurve::NistP256 }),
        "the server led with a key the policy never asked for"
    );
}
