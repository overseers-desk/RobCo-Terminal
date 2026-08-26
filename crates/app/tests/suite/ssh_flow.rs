//! The SSH bank flow end to end through the surface the binary runs:
//! `TerminalSurface::headless` dialling the in-process far side that
//! `ssh_link::test_server` serves, asserted at what the user reads on the
//! glass, the shape `tmux_flow` set. No sshd runs and none is needed.
//!
//! The environment (SSH_AUTH_SOCK) is set once here and both tests want it
//! set, so ordering between them does not matter.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use app::ssh::{KnownHosts, SshRequest};
use app::window::TerminalSurface;
use ssh_link::russh::keys::{Algorithm, PrivateKey};
use ssh_link::test_server::{mint, serve_agent, serve_echo};
use term::{viewport_text, CellSize, SessionConfig, Viewport};

/// One far side for the whole suite: an echo server whose host key the
/// fixtures record, and an agent holding the one identity it authorizes.
struct FarSide {
    /// Kept alive: the servers run on its threads.
    _rt: tokio::runtime::Runtime,
    port: u16,
    host_key_line: String,
    _dir: tempfile::TempDir,
}

fn far_side() -> &'static FarSide {
    static FAR: OnceLock<FarSide> = OnceLock::new();
    FAR.get_or_init(|| {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let dir = tempfile::tempdir().expect("scratch dir");
        let identity = mint(Algorithm::Ed25519);
        let host_key: PrivateKey = mint(Algorithm::Ed25519);
        let host_public = host_key.public_key().to_openssh().expect("openssh form");
        let (port, _seen) =
            rt.block_on(serve_echo(vec![host_key], identity.public_key().clone()));
        let sock = rt.block_on(serve_agent(dir.path(), &identity));
        std::env::set_var("SSH_AUTH_SOCK", &sock);
        FarSide {
            _rt: rt,
            port,
            host_key_line: format!("[127.0.0.1]:{port} {host_public}"),
            _dir: dir,
        }
    })
}

fn shell() -> SessionConfig {
    SessionConfig {
        program: Some("/bin/sh".into()),
        ..SessionConfig::default()
    }
}

fn surface() -> TerminalSurface {
    let viewport = Viewport::new(720, 490, 1.0, CellSize::new(9.0, 18.0));
    TerminalSurface::headless(&shell(), viewport)
}

fn request(port: u16) -> SshRequest {
    SshRequest { user: "overseer".into(), host: "127.0.0.1".into(), port }
}

fn known_hosts(lines: &[&str]) -> PathBuf {
    let dir = tempfile::tempdir().expect("scratch dir");
    let path = dir.path().join("known_hosts");
    std::fs::write(&path, format!("{}\n", lines.join("\n"))).expect("fixture");
    // Leak the dir so the file outlives this helper; a test process's
    // scratch is the OS's to sweep.
    std::mem::forget(dir);
    path
}

/// Pump until the glass satisfies `pred`; a timeout says what stood there.
fn pump_until(surface: &mut TerminalSurface, what: &str, pred: impl Fn(&TerminalSurface) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        surface.pump();
        if pred(surface) {
            return;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    panic!("timed out waiting for {what}\nglass: {:?}", surface.viewport_text());
}

fn glass_contains(surface: &TerminalSurface, word: &str) -> bool {
    surface
        .channels()
        .current()
        .map(|row| viewport_text(row.session.term()).join("\n").contains(word))
        .unwrap_or(false)
}

#[test]
fn an_ssh_bank_lives_types_and_dies_on_the_glass() {
    let far = far_side();
    let mut s = surface();
    let home = (s.channels().current_bank(), s.channels().current_channel());
    assert_eq!(home, (0, 1), "the set came up on the home shell");

    s.connect_ssh_with(
        &request(far.port),
        Box::new(KnownHosts::over(vec![known_hosts(&[&far.host_key_line])])),
    );
    // The ask takes the air: a new bank, its first channel current.
    let bank = s.channels().current_bank();
    assert_ne!(bank, 0, "the connection stands as its own bank");
    assert_eq!(s.channels().current_channel(), 1);
    assert_eq!(s.channels().current().unwrap().title, "overseer@127.0.0.1");

    // The far side's greeting reaches the glass through the ordinary pump.
    pump_until(&mut s, "the remote shell's greeting", |s| glass_contains(s, "ready"));

    // Typing goes out the wire and the echo comes home.
    s.write(b"wasteland");
    pump_until(&mut s, "the echo of what was typed", |s| glass_contains(s, "wasteland"));

    // The remote end exits: the row dies the way a shell's does, the bank
    // goes with it, and the air falls home.
    s.write(b"\x04");
    pump_until(&mut s, "the bank to be swept", |s| {
        s.channels().rows().iter().all(|r| r.bank == 0)
    });
    assert_eq!(s.channels().current_bank(), 0);
}

#[test]
fn a_refused_host_key_is_readable_until_the_user_closes_it() {
    let far = far_side();
    let mut s = surface();

    // An empty known_hosts: unknown host, refuse-by-default.
    s.connect_ssh_with(
        &request(far.port),
        Box::new(KnownHosts::over(vec![known_hosts(&[])])),
    );
    let bank = s.channels().current_bank();
    assert_ne!(bank, 0);

    // The refusal lands on the channel's own glass and the row stays: a
    // connection that never lived keeps its slot, because that slot is the
    // only place its refusal is readable.
    pump_until(&mut s, "the refusal on the glass", |s| {
        glass_contains(s, "no host key is recorded")
    });
    for _ in 0..5 {
        s.pump();
    }
    assert_eq!(s.channels().current_bank(), bank, "the dead glass is still readable");

    // The user, having read it, closes the channel: the bank goes.
    s.close_channel();
    assert!(s.channels().rows().iter().all(|r| r.bank == 0));
}
