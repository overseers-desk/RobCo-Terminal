//! The settings window, when it lives inside this binary.
//!
//! On Windows under the `embedded-settings` feature the Tcl/Tk settings
//! application is not a second executable beside this one: its interpreter is
//! linked in and its whole script tree rides along as a zip in the binary's
//! own data. [`run`] hands both to the C entry point in
//! `settings/zipfs/appinit.c`, which mounts the zip as the interpreter's
//! filesystem and starts Tk.
//!
//! The process that calls this becomes the settings window. `Tk_Main` runs
//! the event loop and exits the process when the window closes; it does not
//! return, and the `ExitCode` in the signature is there for the compiler
//! rather than for a caller to read. That is why nothing in the terminal
//! calls this on itself: the terminal spawns a *copy* of the binary with
//! `--settings`, and that copy is the one that stops being a terminal.
//!
//! Both arms below export the same `run`, so a caller writes no `cfg` of its
//! own. Off Windows, or with the feature off, `run` is a refusal in one line
//! on stderr: the flag exists everywhere the binary does, and a build that
//! cannot honour it says so instead of doing nothing.

#[cfg(all(windows, feature = "embedded-settings"))]
mod embedded {
    use std::ffi::CString;
    use std::path::Path;
    use std::process::ExitCode;

    /// The settings application's scripts, packed by
    /// `settings/zipfs/build-selfcontained.ps1 -Embed` and named to the
    /// compiler by `build.rs` through `ROBCO_SETTINGS_ZIP`. Bytes in this
    /// binary's data, mounted as a filesystem by the C side; nothing is ever
    /// written to disk for the interpreter to find.
    const ARCHIVE: &[u8] = include_bytes!(env!("ROBCO_SETTINGS_ZIP"));

    extern "C" {
        /// The agreed boundary with `settings/zipfs/appinit.c`, compiled under
        /// `ROBCO_EMBEDDED_SETTINGS`: an argv for the interpreter, the
        /// payload zip as a pointer and a length, and a path for the
        /// diagnostics an interpreter that fails before it has a window has
        /// nowhere else to put. `Tk_Main` is inside; in practice it does not
        /// return.
        fn RobcoSettingsEmbedded_Main(
            argc: std::os::raw::c_int,
            argv: *mut *mut std::os::raw::c_char,
            zip: *const std::ffi::c_void,
            ziplen: usize,
            diagfile: *const std::os::raw::c_char,
        ) -> std::os::raw::c_int;
    }

    /// Become the settings window, passing `passthrough` on to the
    /// interpreter as arguments after argv[0].
    ///
    /// Returns only if the C side declines to start Tk at all, which is why
    /// the return value is a failure code and never a success one.
    pub fn run(passthrough: &[&str], diagfile: &Path) -> ExitCode {
        // argv[0] as the interpreter expects it: the program being run. This
        // binary's own path, since this binary *is* the settings application
        // for the length of this call.
        let program = std::env::current_exe()
            .map(|exe| exe.display().to_string())
            .unwrap_or_else(|_| "robco-term".to_string());

        // The CStrings are kept in `owned` for the whole call: `pointers`
        // holds borrowed pointers into them, and dropping the strings while
        // Tk still holds its argv would hand the interpreter freed memory.
        let mut owned: Vec<CString> = Vec::with_capacity(passthrough.len() + 1);
        owned.push(nul_free(&program));
        for arg in passthrough {
            owned.push(nul_free(arg));
        }
        let mut pointers: Vec<*mut std::os::raw::c_char> = owned
            .iter()
            .map(|s| s.as_ptr() as *mut std::os::raw::c_char)
            .collect();

        let diag = nul_free(&diagfile.display().to_string());

        let code = unsafe {
            RobcoSettingsEmbedded_Main(
                pointers.len() as std::os::raw::c_int,
                pointers.as_mut_ptr(),
                ARCHIVE.as_ptr().cast(),
                ARCHIVE.len(),
                diag.as_ptr(),
            )
        };
        // Reached only on a refusal, and `owned` is still alive up to here on
        // purpose.
        drop(owned);
        if code == 0 {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        }
    }

    /// A `CString` from text that may, in principle, carry a NUL: a path off
    /// Windows's own APIs will not, but the conversion has a failure case and
    /// truncating at the NUL is the reading that keeps a path recognisable
    /// rather than replacing it with nothing.
    fn nul_free(text: &str) -> CString {
        match CString::new(text) {
            Ok(c) => c,
            Err(e) => {
                let upto = e.nul_position();
                CString::new(&text.as_bytes()[..upto]).unwrap_or_default()
            }
        }
    }
}

#[cfg(not(all(windows, feature = "embedded-settings")))]
mod embedded {
    use std::path::Path;
    use std::process::ExitCode;

    /// The same entry point on every build that carries no interpreter: a
    /// sentence on stderr and a failure. This is a visible no-op rather than
    /// a silent one, because the only way to get here is to have asked for
    /// the embedded window by name.
    pub fn run(_passthrough: &[&str], _diagfile: &Path) -> ExitCode {
        eprintln!(
            "the settings window is not embedded in this build: this is not an \
             embedded build (Windows plus the `embedded-settings` feature). \
             Run the companion robco-settings application instead."
        );
        ExitCode::FAILURE
    }
}

pub use embedded::run;
