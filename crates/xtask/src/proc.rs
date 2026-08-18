//! Small subprocess helpers shared by snap and mask. Driving the
//! battle-tested CLI tools (xdotool, xprop, import, python3) is a better
//! trade than reimplementing image/X11 logic in Rust, so this module wraps
//! shelling out to them rather than replacing them.

use anyhow::{bail, Context, Result};
use std::process::{Command, Output, Stdio};

/// Run a command, discarding stdio, and error if it exits non-zero. A call
/// site that wants to ignore failure uses [`run_ignore_status`] instead.
pub fn run_ok(cmd: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(cmd)
        .args(args)
        .status()
        .with_context(|| format!("spawning `{cmd}`"))?;
    if !status.success() {
        bail!("`{cmd} {}` exited with {status}", args.join(" "));
    }
    Ok(())
}

/// Same as [`run_ok`] but never errors on a non-zero exit, for a call like
/// `xdotool windowsize` whose failure is not fatal to the caller.
pub fn run_ignore_status(cmd: &str, args: &[&str]) {
    let _ = Command::new(cmd).args(args).status();
}

/// Run a command and capture stdout as a String (trimmed of a trailing
/// newline only, like `$(...)` command substitution in bash).
pub fn capture(cmd: &str, args: &[&str]) -> Result<String> {
    let output: Output = Command::new(cmd)
        .args(args)
        .stderr(Stdio::inherit())
        .output()
        .with_context(|| format!("spawning `{cmd}`"))?;
    if !output.status.success() {
        bail!("`{cmd} {}` exited with {}", args.join(" "), output.status);
    }
    let mut s = String::from_utf8(output.stdout)
        .with_context(|| format!("`{cmd}` gave non-UTF8 output"))?;
    if s.ends_with('\n') {
        s.pop();
    }
    Ok(s)
}

/// Same as [`capture`] but returns "" instead of erroring on a non-zero
/// exit: xdotool search/getwindowpid routinely exit non-zero when nothing
/// matches yet, which callers here treat as "no candidate", not a failure.
pub fn capture_lenient(cmd: &str, args: &[&str]) -> String {
    let output = Command::new(cmd).args(args).stderr(Stdio::null()).output();
    match output {
        Ok(o) => {
            let mut s = String::from_utf8_lossy(&o.stdout).into_owned();
            if s.ends_with('\n') {
                s.pop();
            }
            s
        }
        Err(_) => String::new(),
    }
}

/// Re-run this same xtask under `xvfb-run` on a private display, guarded
/// by an environment variable so the inner invocation does not wrap
/// itself again.
///
/// Rust has no direct process-image-replace on all targets, so this spawns
/// `current_exe()` as a child rather than exec-replacing itself; it then
/// exits with the child's code, which is externally indistinguishable from
/// a replace.
pub fn reexec_under_xvfb(guard_env: &str, screen: &str, argv: &[String]) -> Result<()> {
    let self_exe = std::env::current_exe().context("resolving current_exe for re-exec")?;
    let status = Command::new("xvfb-run")
        .arg("-a")
        .arg("-s")
        .arg(screen)
        .arg(&self_exe)
        .args(argv)
        .env(guard_env, "1")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("spawning xvfb-run")?;
    std::process::exit(status.code().unwrap_or(1));
}
