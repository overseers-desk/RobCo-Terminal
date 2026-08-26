//! Single-instance arbitration and the new-window IPC.
//!
//! A second launch of the same binary does not open a second process: it
//! hands `"new-window"` (optionally `"new-window --fullscreen"`) to the
//! running one and exits 0. If no primary answers, it warns and runs
//! standalone.
//!
//! ## Mechanism
//!
//! A **Unix domain socket in `$XDG_RUNTIME_DIR`, guarded by an `flock`ed
//! lock file next to it**. That is the mechanism the XDG base-directory spec
//! exists to serve: the directory is per-user, mode 0700, and cleared at
//! logout, so a stale socket cannot outlive the session that made it. D-Bus
//! name ownership is the other conventional answer and is rejected here: it
//! needs a session bus, which a bare X session, an SSH login, or the
//! harness's Xvfb run may not have, and the contract (item 5) requires the
//! whole thing to be redirectable by environment variable for isolated
//! scratch runs.
//!
//! The lock file, not the socket's existence, decides who is primary: a
//! socket file left behind by a killed process is indistinguishable from a
//! live one until you try to connect, whereas an `flock` is released by
//! the kernel when the holder dies, whatever killed it. So: take the lock
//! or you are not primary; if you are not primary, connect and hand over.
//!
//! ## Wire format
//!
//! One line, `\n`-terminated, so a shell one-liner can drive it directly:
//!
//! ```text
//! new-window
//! new-window --fullscreen
//! ```
//!
//! The primary answers `ok\n`, which is what lets the sender exit only
//! after the request has actually been taken.
//!
//! ## Isolation
//!
//! Every path derives from `$XDG_RUNTIME_DIR` (falling back to `$TMPDIR`,
//! then `/tmp`) and carries the uid, so two users on one machine, and two
//! scratch runs under different `XDG_RUNTIME_DIR`s, never collide --
//! contract item 5.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The request a second launch sends to the running one.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NewWindow {
    pub fullscreen: bool,
    /// `--ssh [user@]host[:port]`: the window dials this instead of opening
    /// a shell. On the wire so a second `--ssh` invocation's handoff opens
    /// the window that was asked for, not a shell that happens to be local.
    pub ssh: Option<String>,
}

impl NewWindow {
    /// The wire line, without its terminator.
    pub fn encode(&self) -> String {
        let mut line = "new-window".to_string();
        if self.fullscreen {
            line.push_str(" --fullscreen");
        }
        if let Some(ssh) = &self.ssh {
            line.push_str(" --ssh ");
            line.push_str(ssh);
        }
        line
    }

    /// Parses a wire line. An *empty* message and anything starting with
    /// `new-window` are both new-window requests, and an unknown token is
    /// ignored; that keeps a future message type from being read as a
    /// window request by an older binary.
    pub fn decode(line: &str) -> Option<Self> {
        let line = line.trim_end_matches(['\n', '\r']);
        if !(line.is_empty() || line.starts_with("new-window")) {
            return None;
        }
        let mut tokens = line.split_whitespace().skip(1);
        let mut request = NewWindow::default();
        while let Some(token) = tokens.next() {
            match token {
                "--fullscreen" => request.fullscreen = true,
                "--ssh" => request.ssh = tokens.next().map(str::to_string),
                _ => {}
            }
        }
        Some(request)
    }
}

/// What `acquire` decided this process is.
pub enum Role {
    /// This process owns the instance lock and must serve requests.
    Primary(Primary),
    /// A running primary accepted our request; the caller exits 0.
    Delivered,
    /// No primary answered and we could not become one: "primary not
    /// reachable, continuing as independent instance."
    Independent,
}

/// The primary instance's server side. Dropping it releases the lock and
/// removes the socket.
pub struct Primary {
    listener: Option<UnixListener>,
    socket_path: PathBuf,
    lock_path: PathBuf,
    _lock: std::fs::File,
}

/// Where this identity's runtime files live: `$XDG_RUNTIME_DIR`, else
/// `$TMPDIR`, else `/tmp`.
fn runtime_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    if let Some(dir) = std::env::var_os("TMPDIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    PathBuf::from("/tmp")
}

/// The socket and lock paths for an identity (the binary's basename).
pub fn paths(identity: &str) -> (PathBuf, PathBuf) {
    let dir = runtime_dir();
    let uid = unsafe { libc::getuid() };
    (
        dir.join(format!("{identity}-{uid}.sock")),
        dir.join(format!("{identity}-{uid}.lock")),
    )
}

/// Become the primary, or hand `message` to the one already running.
///
/// Never fails the process: every error path degrades to
/// [`Role::Independent`], because a terminal that will not start because
/// its runtime directory is odd is worse than one that starts twice.
pub fn acquire(identity: &str, message: NewWindow) -> Role {
    let (socket_path, lock_path) = paths(identity);

    let lock = match std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
    {
        Ok(f) => f,
        Err(e) => {
            eprintln!("single-instance: cannot open {}: {e}", lock_path.display());
            return Role::Independent;
        }
    };

    // LOCK_NB: we want the answer "someone else holds it", not a wait.
    let locked = unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0;

    if !locked {
        // Someone is primary. Hand over and let the caller exit.
        return match send(&socket_path, message) {
            Ok(()) => Role::Delivered,
            Err(e) => {
                eprintln!(
                    "single-instance: primary not reachable ({e}), \
                     continuing as independent instance."
                );
                Role::Independent
            }
        };
    }

    // We hold the lock, so any socket file at that path is a corpse: the
    // process that made it cannot still be running, or it would hold the
    // lock. Unlink before binding, since bind() fails on an existing path.
    let _ = std::fs::remove_file(&socket_path);
    let listener = match UnixListener::bind(&socket_path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!(
                "single-instance: cannot bind {}: {e}, \
                 continuing as independent instance.",
                socket_path.display()
            );
            return Role::Independent;
        }
    };
    restrict(&socket_path);

    Role::Primary(Primary {
        listener: Some(listener),
        socket_path,
        lock_path,
        _lock: lock,
    })
}

/// 0600 on the socket: nothing outside this uid opens windows in our
/// process. (`$XDG_RUNTIME_DIR` is already 0700, but `$TMPDIR` may not be.)
fn restrict(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

impl Primary {
    /// The socket this instance is listening on.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Start serving requests on a background thread, calling `handler`
    /// for each one.
    ///
    /// The handler runs on that thread, not on the event loop's, so what
    /// it is given in practice is a
    /// [`winit::event_loop::EventLoopProxy`]: it posts a user event and
    /// returns. Keeping the trait bound at `Fn(NewWindow) + Send` instead
    /// of naming winit here is what lets this module be tested without a
    /// display.
    pub fn serve<F>(&mut self, handler: F)
    where
        F: Fn(NewWindow) + Send + 'static,
    {
        let listener = match self.listener.take() {
            Some(l) => l,
            None => return,
        };
        std::thread::Builder::new()
            .name("single-instance".into())
            .spawn(move || {
                for stream in listener.incoming() {
                    let Ok(stream) = stream else { continue };
                    if let Some(request) = read_request(stream) {
                        handler(request);
                    }
                }
            })
            .expect("spawning the single-instance listener thread");
    }
}

/// Reads one request line and acknowledges it. The ack is written *before*
/// the handler runs on the main thread: it means "taken", not "window is
/// up".
fn read_request(stream: UnixStream) -> Option<NewWindow> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line).ok()? == 0 && line.is_empty() {
        return None;
    }
    let request = NewWindow::decode(&line)?;
    let mut stream = reader.into_inner();
    let _ = stream.write_all(b"ok\n");
    let _ = stream.flush();
    Some(request)
}

/// Client side: hand `message` to whoever is listening on `socket_path`.
fn send(socket_path: &Path, message: NewWindow) -> std::io::Result<()> {
    let mut stream = UnixStream::connect(socket_path)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    stream.write_all(message.encode().as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;

    // Wait for the ack, so exiting 0 really does mean the request landed.
    let mut ack = String::new();
    stream.take(16).read_to_string(&mut ack)?;
    if ack.trim() == "ok" {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("primary answered {ack:?}"),
        ))
    }
}

impl Drop for Primary {
    fn drop(&mut self) {
        // Order matters: unlink the socket while still holding the lock,
        // so a starting instance either sees the lock held (and connects
        // to a socket that is still there) or sees it free (and unlinks
        // whatever is left anyway).
        let _ = std::fs::remove_file(&self.socket_path);
        let _ = std::fs::remove_file(&self.lock_path);
        // `_lock`'s File drop closes the fd, releasing the flock.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    /// Each test gets its own runtime dir, since the paths are global to
    /// the identity by design. `XDG_RUNTIME_DIR` is process-global state,
    /// so the guard serialises the tests that redirect it; without this
    /// the default multi-threaded test runner has one test's scratch dir
    /// deleted out from under another.
    static ENV_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct Scratch(PathBuf, Option<std::sync::MutexGuard<'static, ()>>);

    impl Scratch {
        fn new(tag: &str) -> Self {
            let guard = ENV_GUARD.lock().unwrap_or_else(|e| e.into_inner());
            Self::with_guard(tag, guard)
        }

        fn with_guard(tag: &str, guard: std::sync::MutexGuard<'static, ()>) -> Self {
            let dir = std::env::temp_dir()
                .join(format!("robco-instance-test-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::env::set_var("XDG_RUNTIME_DIR", &dir);
            Scratch(dir, Some(guard))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
            self.1.take();
        }
    }

    #[test]
    fn wire_format_round_trips() {
        assert_eq!(NewWindow::default().encode(), "new-window");
        assert_eq!(
            NewWindow { fullscreen: true, ssh: None }.encode(),
            "new-window --fullscreen"
        );
        assert_eq!(
            NewWindow::decode("new-window --fullscreen\n"),
            Some(NewWindow { fullscreen: true, ssh: None })
        );
        assert_eq!(
            NewWindow::decode("new-window\n"),
            Some(NewWindow::default())
        );
        // An empty message is a new-window request too.
        assert_eq!(NewWindow::decode(""), Some(NewWindow::default()));
        let dialled = NewWindow { fullscreen: true, ssh: Some("overseer@vault:2222".into()) };
        assert_eq!(dialled.encode(), "new-window --fullscreen --ssh overseer@vault:2222");
        assert_eq!(NewWindow::decode(&dialled.encode()), Some(dialled));
        assert_eq!(NewWindow::decode("degauss"), None);
    }

    /// The whole done-test in miniature, without a display: a first
    /// acquire is primary, a second delivers to it, and the handler sees
    /// the fullscreen flag.
    #[test]
    fn second_acquire_delivers_to_the_first() {
        let _scratch = Scratch::new("delivery");
        let (tx, rx) = mpsc::channel();

        let Role::Primary(mut primary) = acquire("test-identity", NewWindow::default()) else {
            panic!("first acquire should be primary");
        };
        primary.serve(move |req| tx.send(req).unwrap());

        match acquire("test-identity", NewWindow { fullscreen: true, ssh: None }) {
            Role::Delivered => {}
            _ => panic!("second acquire should deliver to the primary"),
        }

        let got = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(got, NewWindow { fullscreen: true, ssh: None });
    }

    /// A socket file with nothing behind it (the primary was SIGKILLed)
    /// must not make the next launch think it is a secondary forever.
    #[test]
    fn a_stale_socket_does_not_block_a_new_primary() {
        let scratch = Scratch::new("stale");
        let (socket_path, _lock_path) = paths("test-stale");
        std::fs::write(&socket_path, b"not a socket").unwrap();
        assert!(socket_path.exists());

        match acquire("test-stale", NewWindow::default()) {
            Role::Primary(_) => {}
            _ => panic!("a stale socket file must not prevent becoming primary"),
        }
        drop(scratch);
    }

    /// Dropping the primary must leave nothing behind for the next run.
    #[test]
    fn drop_cleans_up() {
        let _scratch = Scratch::new("cleanup");
        let (socket_path, lock_path) = paths("test-cleanup");
        let role = acquire("test-cleanup", NewWindow::default());
        assert!(socket_path.exists());
        drop(role);
        assert!(!socket_path.exists());
        assert!(!lock_path.exists());
    }
}
