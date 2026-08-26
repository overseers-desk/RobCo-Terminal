//! Client against the in-process far side in `ssh_link::test_server`.
//! No sshd runs here and none is needed; see that module's doc.
//!
//! Everything environmental (SSH_AUTH_SOCK) lives in the one test that owns
//! it, sequenced inside that test, because the test binary's threads share
//! the process environment.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use ssh_link::russh::keys::{ssh_key, Algorithm, PrivateKey, PublicKeyOrCertificate};
use ssh_link::test_server::{mint, serve_agent, serve_echo};
use ssh_link::{ChannelHandle, HostPolicy, Link, SshTarget, WireEvent};

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
        key_file: None,
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
    mint(Algorithm::Ed25519)
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
/// `HOME` is pointed at the scratch directory, so the default-key scan
/// reads this test's `.ssh` and never the developer's.
#[tokio::test(flavor = "multi_thread")]
async fn a_connection_lives_and_dies_on_the_glass() {
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("HOME", dir.path());
    let identity = ed25519();
    let (port, seen) = serve_echo(vec![ed25519()], identity.public_key().clone()).await;

    // No agent, no key anywhere: the closing line names what the server
    // would take, and the row ends.
    std::env::remove_var("SSH_AUTH_SOCK");
    let saw = Arc::new(Mutex::new(None));
    let policy = Scripted { verdict: Ok(()), order: None, saw: saw.clone() };
    let (_link, mut handle) = Link::connect(target(port), Box::new(policy)).unwrap();
    let log = tokio::task::spawn_blocking(move || {
        wait_for(&mut handle, |e| matches!(e, WireEvent::Eof))
    })
    .await
    .unwrap();
    assert!(text_of(&log).contains("no agent is reachable"), "{log:?}");
    assert!(text_of(&log).contains("the server offers"), "{log:?}");

    // A named key file authenticates with no agent in the world.
    let key_path = dir.path().join("vault_key");
    std::fs::write(
        &key_path,
        identity.to_openssh(ssh_key::LineEnding::LF).unwrap().as_bytes(),
    )
    .unwrap();
    let policy = Scripted { verdict: Ok(()), order: None, saw: Arc::new(Mutex::new(None)) };
    let mut with_key = target(port);
    with_key.key_file = Some(key_path.clone());
    let (_key_link, mut handle) = Link::connect(with_key, Box::new(policy)).unwrap();
    let log = tokio::task::spawn_blocking(move || {
        wait_for(&mut handle, |e| {
            matches!(e, WireEvent::Data(d) if d.windows(5).any(|w| w == b"ready"))
        })
    })
    .await
    .unwrap();
    assert!(text_of(&log).contains("authenticated as overseer"), "{log:?}");

    // An encrypted key is named and skipped, never silently ignored: its
    // passphrase waits on the prompt surface (#14).
    let sealed = identity.encrypt(&mut rand::rng(), "tumblers").unwrap();
    let sealed_path = dir.path().join("sealed_key");
    std::fs::write(
        &sealed_path,
        sealed.to_openssh(ssh_key::LineEnding::LF).unwrap().as_bytes(),
    )
    .unwrap();
    let policy = Scripted { verdict: Ok(()), order: None, saw: Arc::new(Mutex::new(None)) };
    let mut with_sealed = target(port);
    with_sealed.key_file = Some(sealed_path);
    let (_sealed_link, mut handle) = Link::connect(with_sealed, Box::new(policy)).unwrap();
    let log = tokio::task::spawn_blocking(move || {
        wait_for(&mut handle, |e| matches!(e, WireEvent::Eof))
    })
    .await
    .unwrap();
    assert!(
        text_of(&log).contains("cannot ask for its passphrase yet"),
        "{log:?}"
    );

    // Agent up: connect, shell, echo, resize, remote exit, the whole life.
    let sock = serve_agent(dir.path(), &identity).await;
    std::env::set_var("SSH_AUTH_SOCK", &sock);
    let saw = Arc::new(Mutex::new(None));
    let policy = Scripted { verdict: Ok(()), order: None, saw: saw.clone() };
    let (link, mut handle) = Link::connect(target(port), Box::new(policy)).unwrap();
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
        // A second channel multiplexes over the same connection, lives
        // its own life, and its close leaves the first standing.
        let mut second = link.open_channel((80, 24, 720, 432)).expect("open");
        wait_for(&mut second, |e| {
            matches!(e, WireEvent::Data(d) if d.windows(5).any(|w| w == b"ready"))
        });
        second.send(b"twin");
        wait_for(&mut second, |e| {
            matches!(e, WireEvent::Data(d) if d.windows(4).any(|w| w == b"twin"))
        });
        drop(second);
        handle.send(b"still here");
        log.extend(wait_for(&mut handle, |e| {
            matches!(e, WireEvent::Data(d) if d.windows(10).any(|w| w == b"still here"))
        }));

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

    // With the agent gone again and no key named, a key sitting in the
    // default position carries the connection on its own.
    std::env::remove_var("SSH_AUTH_SOCK");
    std::fs::create_dir_all(dir.path().join(".ssh")).unwrap();
    std::fs::write(
        dir.path().join(".ssh").join("id_ed25519"),
        identity.to_openssh(ssh_key::LineEnding::LF).unwrap().as_bytes(),
    )
    .unwrap();
    let policy = Scripted { verdict: Ok(()), order: None, saw: Arc::new(Mutex::new(None)) };
    let (_default_link, mut handle) = Link::connect(target(port), Box::new(policy)).unwrap();
    let log = tokio::task::spawn_blocking(move || {
        wait_for(&mut handle, |e| {
            matches!(e, WireEvent::Data(d) if d.windows(5).any(|w| w == b"ready"))
        })
    })
    .await
    .unwrap();
    assert!(text_of(&log).contains("id_ed25519"), "{log:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_host_key_speaks_the_policy_and_ends_the_row() {
    let identity = ed25519();
    let (port, _seen) = serve_echo(vec![ed25519()], identity.public_key().clone()).await;

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
    let identity = ed25519();
    let host_ed = ed25519();
    let host_p256 = mint(Algorithm::Ecdsa { curve: ssh_key::EcdsaCurve::NistP256 });
    let (port, _seen) = serve_echo(
        vec![host_ed, host_p256],
        identity.public_key().clone(),
    )
    .await;

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
