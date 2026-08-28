//! Client against the in-process far side in `ssh_link::test_server`.
//! No sshd runs here and none is needed; see that module's doc.
//!
//! Everything environmental (SSH_AUTH_SOCK) lives in the one test that owns
//! it, sequenced inside that test, because the test binary's threads share
//! the process environment.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use ssh_link::russh::keys::{ssh_key, Algorithm, PrivateKey, PublicKeyOrCertificate};
use ssh_link::test_server::{mint, serve_agent, serve_echo, serve_with, AuthPlan};
use ssh_link::{Answer, Asker, ChannelHandle, HostPolicy, Link, SshTarget, WireEvent};

/// A policy for the transport tests: scripted verdict, recorded evidence.
/// The real known_hosts policy lives in the app crate with its fixtures;
/// what this side proves is that the transport obeys whatever the policy
/// says and carries its words to the wire.
struct Scripted {
    verdict: Result<(), String>,
    order: Option<Vec<Algorithm>>,
    saw: Arc<Mutex<Option<Algorithm>>>,
    /// When set, the policy asks this before deciding, and the answer
    /// decides: what proves the question reached a desk and the verdict
    /// came back to the handshake.
    consult: Option<String>,
}

impl Scripted {
    fn verdict(verdict: Result<(), String>) -> Self {
        Self { verdict, order: None, saw: Arc::new(Mutex::new(None)), consult: None }
    }

    fn watching(verdict: Result<(), String>, saw: Arc<Mutex<Option<Algorithm>>>) -> Self {
        Self { verdict, order: None, saw, consult: None }
    }
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
        ask: &Asker,
    ) -> Result<(), String> {
        if let PublicKeyOrCertificate::PublicKey { key, .. } = key {
            *self.saw.lock().unwrap() = Some(key.algorithm());
        }
        if let Some(question) = &self.consult {
            return match ask.ask(question.clone(), Answer::YesNo).as_deref() {
                Some("yes") => Ok(()),
                other => Err(format!("the desk said {other:?}")),
            };
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
        key_files: Vec::new(),
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
            let ended = !done && matches!(event, WireEvent::Eof);
            log.push(event);
            if done {
                return log;
            }
            // Eof is terminal, and a dead handle repeats it on every poll
            // (channel.rs's "no row outlives its wire"): a wait that is
            // not for Eof must end here, or the log grows Eofs until the
            // test binary is the process the OOM killer picks.
            if ended {
                panic!("the connection ended before the matching event; saw: {log:?}");
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
    let policy = Scripted::watching(Ok(()), saw.clone());
    let (_link, mut handle) =
        Link::connect(target(port), Box::new(policy), Asker::closed()).unwrap();
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
    let policy = Scripted::verdict(Ok(()));
    let mut with_key = target(port);
    with_key.key_files = vec![key_path.clone()];
    let (_key_link, mut handle) =
        Link::connect(with_key, Box::new(policy), Asker::closed()).unwrap();
    let log = tokio::task::spawn_blocking(move || {
        wait_for(&mut handle, |e| {
            matches!(e, WireEvent::Data(d) if d.windows(5).any(|w| w == b"ready"))
        })
    })
    .await
    .unwrap();
    assert!(text_of(&log).contains("authenticated as overseer"), "{log:?}");

    // An encrypted key is asked about. With nobody at the desk there is
    // nobody to ask, so it is a cancellation and the row says so.
    let sealed = identity.encrypt(&mut rand::rng(), "tumblers").unwrap();
    let sealed_path = dir.path().join("sealed_key");
    std::fs::write(
        &sealed_path,
        sealed.to_openssh(ssh_key::LineEnding::LF).unwrap().as_bytes(),
    )
    .unwrap();
    let policy = Scripted::verdict(Ok(()));
    let mut with_sealed = target(port);
    with_sealed.key_files = vec![sealed_path];
    let (_sealed_link, mut handle) =
        Link::connect(with_sealed, Box::new(policy), Asker::closed()).unwrap();
    let log = tokio::task::spawn_blocking(move || {
        wait_for(&mut handle, |e| matches!(e, WireEvent::Eof))
    })
    .await
    .unwrap();
    assert!(text_of(&log).contains("the question was cancelled"), "{log:?}");

    // Agent up: connect, shell, echo, resize, remote exit, the whole life.
    let sock = serve_agent(dir.path(), &identity).await;
    std::env::set_var("SSH_AUTH_SOCK", &sock);
    let saw = Arc::new(Mutex::new(None));
    let policy = Scripted::watching(Ok(()), saw.clone());
    let (link, mut handle) =
        Link::connect(target(port), Box::new(policy), Asker::closed()).unwrap();
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
    let policy = Scripted::verdict(Ok(()));
    let (_default_link, mut handle) =
        Link::connect(target(port), Box::new(policy), Asker::closed()).unwrap();
    let log = tokio::task::spawn_blocking(move || {
        wait_for(&mut handle, |e| {
            matches!(e, WireEvent::Data(d) if d.windows(5).any(|w| w == b"ready"))
        })
    })
    .await
    .unwrap();
    assert!(text_of(&log).contains("id_ed25519"), "{log:?}");
}

/// Somebody at the desk, answering from a script: one entry per question,
/// `None` for the answer nobody gives. Answers the transcript -- every
/// prompt asked and every line said, in order.
///
/// It runs on a thread of its own because that is where a surface would
/// be: the asking side blocks, so the answering side cannot be the same
/// thread.
fn deskhand(
    mut desk: ssh_link::AskDesk,
    answers: Vec<Option<&'static str>>,
) -> std::thread::JoinHandle<Vec<String>> {
    std::thread::spawn(move || {
        let mut transcript = Vec::new();
        for answer in answers {
            let deadline = std::time::Instant::now() + Duration::from_secs(20);
            loop {
                match desk.take() {
                    Some(ssh_link::Ask::Question(question)) => {
                        transcript.push(question.prompt().to_string());
                        match answer {
                            Some(text) => question.answer(text.to_string()),
                            None => question.cancel(),
                        }
                        break;
                    }
                    Some(ssh_link::Ask::Say(text)) => transcript.push(text),
                    None => {
                        assert!(
                            std::time::Instant::now() < deadline,
                            "nothing was asked; heard: {transcript:?}"
                        );
                        std::thread::sleep(Duration::from_millis(5));
                    }
                }
            }
        }
        transcript
    })
}

/// An encrypted key file and the two ways the question about it can end.
/// The key is named on the target, so it is tried before the agent and
/// this test owes nothing to the environment.
#[tokio::test(flavor = "multi_thread")]
async fn an_encrypted_key_opens_on_a_passphrase_and_a_cancel_ends_the_attempt() {
    let dir = tempfile::tempdir().unwrap();
    let identity = ed25519();
    let (port, _seen) = serve_echo(vec![ed25519()], identity.public_key().clone()).await;
    let sealed = identity.encrypt(&mut rand::rng(), "tumblers").unwrap();
    let path = dir.path().join("sealed_key");
    std::fs::write(
        &path,
        sealed.to_openssh(ssh_key::LineEnding::LF).unwrap().as_bytes(),
    )
    .unwrap();

    // Wrong, then right: the second attempt gets in, and the line between
    // the two says which of them was wrong.
    let (asker, desk) = ssh_link::ask::desk();
    let hand = deskhand(desk, vec![Some("wrong"), Some("tumblers")]);
    let mut sealed_target = target(port);
    sealed_target.key_files = vec![path.clone()];
    let (_link, mut handle) =
        Link::connect(sealed_target, Box::new(Scripted::verdict(Ok(()))), asker).unwrap();
    let log = tokio::task::spawn_blocking(move || {
        wait_for(&mut handle, |e| {
            matches!(e, WireEvent::Data(d) if d.windows(5).any(|w| w == b"ready"))
        })
    })
    .await
    .unwrap();
    let prompts = hand.join().unwrap();
    assert_eq!(prompts.len(), 2, "{prompts:?}");
    assert!(prompts[0].contains("passphrase for"), "{prompts:?}");
    assert!(prompts[0].contains("sealed_key"), "{prompts:?}");
    assert!(text_of(&log).contains("that passphrase did not open"), "{log:?}");
    assert!(text_of(&log).contains("authenticated as overseer"), "{log:?}");

    // Withdrawn: the attempt ends where it stands. No agent is tried, no
    // default key is tried, and no shell ever exists.
    let (asker, desk) = ssh_link::ask::desk();
    let hand = deskhand(desk, vec![None]);
    let mut sealed_target = target(port);
    sealed_target.key_files = vec![path];
    let (_link, mut handle) =
        Link::connect(sealed_target, Box::new(Scripted::verdict(Ok(()))), asker).unwrap();
    let log = tokio::task::spawn_blocking(move || {
        wait_for(&mut handle, |e| matches!(e, WireEvent::Eof))
    })
    .await
    .unwrap();
    hand.join().unwrap();
    assert!(text_of(&log).contains("the question was cancelled"), "{log:?}");
    assert!(!text_of(&log).contains("agent"), "the sequence went on: {log:?}");
    assert!(
        !log.iter().any(|e| matches!(e, WireEvent::Data(_))),
        "a cancelled question opened a shell: {log:?}"
    );
}

/// A password server: wrong once, then right, then a run of three wrong
/// ones that closes the row.
#[tokio::test(flavor = "multi_thread")]
async fn a_password_is_asked_for_retried_and_finally_given_up_on() {
    let (port, _seen) = serve_with(
        vec![ed25519()],
        AuthPlan::Password {
            user: "overseer".into(),
            password: "tumblers".into(),
            refuse_first: 0,
        },
    )
    .await;

    let (asker, desk) = ssh_link::ask::desk();
    let hand = deskhand(desk, vec![Some("wrong"), Some("tumblers")]);
    let (_link, mut handle) =
        Link::connect(target(port), Box::new(Scripted::verdict(Ok(()))), asker).unwrap();
    let log = tokio::task::spawn_blocking(move || {
        wait_for(&mut handle, |e| {
            matches!(e, WireEvent::Data(d) if d.windows(5).any(|w| w == b"ready"))
        })
    })
    .await
    .unwrap();
    let prompts = hand.join().unwrap();
    assert_eq!(prompts.len(), 2, "{prompts:?}");
    assert!(prompts[0].contains("overseer@127.0.0.1's password"), "{prompts:?}");
    assert!(text_of(&log).contains("permission denied, please try again"), "{log:?}");
    assert!(text_of(&log).contains("authenticated as overseer (password)"), "{log:?}");
    // A key-only method was never reached for: the server offers password
    // alone, so nothing here went looking for an agent.
    assert!(!text_of(&log).contains("agent"), "{log:?}");

    // Three wrong ones and the row closes with the summary naming what the
    // server would have taken.
    let (asker, desk) = ssh_link::ask::desk();
    let hand = deskhand(desk, vec![Some("no"), Some("nope"), Some("never")]);
    let (_link, mut handle) =
        Link::connect(target(port), Box::new(Scripted::verdict(Ok(()))), asker).unwrap();
    let log = tokio::task::spawn_blocking(move || {
        wait_for(&mut handle, |e| matches!(e, WireEvent::Eof))
    })
    .await
    .unwrap();
    let prompts = hand.join().unwrap();
    assert_eq!(prompts.len(), 3, "three tries, no more: {prompts:?}");
    assert!(text_of(&log).contains("authentication failed; the server offers"), "{log:?}");
    assert!(text_of(&log).contains("Password"), "{log:?}");
}

/// The server composes the questions and this asks them in its order,
/// with its own echo flag on each.
#[tokio::test(flavor = "multi_thread")]
async fn a_servers_challenge_is_asked_in_its_own_order() {
    let (port, _seen) = serve_with(
        vec![ed25519()],
        AuthPlan::KeyboardInteractive {
            answers: vec!["A-2472".into(), "tumblers".into()],
        },
    )
    .await;

    let (asker, desk) = ssh_link::ask::desk();
    let hand = deskhand(desk, vec![Some("A-2472"), Some("tumblers")]);
    let (_link, mut handle) =
        Link::connect(target(port), Box::new(Scripted::verdict(Ok(()))), asker).unwrap();
    let log = tokio::task::spawn_blocking(move || {
        wait_for(&mut handle, |e| {
            matches!(e, WireEvent::Data(d) if d.windows(5).any(|w| w == b"ready"))
        })
    })
    .await
    .unwrap();
    let prompts = hand.join().unwrap();
    assert_eq!(
        prompts,
        vec!["Employee number: ".to_string(), "Passphrase: ".to_string()],
        "the server's prompts, in the server's order"
    );
    // Its framing went on the wire ahead of them.
    assert!(text_of(&log).contains("Vault-Tec Overseer Terminal"), "{log:?}");
    assert!(text_of(&log).contains("Two questions before the door opens."), "{log:?}");
    assert!(
        text_of(&log).contains("authenticated as overseer (keyboard-interactive)"),
        "{log:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_host_key_speaks_the_policy_and_ends_the_row() {
    let identity = ed25519();
    let (port, _seen) = serve_echo(vec![ed25519()], identity.public_key().clone()).await;

    let policy = Scripted::verdict(Err("the vault door stays shut: unknown host key".into()));
    let (_link, mut handle) =
        Link::connect(target(port), Box::new(policy), Asker::closed()).unwrap();
    let log = tokio::task::spawn_blocking(move || {
        wait_for(&mut handle, |e| matches!(e, WireEvent::Eof))
    })
    .await
    .unwrap();
    // The policy's words, not the library's, and no auth was attempted
    // (nothing here read SSH_AUTH_SOCK: refusal precedes auth).
    assert!(text_of(&log).contains("the vault door stays shut"), "{log:?}");
}

/// The question channel end to end through the transport: a policy asks
/// from inside the handshake, the question surfaces at a desk on another
/// thread, and the answer typed there is the verdict the connection acts
/// on. Both answers, because a trust question that could only say yes
/// would not be a question.
#[tokio::test(flavor = "multi_thread")]
async fn a_policys_question_reaches_the_desk_and_the_answer_is_the_verdict() {
    let identity = ed25519();
    let (port, _seen) = serve_echo(vec![ed25519()], identity.public_key().clone()).await;

    for (answer, refused) in [("yes", false), ("no", true)] {
        let (asker, mut desk) = ssh_link::ask::desk();
        let mut policy = Scripted::verdict(Ok(()));
        policy.consult = Some("accept this key? ".into());
        let (_link, mut handle) = Link::connect(target(port), Box::new(policy), asker).unwrap();

        let log = tokio::task::spawn_blocking(move || {
            // The desk is polled the way the surface polls it, on the same
            // loop that drains the wire.
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            let mut asked = None;
            while asked.is_none() {
                assert!(std::time::Instant::now() < deadline, "nothing was asked");
                match desk.take() {
                    Some(ssh_link::Ask::Question(question)) => asked = Some(question),
                    Some(ssh_link::Ask::Say(_)) => {}
                    None => std::thread::sleep(Duration::from_millis(5)),
                }
            }
            let question = asked.unwrap();
            assert_eq!(question.prompt(), "accept this key? ");
            assert_eq!(question.kind(), Answer::YesNo);
            question.answer(answer.to_string());
            wait_for(&mut handle, |e| {
                matches!(e, WireEvent::Eof)
                    || matches!(e, WireEvent::Data(d) if d.windows(5).any(|w| w == b"ready"))
            })
        })
        .await
        .unwrap();

        if refused {
            assert!(text_of(&log).contains(r#"the desk said Some("no")"#), "{log:?}");
        } else {
            // Past trust and into auth: the handshake carried on, which is
            // the whole proof that the verdict came back where it was asked.
            assert!(!text_of(&log).contains("the desk said"), "{log:?}");
        }
    }
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
        consult: None,
    };
    let (_link, mut handle) =
        Link::connect(target(port), Box::new(policy), Asker::closed()).unwrap();
    tokio::task::spawn_blocking(move || wait_for(&mut handle, |e| matches!(e, WireEvent::Eof)))
        .await
        .unwrap();
    assert_eq!(
        *saw.lock().unwrap(),
        Some(Algorithm::Ecdsa { curve: ssh_key::EcdsaCurve::NistP256 }),
        "the server led with a key the policy never asked for"
    );
}

/// A committed test fixture, by name. The `.ppk` files were written once
/// by `puttygen` (putty-tools 0.83): an OpenSSH key converted with
/// `-O private` at `--ppk-param version=3` (and `version=2` for the RSA
/// one), the sealed one with `--new-passphrase` naming `atomic-cafe` and
/// `memory=1024,passes=1`: puttygen's default Argon2 cost is calibrated
/// to ~100ms of release-speed hashing, which a debug build multiplies
/// into seconds per attempt, and the lockout connection below makes four
/// load attempts against a ten-second deadline.
fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// The identity inside a fixture, read the way the transport reads it.
fn fixture_key(name: &str, passphrase: Option<&str>) -> PrivateKey {
    let text = std::fs::read_to_string(fixture(name)).unwrap();
    ssh_link::russh::keys::decode_secret_key(&text, passphrase).unwrap()
}

/// A PuTTY key file is one more format the loader reads: v3 and v2 alike
/// authenticate with nobody asked anything.
#[tokio::test(flavor = "multi_thread")]
async fn a_ppk_key_authenticates_like_any_other() {
    for name in ["ed25519_plain.ppk", "rsa_v2_plain.ppk"] {
        let identity = fixture_key(name, None);
        let (port, _seen) = serve_echo(vec![ed25519()], identity.public_key().clone()).await;
        let mut with_key = target(port);
        with_key.key_files = vec![fixture(name)];
        let (_link, mut handle) =
            Link::connect(with_key, Box::new(Scripted::verdict(Ok(()))), Asker::closed())
                .unwrap();
        let log = tokio::task::spawn_blocking(move || {
            wait_for(&mut handle, |e| {
                matches!(e, WireEvent::Data(d) if d.windows(5).any(|w| w == b"ready"))
            })
        })
        .await
        .unwrap();
        assert!(
            text_of(&log).contains("authenticated as overseer"),
            "{name}: {log:?}"
        );
        assert!(
            !text_of(&log).contains("passphrase"),
            "{name} asked for a passphrase: {log:?}"
        );
    }
}

/// An encrypted ppk is asked about like an encrypted OpenSSH key: the
/// wrong passphrase is named as such, the right one gets in, and three
/// misses leave the file locked.
#[tokio::test(flavor = "multi_thread")]
async fn an_encrypted_ppk_opens_on_its_passphrase() {
    let identity = fixture_key("ed25519_sealed.ppk", Some("atomic-cafe"));
    let (port, _seen) = serve_echo(vec![ed25519()], identity.public_key().clone()).await;

    let (asker, desk) = ssh_link::ask::desk();
    let hand = deskhand(desk, vec![Some("wrong"), Some("atomic-cafe")]);
    let mut sealed_target = target(port);
    sealed_target.key_files = vec![fixture("ed25519_sealed.ppk")];
    let (_link, mut handle) =
        Link::connect(sealed_target, Box::new(Scripted::verdict(Ok(()))), asker).unwrap();
    let log = tokio::task::spawn_blocking(move || {
        wait_for(&mut handle, |e| {
            matches!(e, WireEvent::Data(d) if d.windows(5).any(|w| w == b"ready"))
        })
    })
    .await
    .unwrap();
    let prompts = hand.join().unwrap();
    assert_eq!(prompts.len(), 2, "{prompts:?}");
    assert!(prompts[0].contains("passphrase for"), "{prompts:?}");
    assert!(prompts[0].contains("ed25519_sealed.ppk"), "{prompts:?}");
    assert!(text_of(&log).contains("that passphrase did not open"), "{log:?}");
    assert!(text_of(&log).contains("authenticated as overseer"), "{log:?}");

    // Three misses: the file is reported locked rather than asked about a
    // fourth time. The sequence then moves on; the lock line is the claim
    // under test, so the wait ends on it.
    let (asker, desk) = ssh_link::ask::desk();
    let hand = deskhand(desk, vec![Some("no"), Some("nope"), Some("never")]);
    let mut sealed_target = target(port);
    sealed_target.key_files = vec![fixture("ed25519_sealed.ppk")];
    let (_link, mut handle) =
        Link::connect(sealed_target, Box::new(Scripted::verdict(Ok(()))), asker).unwrap();
    let log = tokio::task::spawn_blocking(move || {
        wait_for(&mut handle, |e| {
            matches!(e, WireEvent::Notice(t) if t.contains("stays locked"))
        })
    })
    .await
    .unwrap();
    hand.join().unwrap();
    assert!(text_of(&log).contains("ed25519_sealed.ppk stays locked"), "{log:?}");
}

/// A ppk header on an unreadable body is an unreadable file, not a
/// passphrase question: the header says `none`, so there is nothing to ask.
#[tokio::test(flavor = "multi_thread")]
async fn a_broken_unencrypted_ppk_is_named_unreadable_with_no_question() {
    let dir = tempfile::tempdir().unwrap();
    let identity = ed25519();
    let (port, _seen) = serve_echo(vec![ed25519()], identity.public_key().clone()).await;
    let _ = identity;
    let path = dir.path().join("broken.ppk");
    std::fs::write(
        &path,
        "PuTTY-User-Key-File-3: ssh-ed25519\nEncryption: none\nComment: broken\n",
    )
    .unwrap();
    let mut with_key = target(port);
    with_key.key_files = vec![path];
    let (_link, mut handle) =
        Link::connect(with_key, Box::new(Scripted::verdict(Ok(()))), Asker::closed()).unwrap();
    let log = tokio::task::spawn_blocking(move || {
        wait_for(&mut handle, |e| {
            matches!(e, WireEvent::Notice(t) if t.contains("could not read"))
        })
    })
    .await
    .unwrap();
    assert!(!text_of(&log).contains("passphrase"), "{log:?}");
    assert!(!text_of(&log).contains("cancelled"), "{log:?}");
}
