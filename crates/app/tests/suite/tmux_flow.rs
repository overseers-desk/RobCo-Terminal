//! The tmux -CC page flow against a live tmux, end to end through the surface
//! the binary runs: `scripts/eval/tmux-flow.sh`'s phases, minus the pixels.
//!
//! The shell script drives the appliance's own UI with xdotool and judges
//! the glass; what
//! it is *really* asserting (attach transport, per-window channels, output
//! routing, the page's lifetime, detach and gateway death) lives below the
//! pixels, in the model these tests reach directly. `TerminalSurface::headless`
//! is the same surface the window runs, pump and all, so the whole chain is
//! under test: a real `tmux -CC` client on the channel's own PTY, the DCS tap
//! peeling the envelope, the codec, the gateway, and `app::channels`.
//!
//! Phases 8 to 11 are the interaction layer's, and they are the same shape one
//! level up: what the user does with the attachment (typing at the anchor, a
//! special key on its way to a far pane, a remote channel's close) asserted
//! against what tmux and the model then hold.
//!
//! Phase 7 is the killed-client re-attach reproduction: kill the whole appliance (not
//! a detach) over a session carrying several windows, re-attach, and every
//! pre-existing window must get a channel.
//!
//! Needs `/usr/bin/tmux` (3.5a is what the codec was measured against) and a
//! few seconds; skips without it, like `tmux-cc`'s live tests.

use std::path::{Path, PathBuf};
use std::process::Command as OsCommand;
use std::time::{Duration, Instant};

use app::channels::ChannelKind;
use app::window::TerminalSurface;
use term::{viewport_text, CellSize, SessionConfig, Viewport};
use winit::keyboard::{Key, ModifiersState, NamedKey};

const TMUX: &str = "/usr/bin/tmux";

fn have_tmux() -> bool {
    let ok = Path::new(TMUX).exists();
    if !ok {
        eprintln!("skipping: no {TMUX}");
    }
    ok
}

/// A private tmux server, its socket under a scratch, killed on drop.
struct Server {
    socket: PathBuf,
    _dir: tempfile::TempDir,
}

impl Server {
    /// One detached session named `one`, every pane running `/bin/cat`: it
    /// echoes, it never prints a prompt, and automatic renaming is off so
    /// every name in an assertion is one somebody asked for.
    fn start() -> Server {
        let dir = tempfile::tempdir().expect("scratch dir");
        let server = Server {
            socket: dir.path().join("socket"),
            _dir: dir,
        };
        server.run(&[
            "new-session",
            "-d",
            "-s",
            "one",
            "-x",
            "80",
            "-y",
            "24",
            "/bin/cat",
        ]);
        server.run(&["set-option", "-g", "default-command", "/bin/cat"]);
        server.run(&["set-option", "-g", "automatic-rename", "off"]);
        server
    }

    /// The other hand: make things happen on the server that the client must
    /// then hear about.
    fn run(&self, args: &[&str]) -> String {
        let out = OsCommand::new(TMUX)
            .arg("-S")
            .arg(&self.socket)
            .args(args)
            .output()
            .expect("run tmux");
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn run_bytes(&self, args: &[&[u8]]) -> String {
        use std::os::unix::ffi::OsStrExt;
        let out = OsCommand::new(TMUX)
            .arg("-S")
            .arg(&self.socket)
            .args(args.iter().map(|a| std::ffi::OsStr::from_bytes(a)))
            .output()
            .expect("run tmux");
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn windows(&self) -> Vec<String> {
        self.run(&["list-windows", "-t", "one", "-F", "#{window_id}"])
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn clients(&self) -> usize {
        self.run(&["list-clients", "-F", "#{client_pid}"])
            .lines()
            .count()
    }

    /// Put `bytes` into a pane's `cat`, which echoes them onto the pane's own
    /// screen. `-H` because the interesting bytes are escapes and `send-keys`
    /// would otherwise read them as key names.
    fn send_raw(&self, target: &str, bytes: &[u8]) {
        let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let mut args = vec!["send-keys", "-H", "-t", target];
        args.extend(hex.iter().map(String::as_str));
        self.run(&args);
    }

    /// What tmux's own emulator holds for a pane, with its attributes turned
    /// back into escape sequences: the very thing the gateway's
    /// `capture-pane -peqJ` reply carries on attach.
    fn captured(&self, target: &str) -> String {
        self.run(&["capture-pane", "-p", "-e", "-t", target])
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = OsCommand::new(TMUX)
            .arg("-S")
            .arg(&self.socket)
            .arg("kill-server")
            .output();
    }
}

/// The channel's shell: an interactive bash the test types the attach into,
/// exactly the flow script's posture. `--norc` so the developer's prompt
/// hooks stay out of the transcript.
fn shell() -> SessionConfig {
    SessionConfig {
        program: Some("/bin/bash".to_string()),
        args: vec!["--norc".to_string(), "--noprofile".to_string()],
        working_directory: None,
        env: vec![("TERM".to_string(), "xterm-256color".to_string())],
        scrollback: 500,
    }
}

fn surface() -> TerminalSurface {
    // A tmux client refuses to attach from inside another tmux; the test
    // must not inherit a developer session's TMUX into the shell.
    std::env::remove_var("TMUX");
    let viewport = Viewport::new(720, 490, 1.0, CellSize::new(9.0, 18.0));
    TerminalSurface::headless(&shell(), viewport)
}

/// Type the attach into the channel on the air.
fn type_attach(surface: &mut TerminalSurface, server: &Server) {
    let line = format!("{TMUX} -S {} -CC attach -t one\r", server.socket.display());
    surface.write(line.as_bytes());
}

/// The surface's channel rows, flattened for a failure message.
fn rows_of(surface: &TerminalSurface) -> String {
    surface
        .channels()
        .rows()
        .iter()
        .map(|r| {
            format!(
                "(page {} ch {} {:?} win {:?} pane {:?} title {:?})",
                r.page, r.channel, r.kind, r.window_id, r.pane_id, r.title
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Pump until the model satisfies `pred`; a timeout is a failure that says
/// what the model held instead.
fn pump_until(surface: &mut TerminalSurface, what: &str, pred: impl Fn(&TerminalSurface) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        surface.pump();
        if pred(surface) {
            return;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    panic!(
        "timed out waiting for {what}\nrows: {}\nglass: {:?}",
        rows_of(surface),
        surface.viewport_text()
    );
}

/// Attach and wait until the page stands with a channel per existing window.
/// Answers the tmux page's id.
fn attach(surface: &mut TerminalSurface, server: &Server) -> u32 {
    let windows = server.windows().len();
    type_attach(surface, server);
    pump_until(surface, "the attach to raise its page and channels", |s| {
        s.channels().pages().len() == 2
            && s.channels()
                .rows()
                .iter()
                .filter(|r| r.kind == ChannelKind::Remote)
                .count()
                == windows
    });
    surface
        .channels()
        .pages()
        .iter()
        .find(|p| p.kind == app::channels::PageKind::Tmux)
        .expect("a tmux page")
        .id
}

/// One key press, as the window's own `key_pressed` would reduce it.
///
/// `text` is the text the platform produced with **every** modifier applied --
/// winit's `text_with_all_modifiers`, which the window reads through
/// `app::window::key_text`. So `Ctrl+C` arrives here as `Some("\u{3}")`, the
/// way X11 hands it over, and not as the bare `"c"` of winit's plain `text`
/// field.
fn press(surface: &mut TerminalSurface, key: Key, text: Option<&str>, mods: ModifiersState) {
    surface.key_input(&key, text, mods);
}

fn typed(surface: &mut TerminalSurface, letter: &str) {
    press(
        surface,
        Key::Character(letter.into()),
        Some(letter),
        ModifiersState::empty(),
    );
}

/// The short host name tmux reports, which is what the anchor is named for.
fn host() -> String {
    String::from_utf8_lossy(
        &OsCommand::new("hostname")
            .arg("-s")
            .output()
            .unwrap()
            .stdout,
    )
    .trim()
    .to_string()
}

/// The row a window id landed on.
fn row_of<'a>(
    surface: &'a TerminalSurface,
    page: u32,
    window: &str,
) -> Option<&'a app::channels::Row<app::window::AppSession>> {
    surface
        .channels()
        .rows()
        .iter()
        .find(|r| r.page == page && r.window_id == window)
}

#[test]
fn phase_1_a_fresh_attach_populates_a_channel_for_every_window_and_the_first_greets() {
    if !have_tmux() {
        return;
    }
    let server = Server::start();
    server.run(&["new-window", "-t", "one", "-n", "logs"]);
    assert_eq!(server.windows().len(), 2, "the session starts with two");

    // Colour the page before anybody attaches to it.
    //
    // `capture-pane -peqJ` reports a pane's attributes as the escape
    // sequences that produce them, so the reply that draws this window's
    // screen on attach is full of raw `ESC [ ... m` -- inside the control
    // mode DCS envelope, where a VT parser reads the first `ESC` as the end
    // of the string. Under that reading the attachment dies here, before a
    // single channel stands, which is why this sits in phase 1 rather than
    // in a phase of its own.
    let windows = server.windows();
    server.send_raw(&windows[0], b"\x1b[31mRED-PROMPT\x1b[0m $ \r\n");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !server.captured(&windows[0]).contains("RED-PROMPT") {
        assert!(
            Instant::now() < deadline,
            "the pane never took the coloured text"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        server.captured(&windows[0]).contains('\x1b'),
        "capture-pane -e reported no escape sequences; the scenario is vacuous"
    );

    let mut surface = surface();
    let page = attach(&mut surface, &server);

    // The coloured page came across the envelope and onto the channel's own
    // glass: every byte of that capture survived the DCS tap.
    pump_until(&mut surface, "the coloured capture to land", |s| {
        row_of(s, page, &windows[0])
            .map(|r| {
                viewport_text(r.session.term())
                    .join("\n")
                    .contains("RED-PROMPT")
            })
            .unwrap_or(false)
    });

    // Every pre-existing window has a channel, behind the anchor at slot 1.
    let rows = surface.channels().rows();
    let anchor = rows
        .iter()
        .find(|r| r.kind == ChannelKind::Anchor)
        .expect("an anchor");
    assert_eq!((anchor.page, anchor.channel), (page, 1));
    for (slot, window) in server.windows().iter().enumerate() {
        let row = row_of(&surface, page, window).unwrap_or_else(|| {
            panic!(
                "window {window} got no channel; rows: {}",
                rows_of(&surface)
            )
        });
        assert_eq!(row.channel as usize, slot + 2, "windows fill from slot 2");
    }

    // The greets rule: the attach's first window takes the air; the ones tmux
    // volunteers after it do not.
    assert_eq!(
        (
            surface.channels().current_page(),
            surface.channels().current_channel()
        ),
        (page, 2)
    );

    // The anchor is titled for the machine the session is on.
    let host = host();
    pump_until(&mut surface, "the host to resolve", |s| {
        s.channels()
            .slot_title(page, 1)
            .is_some_and(|t| t == format!("tmux -CC # @{host}"))
    });

    // The client-size law, read off its real effect: tmux draws the
    // session's windows at the size the client published. (The session was
    // created 80x24; the published grid differs, so a window standing at it
    // proves the `refresh-client -C` arrived. `#{client_height}` itself
    // reads empty for a control client on tmux 3.5a, so the client's row is
    // no instrument.)
    let size = Viewport::new(720, 490, 1.0, CellSize::new(9.0, 18.0)).term_size();
    let want = format!("{}x{}", size.cols(), size.rows());
    assert_ne!(want, "80x24", "the probe must differ from the seed size");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let got = server.run(&["list-windows", "-F", "#{window_width}x#{window_height}"]);
        if got.lines().all(|l| l.trim() == want) && !got.trim().is_empty() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "tmux never drew the windows at the glass's grid: wanted {want}, windows say {got:?}"
        );
        surface.pump();
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn phase_2_new_windows_arrive_volunteered_stand_back_and_asked_for_take_the_air() {
    if !have_tmux() {
        return;
    }
    let server = Server::start();
    let mut surface = surface();
    let page = attach(&mut surface, &server);
    assert_eq!(
        (
            surface.channels().current_page(),
            surface.channels().current_channel()
        ),
        (page, 2)
    );

    // A window somebody else opens lines up without taking the air.
    server.run(&["new-window", "-t", "one", "-n", "fresh"]);
    pump_until(&mut surface, "the volunteered window's channel", |s| {
        s.channels().slot_title(page, 3) == Some("fresh")
    });
    assert_eq!(surface.channels().current_channel(), 2, "the air held");

    // A window this set asks for takes it outright (Ctrl+Shift+T's path).
    surface.new_channel();
    pump_until(&mut surface, "the asked-for window to take the air", |s| {
        s.channels().current_channel() == 4
    });
    assert_eq!(server.windows().len(), 3);
}

#[test]
fn phase_3_a_rename_lands_unescaped_in_the_bank() {
    if !have_tmux() {
        return;
    }
    let server = Server::start();
    let mut surface = surface();
    let page = attach(&mut surface, &server);
    let window = server.windows()[0].clone();

    // A name carrying a backslash arrives vis(3)-escaped on the wire; the
    // codec decodes it, so the bank shows the name itself, not the escaped
    // `a\\b` a verbatim read would show.
    server.run_bytes(&[b"rename-window", b"-t", window.as_bytes(), b"a\\b"]);
    pump_until(&mut surface, "the unescaped rename", |s| {
        s.channels().slot_title(page, 2) == Some(r"a\b")
    });
}

#[test]
fn phase_4_output_flows_to_the_channel_of_its_pane_and_keystrokes_divert_back() {
    if !have_tmux() {
        return;
    }
    let server = Server::start();
    server.run(&["new-window", "-t", "one", "-n", "logs"]);
    let mut surface = surface();
    let page = attach(&mut surface, &server);
    let windows = server.windows();

    // Words typed into each pane from the server side must surface on that
    // window's channel and no other.
    server.run(&["send-keys", "-t", &windows[0], "word-zero", "Enter"]);
    server.run(&["send-keys", "-t", &windows[1], "word-one", "Enter"]);
    pump_until(&mut surface, "both panes' output to route", |s| {
        let sees = |window: &str, word: &str| {
            row_of(s, page, window)
                .map(|r| viewport_text(r.session.term()).join("\n").contains(word))
                .unwrap_or(false)
        };
        sees(&windows[0], "word-zero") && sees(&windows[1], "word-one")
    });
    let crossed = |s: &TerminalSurface, window: &str, word: &str| {
        row_of(s, page, window)
            .map(|r| viewport_text(r.session.term()).join("\n").contains(word))
            .unwrap_or(false)
    };
    assert!(
        !crossed(&surface, &windows[0], "word-one"),
        "window {}'s output leaked into {}",
        windows[1],
        windows[0]
    );
    assert!(!crossed(&surface, &windows[1], "word-zero"));

    // The write side: keystrokes on the remote channel on the air become
    // send-keys, land in the pane's cat, and echo back as %output.
    assert_eq!(surface.channels().current_channel(), 2);
    surface.write(b"typed-into-tmux\r");
    pump_until(&mut surface, "the diverted keystrokes to echo back", |s| {
        viewport_text(s.channels().current().unwrap().session.term())
            .join("\n")
            .contains("typed-into-tmux")
    });
    assert!(
        !crossed(&surface, &windows[1], "typed-into-tmux"),
        "keystrokes for one pane reached another"
    );
}

#[test]
fn phase_5_a_killed_window_loses_its_channel_and_the_page_stands() {
    if !have_tmux() {
        return;
    }
    let server = Server::start();
    server.run(&["new-window", "-t", "one", "-n", "logs"]);
    server.run(&["new-window", "-t", "one", "-n", "spare"]);
    let mut surface = surface();
    let page = attach(&mut surface, &server);
    let doomed = server.windows()[1].clone();

    server.run(&["kill-window", "-t", &doomed]);
    pump_until(&mut surface, "the killed window's channel to go", |s| {
        row_of(s, page, &doomed).is_none()
    });
    // The page and its other channels stand; the air never left slot 2.
    assert_eq!(surface.channels().pages().len(), 2);
    assert_eq!(surface.channels().current_channel(), 2);
    assert_eq!(
        surface
            .channels()
            .rows()
            .iter()
            .filter(|r| r.page == page)
            .count(),
        3,
        "anchor and two survivors: {}",
        rows_of(&surface)
    );
}

#[test]
fn phase_6_detach_brings_the_anchor_home_as_the_local_shell_it_never_stopped_being() {
    if !have_tmux() {
        return;
    }
    let server = Server::start();
    let mut surface = surface();
    let page = attach(&mut surface, &server);

    // Onto the anchor, then close it: an anchor's close is a detach, and the
    // row comes home when %exit echoes back.
    surface.cycle_channel(-1);
    assert_eq!(
        (
            surface.channels().current_page(),
            surface.channels().current_channel()
        ),
        (page, 1)
    );
    surface.close_channel();
    pump_until(&mut surface, "the page to collapse home", |s| {
        s.channels().pages().len() == 1 && s.channels().current_page() == 0
    });
    assert_eq!(surface.channels().current_channel(), 1, "the held slot");
    assert_eq!(
        surface.channels().current().unwrap().kind,
        ChannelKind::Local
    );
    assert_eq!(server.clients(), 0, "tmux let the client go");
    assert!(
        !server.windows().is_empty(),
        "tmux kept the session and its windows"
    );
}

/// The killed-client re-attach phase, its required reproduction: kill the whole
/// appliance (a crash, not a detach; the tmux server and its windows
/// survive), then attach from a fresh appliance. Every window the session
/// already holds must get a channel, the same as if it had been opened from
/// this client. The failure mode this guards against is a listener race that
/// drops previously open windows from the bank on reattach; that race needs
/// an asynchronous step between attach and listener registration, which this
/// architecture's synchronous attach does not have, so this test is the
/// proof that the re-attach keeps every window on the bank.
#[test]
fn phase_7_issue_4_reattach_after_appliance_death_restores_every_window() {
    if !have_tmux() {
        return;
    }
    let server = Server::start();
    let mut first = surface();
    let page = attach(&mut first, &server);
    server.run(&["new-window", "-t", "one", "-n", "second"]);
    server.run(&["new-window", "-t", "one", "-n", "third"]);
    pump_until(&mut first, "three windows on the first appliance", |s| {
        s.channels()
            .rows()
            .iter()
            .filter(|r| r.page == page && r.kind == ChannelKind::Remote)
            .count()
            == 3
    });

    // The kill. Dropping the surface closes the PTY master: SIGHUP takes the
    // shell and the tmux client with it, mid-protocol, no detach-client and
    // no %exit ever sent.
    drop(first);
    let deadline = Instant::now() + Duration::from_secs(10);
    while server.clients() > 0 {
        assert!(
            Instant::now() < deadline,
            "the killed appliance's client never left the server's books"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let windows = server.windows();
    assert_eq!(windows.len(), 3, "the session survived with its windows");

    // The re-attach, from a cold appliance.
    let mut second = surface();
    let page = attach(&mut second, &server);
    for window in &windows {
        assert!(
            row_of(&second, page, window).is_some(),
            "window {window} got no channel on re-attach; rows: {}",
            rows_of(&second)
        );
    }
    // And they are the right channels: names as the server holds them.
    pump_until(&mut second, "the restored titles", |s| {
        s.channels().slot_title(page, 3) == Some("second")
            && s.channels().slot_title(page, 4) == Some("third")
    });
    assert_eq!(
        (
            second.channels().current_page(),
            second.channels().current_channel()
        ),
        (page, 2),
        "the first restored window greets, as a fresh attach's would"
    );
}

// ---- the interaction phases ----------------------------------------------

/// The anchor's keyboard. Two claims, and the first is what makes the second
/// safe to make: while a channel is an anchor its pty is the protocol's
/// wire, so everything typed at it is accepted and dropped, and the codec,
/// which pairs every reply to a command it sent, is still in step
/// afterwards. The one key with a meaning is the bare Enter: tmux itself
/// reads an empty line as "detach", and this build sends the equivalent
/// `detach-client` command in the codec's own voice.
#[test]
fn phase_8_the_anchor_swallows_what_is_typed_at_it_and_the_bare_enter_detaches() {
    if !have_tmux() {
        return;
    }
    let server = Server::start();
    server.run(&["new-window", "-t", "one", "-n", "logs"]);
    let mut surface = surface();
    let page = attach(&mut surface, &server);

    // Onto the anchor, and type a tmux command at it, whole: eleven letters
    // and a keytab key. The command is a real one with a visible effect, so
    // "nothing reached the wire" is a claim tmux can be asked to confirm rather
    // than one this test would have to take on trust.
    surface.cycle_channel(-1);
    assert_eq!(
        (
            surface.channels().current_page(),
            surface.channels().current_channel()
        ),
        (page, 1)
    );
    for letter in ["n", "e", "w", "-", "w", "i", "n", "d", "o", "w"] {
        typed(&mut surface, letter);
    }
    press(
        &mut surface,
        Key::Named(NamedKey::Tab),
        Some("\t"),
        ModifiersState::empty(),
    );
    // And the byte path a paste or a mouse report takes, which is the other
    // way onto that wire and swallowed for the same reason.
    surface.write(b"new-window\r");
    for _ in 0..5 {
        surface.pump();
        std::thread::sleep(Duration::from_millis(20));
    }

    // Nothing typed reached tmux: the session stands, the client stands, and
    // the page stands.
    assert_eq!(
        server.clients(),
        1,
        "the anchor's keys never asked to leave"
    );
    assert_eq!(surface.channels().pages().len(), 2);
    assert_eq!(
        server.windows().len(),
        2,
        "a command typed at the anchor was run: {:?}",
        server.windows()
    );

    // And the codec is still in step. This is the assertion the swallow exists
    // for: a stray byte on that wire makes tmux answer a command nobody sent,
    // and every reply after it lands on the wrong intent. Asking for a window
    // exercises the whole pairing chain (new-window, the %window-add
    // re-listing, the capture and the cursor) and the answer has to arrive on
    // the right channel with the right name.
    surface.new_channel();
    pump_until(&mut surface, "a window asked for after the noise", |s| {
        s.channels()
            .rows()
            .iter()
            .filter(|r| r.page == page && r.kind == ChannelKind::Remote)
            .count()
            == 3
    });
    let fresh = server.windows()[2].clone();
    pump_until(&mut surface, "the fresh window's title", |s| {
        row_of(s, page, &fresh).is_some_and(|r| !r.title.is_empty())
    });

    // Now the one key that means something. Back onto the anchor, bare Enter.
    while surface.channels().current_channel() != 1 {
        surface.cycle_channel(-1);
    }
    assert_eq!(
        surface.channels().current().unwrap().kind,
        ChannelKind::Anchor
    );
    press(
        &mut surface,
        Key::Named(NamedKey::Enter),
        Some("\r"),
        ModifiersState::empty(),
    );
    pump_until(&mut surface, "the bare Enter to detach the page", |s| {
        s.channels().pages().len() == 1 && s.channels().current_page() == 0
    });
    assert_eq!(
        surface.channels().current().unwrap().kind,
        ChannelKind::Local,
        "the anchor came home as the shell it never stopped being"
    );
    assert_eq!(server.clients(), 0, "tmux let the client go");
    assert!(!server.windows().is_empty(), "and kept the session");
}

/// Keystroke diversion, completely: a keytab key and a control key both reach
/// the far pane as the bytes they encode to.
///
/// Keystroke diversion translates nothing: `send_keys` sends
/// `send-keys -H -t <pane> <hex byte>...`, one hex pair per byte, chunked at
/// 256, so there is no key-name table and no key that cannot be sent. The
/// instrument is the pane itself:
/// `cat -v` under `stty raw -echo` prints what it reads, `^[[A` for the arrow
/// and `^C` for the interrupt, and it prints it because the byte arrived, not
/// because a terminal interpreted it.
#[test]
fn phase_10_a_keytab_key_and_a_control_key_reach_the_remote_pane_as_bytes() {
    if !have_tmux() {
        return;
    }
    let server = Server::start();
    // Raw so the line discipline neither echoes nor eats the interrupt: what
    // the pane shows is what `send-keys` delivered.
    server.run(&[
        "new-window",
        "-t",
        "one",
        "-n",
        "keys",
        "stty raw -echo; exec cat -v",
    ]);
    let mut surface = surface();
    let page = attach(&mut surface, &server);
    let window = server.windows()[1].clone();

    // The air onto that window's channel (windows fill from slot 2).
    surface.cycle_channel(1);
    assert_eq!(surface.channels().current_channel(), 3);
    assert_eq!(
        surface.channels().current().unwrap().window_id,
        window,
        "the air is on the raw pane's channel"
    );

    let shows = |s: &TerminalSurface, what: &str| {
        row_of(s, page, &window)
            .map(|r| viewport_text(r.session.term()).join("\n").contains(what))
            .unwrap_or(false)
    };

    // A keytab key: `key Up -AnyMod -AppCuKeys : "\E[A"`.
    press(
        &mut surface,
        Key::Named(NamedKey::ArrowUp),
        None,
        ModifiersState::empty(),
    );
    pump_until(&mut surface, "the arrow to reach the pane", |s| {
        shows(s, "^[[A")
    });

    // A control key, which the keytab does not bind: it rides the event's
    // text with every modifier applied.
    press(
        &mut surface,
        Key::Character("c".into()),
        Some("\u{3}"),
        ModifiersState::CONTROL,
    );
    pump_until(&mut surface, "the interrupt to reach the pane", |s| {
        shows(s, "^C")
    });

    // And neither key crossed into the other window's channel.
    let other = server.windows()[0].clone();
    let crossed = row_of(&surface, page, &other)
        .map(|r| viewport_text(r.session.term()).join("\n"))
        .unwrap_or_default();
    assert!(!crossed.contains("^[[A") && !crossed.contains("^C"));
}

/// Closing a remote channel: the close asks tmux to kill the window and asks
/// the user nothing (there is no confirmation anywhere in the appliance),
/// and the row stays until `%window-close` comes back. The row is the
/// picture of what tmux holds, so it goes when tmux says so and not when the
/// user asks.
#[test]
fn phase_11_a_remote_close_asks_tmux_without_confirming_and_the_row_waits_for_the_answer() {
    if !have_tmux() {
        return;
    }
    let server = Server::start();
    server.run(&["new-window", "-t", "one", "-n", "logs"]);
    let mut surface = surface();
    let page = attach(&mut surface, &server);
    let doomed = server.windows()[0].clone();

    assert_eq!(surface.channels().current_channel(), 2);
    surface.close_channel();
    // Nothing was asked and nothing was removed: the close is a request on the
    // wire, and the row is still standing the instant after it.
    assert!(
        row_of(&surface, page, &doomed).is_some(),
        "the row went before tmux answered; rows: {}",
        rows_of(&surface)
    );

    pump_until(&mut surface, "tmux's own close to take the row", |s| {
        row_of(s, page, &doomed).is_none()
    });
    assert_eq!(server.windows().len(), 1, "tmux killed the window");
    assert_eq!(surface.channels().pages().len(), 2, "the page stands");
}

/// A paste at scale onto a remote channel, which is the flow that finds every
/// place the transport is assumed rather than checked.
///
/// Tens of kilobytes leave as hex `send-keys` lines, three wire bytes per
/// pasted byte, at 256 bytes a line: a hundred-odd command lines arriving at
/// an `O_NONBLOCK` PTY master faster than tmux reads it. A `write_all` there
/// takes a prefix and reports the refusal, and the tail of that line is gone:
/// tmux reads the stump joined to the next command, runs one command where two
/// were sent, and answers one block where two were expected, after which
/// every reply lands on the intent of the command before it, for good.
///
/// So both halves are asserted. The pane's own record of what it received is a
/// file it writes everything to, compared byte for byte with what was pasted;
/// and the pairing is asked to prove itself afterwards, by a command whose
/// reply has to come back on the right intent for a window to appear at all.
#[test]
fn phase_12_a_paste_at_scale_reaches_the_remote_pane_whole_and_the_pairing_holds() {
    if !have_tmux() {
        return;
    }
    let server = Server::start();
    // The instrument: a pane with no line discipline in the way, writing
    // every byte it is handed to a file. `stty raw -echo` is what makes the
    // file a faithful record: no echo doubling it, no CR/LF translation
    // changing its length.
    let sink = server.socket.with_file_name("paste-sink");
    server.run(&[
        "new-window",
        "-t",
        "one",
        "-n",
        "sink",
        &format!("stty raw -echo; exec cat > {}", sink.display()),
    ]);
    let mut surface = surface();
    let page = attach(&mut surface, &server);
    let window = server.windows()[1].clone();

    // The air onto the sink's channel; a paste goes to the channel on the air.
    surface.cycle_channel(1);
    assert_eq!(
        surface.channels().current().unwrap().window_id,
        window,
        "the air is on the sink pane's channel"
    );

    // Thirty-two kilobytes in one write, which is what a paste is. No
    // newlines: nothing in it can be delivered early by a line discipline, so
    // the whole thing has to survive the queue to arrive at all.
    const PASTE: usize = 32 * 1024;
    let paste: Vec<u8> = (0..PASTE).map(|i| b'a' + (i % 26) as u8).collect();
    surface.write(&paste);

    let want = paste.clone();
    let path = sink.clone();
    pump_until(
        &mut surface,
        "the paste to reach the pane whole",
        move |_| {
            std::fs::read(&path)
                .map(|got| got.len() >= want.len())
                .unwrap_or(false)
        },
    );
    let got = std::fs::read(&sink).expect("the pane's record");
    assert_eq!(
        got.len(),
        paste.len(),
        "the pane received a different count"
    );
    assert_eq!(got, paste, "the pane received different bytes");

    // And the pairing: a window asked for after all that traffic still has to
    // come back as a channel with a name, which walks the whole chain --
    // new-window, the %window-add re-listing, the capture and the cursor --
    // with every reply landing on the intent that was registered for it.
    surface.new_channel();
    pump_until(&mut surface, "a window asked for after the paste", |s| {
        s.channels()
            .rows()
            .iter()
            .filter(|r| r.page == page && r.kind == ChannelKind::Remote)
            .count()
            == 3
    });
    let fresh = server.windows()[2].clone();
    pump_until(&mut surface, "the fresh window's title", |s| {
        row_of(s, page, &fresh).is_some_and(|r| !r.title.is_empty())
    });
}

/// A tmux window's output asks for the redraw that shows it.
///
/// A remote row's own `pump` reads nothing and cannot: its bytes arrive off
/// the anchor's wire, which the surface drains after the loop that counts what
/// the channel on the air produced. Counted only there, `pump` answered zero
/// for a tmux window however much it printed, so `Tick.redraw` was left
/// entirely to the effects clock -- the picture arriving on the next effects
/// frame instead of the next frame, and not at all on a window whose effects
/// are not running.
#[test]
fn phase_13_a_remote_panes_output_is_counted_as_the_redraw_it_asks_for() {
    if !have_tmux() {
        return;
    }
    let server = Server::start();
    let mut surface = surface();
    let page = attach(&mut surface, &server);
    let windows = server.windows();

    // The air is on the attachment's first window, which is the pane about to
    // print.
    assert_eq!(surface.channels().current_channel(), 2);
    assert!(row_of(&surface, page, &windows[0]).is_some());

    // Drain what the attach itself is still delivering, so what is counted
    // below can only be the output asked for here.
    for _ in 0..20 {
        surface.pump();
        std::thread::sleep(Duration::from_millis(15));
    }

    server.run(&["send-keys", "-t", &windows[0], "printf REDRAW-ME", "Enter"]);

    let mut counted = 0usize;
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        counted += surface.pump();
        let arrived = viewport_text(surface.channels().current().unwrap().session.term())
            .join("\n")
            .contains("REDRAW-ME");
        if arrived {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the pane's output never reached the channel on the air"
        );
        std::thread::sleep(Duration::from_millis(15));
    }

    assert!(
        counted > 0,
        "the output reached the glass without one pump reporting a byte, so \
         nothing but the effects clock would have drawn it"
    );
}
