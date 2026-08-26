//! Last-gasp crash logger.
//!
//! Arms a handler for the fatal signals (SEGV, ABRT, BUS, FPE, ILL) that
//! writes a raw backtrace to a file and to stderr, then restores the
//! default disposition and re-raises, so cores and system crash tooling
//! still see the signal. The frames carry module+offset; `addr2line`
//! against the binary turns them into file:line.
//!
//! ## The discipline this file keeps
//!
//! A fatal signal often arrives *because* the heap is corrupt, so the
//! handler may call nothing that allocates, locks, or reenters the
//! runtime. That constraint is what makes this file look the way it does:
//!
//! - **The path is precomputed.** `install()` formats the log path once,
//!   at startup, into a fixed `PATH_MAX` buffer. The handler only reads
//!   it. It never formats, never joins, never touches `PathBuf`.
//! - **The signal-number text is built by hand** into a stack array, with
//!   the two digits computed arithmetically. `format!` allocates; even
//!   `write!` into a `String` allocates.
//! - **`backtrace()` is warmed at install time.** The first call loads
//!   libgcc's unwinder and allocates while doing it; that has to be behind
//!   us before the handler runs. This is the subtlety most ports of this
//!   file drop.
//! - **Only async-signal-safe calls in the handler**: `backtrace`,
//!   `backtrace_symbols_fd`, `open`, `write`, `close`, `signal`, `raise`.
//!   No `println!` (Rust's stdout takes a lock), no `std::fs`, no panic.
//!
//! `backtrace`/`backtrace_symbols_fd` are glibc's; on a non-glibc Unix the
//! handler still writes the signal line and re-raises, which is the part
//! that matters for "did it die, and of what".
//!
//! The whole handler is the Unix arm. On Windows `install` arms nothing
//! and answers `None`, which the caller's verbose path reports as "crash
//! log not armed"; the arm to build there is a structured-exception
//! filter (`SetUnhandledExceptionFilter`) under the same discipline.

#[cfg(unix)]
use std::ffi::c_void;
use std::path::Path;
#[cfg(unix)]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(unix)]
const PATH_MAX: usize = 4096;
#[cfg(unix)]
const MAX_FRAMES: usize = 64;

/// The fatal signals this logger arms, in the order they are installed.
#[cfg(unix)]
const FATAL_SIGNALS: [libc::c_int; 5] = [
    libc::SIGSEGV,
    libc::SIGABRT,
    libc::SIGBUS,
    libc::SIGFPE,
    libc::SIGILL,
];

/// The precomputed, NUL-terminated log path. Written once by `install()`
/// before any handler is armed, read-only afterwards, which is what makes
/// the handler's access to it sound.
#[cfg(unix)]
static mut LOG_PATH: [u8; PATH_MAX] = [0; PATH_MAX];
#[cfg(unix)]
static ARMED: AtomicBool = AtomicBool::new(false);

#[cfg(all(unix, target_env = "gnu"))]
extern "C" {
    fn backtrace(buffer: *mut *mut c_void, size: libc::c_int) -> libc::c_int;
    fn backtrace_symbols_fd(buffer: *const *mut c_void, size: libc::c_int, fd: libc::c_int);
}

#[cfg(all(unix, not(target_env = "gnu")))]
unsafe fn backtrace(_buffer: *mut *mut c_void, _size: libc::c_int) -> libc::c_int {
    0
}

#[cfg(all(unix, not(target_env = "gnu")))]
unsafe fn backtrace_symbols_fd(_buffer: *const *mut c_void, _size: libc::c_int, _fd: libc::c_int) {}

/// `write(2)` until the whole NUL-terminated string is out or the fd
/// refuses it.
///
/// # Safety
/// `text` must point at a NUL-terminated buffer that stays valid for the
/// call.
#[cfg(unix)]
unsafe fn write_all(fd: libc::c_int, text: *const u8) {
    let mut at = text;
    let mut remaining = libc::strlen(text as *const libc::c_char);
    while remaining > 0 {
        let put = libc::write(fd, at as *const c_void, remaining);
        if put <= 0 {
            return;
        }
        at = at.add(put as usize);
        remaining -= put as usize;
    }
}

/// The handler. Everything in here is async-signal-safe; see the module
/// comment for why that constrains it so hard.
#[cfg(unix)]
extern "C" fn handle_fatal_signal(signal_number: libc::c_int) {
    unsafe {
        let mut frames: [*mut c_void; MAX_FRAMES] = [std::ptr::null_mut(); MAX_FRAMES];
        let depth = backtrace(frames.as_mut_ptr(), MAX_FRAMES as libc::c_int);

        // "fatal signal NN\n\0", built by hand on the stack.
        let mut head = [0u8; 64];
        let mut at = 0usize;
        for &c in b"fatal signal " {
            head[at] = c;
            at += 1;
        }
        let n = if signal_number < 0 { 0 } else { signal_number };
        if n >= 100 {
            head[at] = b'0' + (n / 100) as u8;
            at += 1;
        }
        if n >= 10 {
            head[at] = b'0' + ((n / 10) % 10) as u8;
            at += 1;
        }
        head[at] = b'0' + (n % 10) as u8;
        at += 1;
        head[at] = b'\n';
        at += 1;
        head[at] = 0;

        let path = &raw const LOG_PATH as *const u8;
        if *path != 0 {
            let fd = libc::open(
                path as *const libc::c_char,
                libc::O_CREAT | libc::O_WRONLY | libc::O_TRUNC,
                0o644 as libc::c_uint,
            );
            if fd >= 0 {
                write_all(fd, head.as_ptr());
                backtrace_symbols_fd(frames.as_ptr(), depth, fd);
                libc::close(fd);
            }
        }
        write_all(libc::STDERR_FILENO, head.as_ptr());
        backtrace_symbols_fd(frames.as_ptr(), depth, libc::STDERR_FILENO);

        // Restore the default disposition and re-raise, so the kernel
        // still produces a core and the desktop's crash reporter still
        // sees a crash rather than a clean exit.
        libc::signal(signal_number, libc::SIG_DFL);
        libc::raise(signal_number);
    }
}

/// The log path for a crash in this process: `<directory>/crash-<unix
/// seconds>-<pid>.log`.
pub fn log_path_in(directory: &Path) -> std::path::PathBuf {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    directory.join(format!("crash-{seconds}-{}.log", std::process::id()))
}

/// Arms the handler, writing crashes under `directory`.
///
/// Returns the path a crash would be written to, or `None` if the logger
/// could not be armed (unwritable directory, or a path that does not fit
/// the fixed buffer). Never fails the process: a terminal that will not
/// start because it could not arm its crash logger is a worse outcome
/// than one that starts without it.
#[cfg(unix)]
pub fn install(directory: &Path) -> Option<std::path::PathBuf> {
    if ARMED.swap(true, Ordering::SeqCst) {
        return None;
    }
    if std::fs::create_dir_all(directory).is_err() {
        return None;
    }
    let path = log_path_in(directory);
    let bytes = path.to_string_lossy().into_owned().into_bytes();
    if bytes.len() >= PATH_MAX {
        return None;
    }

    unsafe {
        let dst = &raw mut LOG_PATH as *mut u8;
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), dst, bytes.len());
        *dst.add(bytes.len()) = 0;

        // Warm the unwinder: the first backtrace() dlopens libgcc and
        // allocates. That must be behind us before a handler ever runs.
        let mut warm: [*mut c_void; 2] = [std::ptr::null_mut(); 2];
        backtrace(warm.as_mut_ptr(), 2);

        for signal_number in FATAL_SIGNALS {
            libc::signal(
                signal_number,
                handle_fatal_signal as *const () as libc::sighandler_t,
            );
        }
    }

    Some(path)
}

/// The unbuilt arm: arms nothing and says so through the `None` the
/// caller already reports. See the module doc for what belongs here.
#[cfg(not(unix))]
pub fn install(_directory: &Path) -> Option<std::path::PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_path_carries_the_pid_and_lives_in_the_directory() {
        let path = log_path_in(Path::new("/some/dir"));
        assert!(path.starts_with("/some/dir"));
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("crash-"), "{name}");
        assert!(
            name.ends_with(&format!("-{}.log", std::process::id())),
            "{name}"
        );
    }

    /// Arming must create the directory and hand back a path inside it,
    /// and must be idempotent (a second install is a no-op, so a
    /// multi-window app cannot re-arm and re-warm mid-flight).
    #[test]
    #[cfg(unix)]
    fn install_creates_the_directory_and_arms_once() {
        let dir = std::env::temp_dir().join(format!("robco-crashlog-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let path = install(&dir).expect("first install arms");
        assert!(dir.is_dir());
        assert!(path.starts_with(&dir));
        assert!(install(&dir).is_none(), "second install must be a no-op");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
