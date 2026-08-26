//! A real tmux server on a real PTY, for the recorder and the live test.
//!
//! Not part of the crate: the codec is bytes in and bytes out, and this is the
//! plumbing that produces the bytes. It lives here because `examples/record.rs`
//! and `tests/live_tmux.rs` need the same three things (a private server, a
//! control client on a terminal, and the DCS envelope off the front), and two
//! copies of that would drift.
//!
//! tmux insists on a terminal: a control client whose stdin is a pipe dies with
//! "tcgetattr failed: Inappropriate ioctl for device" before it says anything.
//! That is why there is a PTY here at all.

#![allow(dead_code)]

// Two halves: the tmux server and its gateway live on Unix, where tmux
// does; the byte helpers at the bottom decode recorded transcripts and
// compile everywhere the transcripts replay.
#[cfg(unix)]
use std::io::{Read, Write};
#[cfg(unix)]
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::Command as OsCommand;
#[cfg(unix)]
use std::time::{Duration, Instant};

#[cfg(unix)]
use rio_vt::teletypewriter::{create_pty_with_spawn, Pty};
use tmux_cc::{Codec, Command, Event};

/// The tmux this is measured against. Recorded in the transcripts' README as
/// well: a protocol claim is only true of a version.
#[cfg(unix)]
pub const TMUX: &str = "/usr/bin/tmux";

/// A private tmux server, killed when it goes out of scope.
///
/// Private in every sense that matters to a test: its own socket under a
/// scratch directory, so nothing here can touch a real server, and nothing a
/// developer is running can answer these commands.
#[cfg(unix)]
pub struct Server {
    socket: PathBuf,
    dir: PathBuf,
}

#[cfg(unix)]
impl Server {
    /// Start a server with one detached session.
    ///
    /// `program` is what every window of the session runs. `/bin/cat` is the
    /// usual choice: it echoes, it never prints a prompt, and a prompt is a
    /// hostname and a working directory baked into a fixture.
    pub fn start(name: &str, session: &str, program: &str) -> Server {
        let dir =
            std::env::temp_dir().join(format!("robco-tmux-cc-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let server = Server {
            socket: dir.join("socket"),
            dir,
        };
        server.run(&[
            "new-session",
            "-d",
            "-s",
            session,
            "-x",
            "80",
            "-y",
            "24",
            program,
        ]);
        // Every later window runs it too. Otherwise `new-window` starts the
        // developer's login shell and its prompt writes the machine's hostname
        // and working directory into a committed fixture.
        server.run(&["set-option", "-g", "default-command", program]);
        // Automatic renaming fires whenever tmux next looks at what a pane is
        // running, which is a race against every `settle` here: a recording
        // gained or lost a `%window-renamed` run to run. Off, so that every
        // rename in a fixture is one somebody asked for.
        server.run(&["set-option", "-g", "automatic-rename", "off"]);
        server
    }

    pub fn socket(&self) -> &Path {
        &self.socket
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Run a tmux command against this server from outside the control
    /// client: the other hand, making things happen that the client must
    /// then hear about.
    pub fn run(&self, args: &[&str]) -> String {
        let out = OsCommand::new(TMUX)
            .arg("-S")
            .arg(&self.socket)
            .args(args)
            .output()
            .expect("run tmux");
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    /// The same, for arguments that are not UTF-8. A window name is bytes as
    /// far as tmux is concerned, and the interesting case is a name that is
    /// not text at all.
    pub fn run_bytes(&self, args: &[&[u8]]) -> String {
        use std::os::unix::ffi::OsStrExt;
        let out = OsCommand::new(TMUX)
            .arg("-S")
            .arg(&self.socket)
            .args(args.iter().map(|a| std::ffi::OsStr::from_bytes(a)))
            .output()
            .expect("run tmux");
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    /// Attach a control-mode client on a PTY.
    pub fn attach(&self, session: &str) -> Gateway {
        Gateway::spawn(&self.socket, session)
    }
}

#[cfg(unix)]
impl Drop for Server {
    fn drop(&mut self) {
        let _ = OsCommand::new(TMUX)
            .arg("-S")
            .arg(&self.socket)
            .arg("kill-server")
            .output();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// One `tmux -CC attach` on a PTY, with everything it has ever written kept.
#[cfg(unix)]
pub struct Gateway {
    pty: Pty,
    transcript: Vec<u8>,
    codec: Codec,
    /// The wire lines sent, in order: the recorder's sidecar, and what a
    /// decode test replays to rebuild the same pending queue.
    sent: Vec<String>,
}

#[cfg(unix)]
impl Gateway {
    fn spawn(socket: &Path, session: &str) -> Gateway {
        let args = vec![
            "-S".to_string(),
            socket.display().to_string(),
            "-CC".to_string(),
            "attach".to_string(),
            "-t".to_string(),
            session.to_string(),
        ];
        let pty = create_pty_with_spawn(
            Some(TMUX),
            args,
            &None,
            // A terminal tmux will not try to be clever about, and a locale
            // that leaves bytes alone.
            Some(vec![
                ("TERM".to_string(), "xterm-256color".to_string()),
                ("LC_ALL".to_string(), "C".to_string()),
            ]),
            80,
            24,
            0,
            0,
        )
        .expect("spawn tmux -CC on a pty");
        Gateway {
            pty,
            transcript: Vec::new(),
            codec: Codec::new(),
            sent: Vec::new(),
        }
    }

    /// Send a typed command through the codec, exactly as the appliance will.
    pub fn send(&mut self, command: &Command) {
        let sent = self.codec.send(command);
        self.sent.push(sent.wire.trim_end().to_string());
        self.pty
            .write_all(sent.wire.as_bytes())
            .expect("write command");
        let _ = self.pty.flush();
    }

    /// Send a line the codec deliberately cannot express: a probe of tmux's
    /// own lexer, or a client flag no appliance sets.
    ///
    /// Still counted in `sent`: tmux answers it with a block like any other,
    /// and a block the pairing queue does not know about throws every later
    /// reply one place wrong.
    pub fn send_raw(&mut self, line: &str) {
        self.sent.push(line.to_string());
        self.pty
            .write_all(format!("{line}\n").as_bytes())
            .expect("write raw line");
        let _ = self.pty.flush();
    }

    /// Drain whatever tmux has written into the transcript.
    pub fn pump(&mut self) -> usize {
        let mut buf = [0u8; 8192];
        let mut total = 0;
        loop {
            match self.pty.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    self.transcript.extend_from_slice(&buf[..n]);
                    total += n;
                    if n < buf.len() {
                        break;
                    }
                }
                // The master is opened non-blocking, so `WouldBlock` is "not
                // yet", never EOF -- the same rule `term::Session::pump` follows.
                Err(_) => break,
            }
        }
        total
    }

    /// Pump for a while, so whatever tmux is about to say is in the
    /// transcript before the next thing is asked of it.
    pub fn settle(&mut self, millis: u64) {
        let until = Instant::now() + Duration::from_millis(millis);
        while Instant::now() < until {
            self.pump();
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Pump until `wanted` is somewhere in the transcript, or give up.
    /// Returns whether it arrived.
    pub fn wait_for(&mut self, wanted: &[u8], millis: u64) -> bool {
        let until = Instant::now() + Duration::from_millis(millis);
        loop {
            self.pump();
            if find(&self.transcript, wanted).is_some() {
                return true;
            }
            if Instant::now() >= until {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Every byte tmux has written to this client, envelope included.
    pub fn transcript(&self) -> &[u8] {
        &self.transcript
    }

    /// The wire lines this gateway has sent, in order.
    pub fn sent(&self) -> &[String] {
        &self.sent
    }

    /// The control stream: the transcript with its DCS envelope removed.
    pub fn control_stream(&self) -> Vec<u8> {
        control_stream(&self.transcript)
    }

    /// Decode everything so far with a fresh codec, having first enqueued one
    /// pending command per line sent. Pre-enqueueing is faithful to FIFO
    /// pairing (the queue holds order, not content) and it is what lets a
    /// finished transcript be replayed at all.
    pub fn replay(&self) -> Vec<Event> {
        replay(&self.transcript, self.sent.len())
    }

    /// The pid of the tmux client process, for the tests that need it to die
    /// without warning.
    pub fn client_pid(&self) -> i32 {
        *self.pty.child.pid
    }

    /// SIGKILL the client: the gateway death of the flow script's phase 5, and
    /// the first half of issue #4's reproduction. No `%exit`, no `ST`.
    pub fn kill_client(&mut self) {
        let _ = OsCommand::new("kill")
            .args(["-9", &self.client_pid().to_string()])
            .output();
        self.settle(300);
    }
}

/// Strip the `ESC P 1000 p` ... `ESC \` envelope tmux wraps its control stream
/// in.
///
/// The shipping path does not use this: the gateway is a terminal session and
/// its VT parser hands the DCS body over already peeled (`term::dcs`). A
/// transcript is raw PTY bytes, so the fixtures need it.
pub fn control_stream(transcript: &[u8]) -> Vec<u8> {
    let Some(start) = find(transcript, b"\x1bP1000p") else {
        return Vec::new();
    };
    let body = &transcript[start + 7..];
    match find(body, b"\x1b\\") {
        Some(end) => body[..end].to_vec(),
        None => body.to_vec(),
    }
}

/// Whether the envelope was closed properly (`ST` seen), which is how a
/// well-behaved tmux says the protocol is over.
pub fn envelope_closed(transcript: &[u8]) -> bool {
    match find(transcript, b"\x1bP1000p") {
        Some(start) => find(&transcript[start..], b"\x1b\\").is_some(),
        None => false,
    }
}

/// Decode a whole transcript with `commands` commands assumed sent.
pub fn replay(transcript: &[u8], commands: usize) -> Vec<Event> {
    let mut codec = Codec::new();
    for _ in 0..commands {
        // Content-free on purpose: the queue holds order, not commands.
        codec.send(&Command::NewWindow);
    }
    codec.feed(&control_stream(transcript))
}

pub fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}
