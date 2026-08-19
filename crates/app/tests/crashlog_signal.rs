//! Standalone on purpose, not part of `tests/suite`: this test raises a
//! fatal signal in its own process to prove the crash logger writes a
//! backtrace on the way down, and a merged suite binary would die with it.
//! The crash logger, proven the only way that means anything: arm it,
//! take a real fatal signal, and read the file that came out.
//!
//! The signal is taken in a forked child, so the test process survives to
//! do the reading. The child does nothing between `fork` and `raise` --
//! no allocation, no locks -- which is what makes forking from a
//! (threaded) test harness safe here: everything the crash path touches
//! was precomputed by `install()` in the parent, before the fork, which
//! is the same discipline the handler itself is written to.

use std::path::Path;
use std::time::{Duration, Instant};

#[test]
fn a_fatal_signal_writes_a_backtrace_and_still_kills_the_process() {
    let dir = std::env::temp_dir().join(format!("robco-crashlog-signal-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    let log_path = app::crashlog::install(&dir).expect("arming the crash logger");
    assert!(!log_path.exists(), "nothing written before a crash");

    let pid = unsafe { libc::fork() };
    assert!(pid >= 0, "fork failed");
    if pid == 0 {
        // Child: take the signal. The handler writes the log, restores
        // the default disposition and re-raises, so we die of SIGSEGV.
        unsafe {
            libc::raise(libc::SIGSEGV);
            // Unreachable if the handler behaved; if it swallowed the
            // signal, exit distinctly so the parent's assertion says so.
            libc::_exit(97);
        }
    }

    let mut status: libc::c_int = 0;
    let waited = unsafe { libc::waitpid(pid, &mut status, 0) };
    assert_eq!(waited, pid, "waitpid");

    // The signal must still reach the process: the point of re-raising is
    // that cores and system crash tooling see a crash, not a clean exit.
    assert!(
        libc::WIFSIGNALED(status),
        "child should have died of a signal, status {status}"
    );
    assert_eq!(libc::WTERMSIG(status), libc::SIGSEGV);

    let text = read_soon(&log_path).expect("crash log written");
    assert!(
        text.starts_with(&format!("fatal signal {}\n", libc::SIGSEGV)),
        "first line names the signal, got: {:?}",
        text.lines().next()
    );
    // backtrace_symbols_fd writes one frame per line; the header plus at
    // least one frame is what "a backtrace was captured" means.
    assert!(
        text.lines().count() > 1,
        "log should carry frames, got: {text:?}"
    );
    // The frames name this test binary, which is what makes addr2line
    // against it useful.
    assert!(
        text.contains("crashlog_signal"),
        "frames should name the crashing binary, got: {text:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// The child writes the file and dies; `waitpid` returning does not
/// guarantee the parent's view of the directory is up to date on every
/// filesystem, so poll briefly rather than race.
fn read_soon(path: &Path) -> Option<String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(text) = std::fs::read_to_string(path) {
            if !text.is_empty() {
                return Some(text);
            }
        }
        if Instant::now() > deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}
