//! Record the transcript fixtures from a real tmux server.
//!
//! ```text
//! cargo run -p robco-tmux-cc --example record
//! ```
//!
//! Writes `crates/tmux-cc/tests/transcripts/*.txt` (every byte tmux wrote to a
//! control client, DCS envelope included) and a `.cmds` sidecar per transcript
//! (the wire lines this client sent, in order, which is what lets the FIFO
//! pairing be replayed). Rerunning overwrites them.
//!
//! Not a test and not run by the suite: it needs a tmux binary and a few
//! seconds of wall clock, and the point of committing its output is that the
//! decode tests do not. Rerun it when the tmux under test changes, and read
//! the diff: a transcript that moves is the protocol having moved.
//!
//! Everything each phase does is stated in `tests/transcripts/README.md`, which
//! this program also writes, so the fixtures cannot outlive their description.

#[path = "../tests/support/mod.rs"]
mod support;

use std::path::{Path, PathBuf};

use support::{Gateway, Server, TMUX};
use tmux_cc::{Command, PaneId, WindowId};

fn main() {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/transcripts");
    std::fs::create_dir_all(&out).expect("transcripts dir");

    let version = std::process::Command::new(TMUX)
        .arg("-V")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    println!("recording against {version}");

    fresh_session(&out);
    second_window(&out);
    rename(&out);
    output_octal(&out);
    kill_window(&out);
    detach_from_the_server(&out);
    reattach_after_kill(&out);
    error_and_extended_output(&out);
    quoting(&out);
    notification_zoo(&out);

    std::fs::write(out.join("README.md"), readme(&version)).expect("write README");
    println!("done");
}

fn write(out: &Path, name: &str, gateway: &Gateway) {
    let transcript = out.join(format!("{name}.txt"));
    std::fs::write(&transcript, gateway.transcript()).expect("write transcript");
    std::fs::write(
        out.join(format!("{name}.cmds")),
        gateway
            .sent()
            .iter()
            .map(|line| format!("{line}\n"))
            .collect::<String>(),
    )
    .expect("write cmds");
    println!(
        "  {name}: {} bytes, {} command(s)",
        gateway.transcript().len(),
        gateway.sent().len()
    );
}

/// Attach to a one-window session and run the standard attach bootstrap.
fn fresh_session(out: &Path) {
    let server = Server::start("fresh", "one", "/bin/cat");
    let mut gateway = server.attach("one");
    gateway.settle(500);
    gateway.send(&Command::HostName);
    gateway.send(&Command::ListPanes);
    gateway.settle(400);
    gateway.send(&Command::ListWindows);
    gateway.settle(400);
    gateway.send(&Command::ClientSize {
        columns: 100,
        rows: 30,
    });
    gateway.settle(400);
    gateway.send(&Command::DetachClient);
    gateway.settle(500);
    write(out, "01-fresh-session", &gateway);
}

/// Ask tmux for a window and hear it arrive.
fn second_window(out: &Path) {
    let server = Server::start("second", "one", "/bin/cat");
    let mut gateway = server.attach("one");
    gateway.settle(500);
    gateway.send(&Command::ListPanes);
    gateway.settle(300);
    gateway.send(&Command::NewWindow);
    gateway.wait_for(b"%window-add", 3000);
    gateway.settle(300);
    // What `%window-add` does not say: which pane the new window shows.
    gateway.send(&Command::ListPanes);
    gateway.settle(400);
    gateway.send(&Command::DetachClient);
    gateway.settle(400);
    write(out, "02-second-window", &gateway);
}

/// Renames, and what tmux does to a name on the way out.
///
/// Every awkward thing a name can hold, one rename at a time: two consecutive
/// spaces (which re-joining split fields would collapse), a backslash, control
/// bytes, valid UTF-8, and bytes that are not UTF-8 at all. This is the
/// evidence behind `escape::unvis`.
fn rename(out: &Path) {
    let server = Server::start("rename", "one", "/bin/cat");
    server.run(&["new-window", "-t", "one", "-d", "/bin/cat"]);
    let mut gateway = server.attach("one");
    gateway.settle(500);
    gateway.send(&Command::ListWindows);
    gateway.settle(300);
    server.run(&["rename-window", "-t", "@0", "two  spaces"]);
    gateway.wait_for(b"%window-renamed", 3000);
    gateway.settle(200);
    server.run(&["rename-window", "-t", "@1", "back\\slash"]);
    gateway.settle(500);
    server.run(&["rename-window", "-t", "@1", "tab\there bell\x07end"]);
    gateway.settle(500);
    server.run(&["rename-window", "-t", "@1", "utf8 \u{4f60}\u{597d} end"]);
    gateway.settle(500);
    // Not UTF-8: the one case that has to come out octal.
    server.run_bytes(&[b"rename-window", b"-t", b"@1", b"high \xff\xfe end"]);
    gateway.settle(500);
    server.run(&["rename-session", "-t", "$0", "sess\\back"]);
    gateway.settle(600);
    gateway.send(&Command::ListWindows);
    gateway.settle(400);
    gateway.send(&Command::DetachClient);
    gateway.settle(400);
    write(out, "03-rename", &gateway);
}

/// Output from two windows, one of them all 256 byte values.
///
/// The pane runs `stty raw -echo` first, or the line discipline mangles what
/// `cat` is about to write and the fixture measures the tty rather than tmux.
fn output_octal(out: &Path) {
    let server = Server::start("octal", "one", "/bin/sh");
    // A fixed path, not one under the server's per-run scratch: the `cat`
    // command goes into the pane as hex and therefore into the committed
    // `.cmds` sidecar, and a path carrying a process id would make every
    // re-record show a change that is not the protocol's.
    let bytes = std::env::temp_dir().join("robco-tmux-cc-allbytes.bin");
    std::fs::write(&bytes, (0..=255u8).collect::<Vec<u8>>()).expect("write byte file");

    server.run(&["new-window", "-t", "one", "-d", "/bin/sh"]);
    let mut gateway = server.attach("one");
    gateway.settle(600);
    gateway.send(&Command::ListPanes);
    gateway.settle(400);

    let pane0 = PaneId::parse("%0").unwrap();
    let pane1 = PaneId::parse("%1").unwrap();
    for pane in [&pane0, &pane1] {
        for command in Command::send_keys(pane, b"stty raw -echo\n") {
            gateway.send(&command);
        }
        gateway.settle(300);
    }
    // Window 0 writes text with the two escapes that matter in it; window 1
    // writes every byte there is.
    for command in Command::send_keys(&pane0, b"printf 'back\\\\slash\\r\\n'\n") {
        gateway.send(&command);
    }
    gateway.settle(500);
    let cat = format!("cat {}\n", bytes.display());
    for command in Command::send_keys(&pane1, cat.as_bytes()) {
        gateway.send(&command);
    }
    gateway.settle(1200);
    gateway.send(&Command::DetachClient);
    gateway.settle(400);
    write(out, "04-output-octal", &gateway);
}

/// Close a background window from the server and the current one through the
/// codec.
fn kill_window(out: &Path) {
    let server = Server::start("kill", "one", "/bin/cat");
    server.run(&["new-window", "-t", "one", "-d", "/bin/cat"]);
    server.run(&["new-window", "-t", "one", "-d", "/bin/cat"]);
    let mut gateway = server.attach("one");
    gateway.settle(600);
    gateway.send(&Command::ListPanes);
    gateway.settle(400);
    server.run(&["kill-window", "-t", "@1"]);
    gateway.wait_for(b"window-close", 3000);
    gateway.settle(300);
    gateway.send(&Command::KillWindow(WindowId::parse("@2").unwrap()));
    gateway.settle(800);
    gateway.send(&Command::DetachClient);
    gateway.settle(500);
    write(out, "05-kill-window", &gateway);
}

/// The other detach: the server throws this client off.
fn detach_from_the_server(out: &Path) {
    let server = Server::start("detach", "one", "/bin/cat");
    let mut gateway = server.attach("one");
    gateway.settle(600);
    gateway.send(&Command::ListPanes);
    gateway.settle(400);
    let clients = server.run(&["list-clients", "-F", "#{client_name}"]);
    for client in clients.lines() {
        server.run(&["detach-client", "-t", client]);
    }
    gateway.settle(800);
    write(out, "06-detach", &gateway);
}

/// Issue #4's shape: a client dies without closing its device string, and the
/// next one attaches onto a session that already carries three windows.
///
/// The recording is the second attach. What it has to show is a negative:
/// tmux announces no `%window-add` for a window that already existed, so a
/// client that learns its windows only from notifications comes up with an
/// empty bank. The listing is the only source of truth on attach.
fn reattach_after_kill(out: &Path) {
    let server = Server::start("reattach", "one", "/bin/cat");
    let mut first = server.attach("one");
    first.settle(500);
    first.send(&Command::NewWindow);
    first.wait_for(b"%window-add", 3000);
    first.send(&Command::NewWindow);
    first.wait_for(b"%window-add @2", 3000);
    first.settle(400);
    first.kill_client();
    write(out, "07a-before-the-kill", &first);

    let mut second = server.attach("one");
    second.settle(700);
    second.send(&Command::ListPanes);
    second.settle(500);
    second.send(&Command::ListWindows);
    second.settle(500);
    second.send(&Command::DetachClient);
    second.settle(500);
    write(out, "07b-reattach", &second);
}

/// A command that fails, and the `%output` form the pause-after flag turns on.
fn error_and_extended_output(out: &Path) {
    let server = Server::start("extended", "one", "/bin/cat");
    let mut gateway = server.attach("one");
    gateway.settle(500);
    gateway.send_raw("this-command-does-not-exist");
    gateway.wait_for(b"%error", 3000);
    gateway.settle(200);
    // Not something the codec can say: no appliance sets this flag. Recorded
    // anyway, because the decoder has to know the form when a host does.
    gateway.send_raw("refresh-client -f pause-after=1");
    gateway.settle(300);
    let pane = PaneId::parse("%0").unwrap();
    for command in Command::send_keys(&pane, b"extended\n") {
        gateway.send(&command);
    }
    gateway.wait_for(b"%extended-output", 3000);
    gateway.settle(500);
    gateway.send(&Command::DetachClient);
    gateway.settle(400);
    write(out, "08-error-and-extended-output", &gateway);
}

/// tmux's own lexer, probed. The evidence behind `command::quote_format`.
fn quoting(out: &Path) {
    let server = Server::start("quoting", "one", "/bin/cat");
    server.run(&["set-environment", "-g", "MYVAR", "expanded"]);
    let mut gateway = server.attach("one");
    gateway.settle(500);
    // Unquoted: `#` starts a comment, the argument vanishes, and
    // display-message prints its default status line instead of erroring.
    gateway.send_raw("display-message -p #{host_short}");
    gateway.settle(300);
    gateway.send(&Command::HostName);
    gateway.settle(300);
    gateway.send_raw(r#"display-message -p "dollar ${MYVAR} end""#);
    gateway.settle(300);
    gateway.send_raw(r#"display-message -p "quote \" backslash \\ end""#);
    gateway.settle(300);
    gateway.send_raw("display-message -p 'single #{host_short} quoted'");
    gateway.settle(300);
    gateway.send(&Command::DetachClient);
    gateway.settle(400);
    write(out, "09-quoting", &gateway);
}

/// As many notification names as one session can be made to produce, so the
/// decoder's breadth is evidence rather than a reading of the manual.
fn notification_zoo(out: &Path) {
    let server = Server::start("zoo", "one", "/bin/cat");
    let mut gateway = server.attach("one");
    gateway.settle(500);
    gateway.send(&Command::ListPanes);
    gateway.settle(300);

    // %sessions-changed, %unlinked-window-add
    server.run(&["new-session", "-d", "-s", "two", "/bin/cat"]);
    gateway.settle(400);
    // %unlinked-window-renamed: that other session's window. And
    // %window-renamed: this one's. Both asked for outright, since automatic
    // renaming is off for every server here.
    server.run(&["rename-window", "-t", "@1", "not-my-window"]);
    gateway.settle(400);
    server.run(&["rename-window", "-t", "@0", "my-window"]);
    gateway.settle(400);
    // %session-renamed
    server.run(&["rename-session", "-t", "one", "one-renamed"]);
    gateway.settle(400);
    // %paste-buffer-changed / %paste-buffer-deleted. The backslash in the
    // buffer name is the point: it arrives single, so buffer names are not
    // vis-encoded the way window and session names are.
    server.run(&["set-buffer", "-b", "buf\\back", "contents"]);
    gateway.settle(300);
    server.run(&["delete-buffer", "-b", "buf\\back"]);
    gateway.settle(300);
    // %message, inside its own reply block, and raw for the same reason.
    gateway.send_raw(r#"display-message "msg \\ back""#);
    gateway.settle(300);
    // %layout-change, then %window-pane-changed. The pane is addressed by its
    // index within the window: pane ids are server-wide, so `%1` is whatever
    // the other session happened to take.
    server.run(&["split-window", "-t", "@0", "-d", "/bin/cat"]);
    gateway.settle(400);
    server.run(&["select-pane", "-t", "@0.1"]);
    gateway.settle(400);
    // %pane-mode-changed
    server.run(&["copy-mode", "-t", "@0.0"]);
    gateway.settle(400);

    // %client-session-changed and %client-detached are about *other* clients,
    // so they need one. A second control client attaches, moves to the other
    // session, and leaves; this gateway hears all three.
    let mut bystander = server.attach("one");
    bystander.settle(500);
    bystander.send_raw("switch-client -t $1");
    bystander.settle(500);
    gateway.settle(300);
    bystander.send(&Command::DetachClient);
    bystander.settle(500);
    gateway.settle(400);
    drop(bystander);

    // %session-changed, on this very client
    gateway.send_raw("switch-client -t $1");
    gateway.settle(600);
    // %exit, with the server going away under it
    server.run(&["kill-server"]);
    gateway.settle(700);
    write(out, "10-notification-zoo", &gateway);
}

fn readme(version: &str) -> String {
    format!(
        "# Recorded tmux control-mode transcripts\n\
         \n\
         Every `.txt` here is the exact byte stream a `tmux -CC attach` client\n\
         received on its PTY, DCS envelope (`ESC P 1000 p` ... `ESC \\`)\n\
         included. Every `.cmds` beside one is the wire lines that client sent,\n\
         in order: the pairing queue, replayable.\n\
         \n\
         Recorded from **{version}** by `cargo run -p robco-tmux-cc --example\n\
         record`, which is the only thing that should ever write them. A\n\
         protocol claim is only true of a version, so a transcript that changes\n\
         under a re-record is the news, not the noise.\n\
         \n\
         | file | what it holds |\n\
         |---|---|\n\
         | `01-fresh-session` | Attach to a one-window session, the attach bootstrap (host, list-panes, list-windows), a client resize, a client-side detach. The attach burst's blocks and the replies to those commands are both here, which is where the `%begin` flags field gives itself away. |\n\
         | `02-second-window` | `new-window` through the codec: `%window-add`, `%session-window-changed`, `%layout-change`, and the re-listing that finds the new window's pane, which `%window-add` does not name. |\n\
         | `03-rename` | Renames from the server, one awkward name at a time: two consecutive spaces, a backslash, a tab and a BEL, valid UTF-8, and bytes that are not UTF-8. The evidence behind `escape::unvis` -- a name is C-escaped by `vis(3)`, not octal-escaped like a payload -- and behind `%session-renamed` carrying a session id the manual does not mention. |\n\
         | `04-output-octal` | Two windows producing output, one of them writing all 256 byte values through a raw tty. The escaping set is measured here: `0x00..=0x1f` and `\\`, and nothing else. |\n\
         | `05-kill-window` | A background window killed from the server and the current one killed through the codec. Both arrive as `%unlinked-window-close`; `%window-close` never appears. |\n\
         | `06-detach` | The server throwing this client off with `detach-client -t`. |\n\
         | `07a-before-the-kill`, `07b-reattach` | Issue #4's shape. The first client builds a three-window session and is SIGKILLed mid-protocol (no `%exit`, no `ST`). The second attaches onto that session: its burst announces **no** window, so a client that learns windows from notifications alone comes up empty. |\n\
         | `08-error-and-extended-output` | A command tmux refuses (`%error`), and the `%extended-output` form that the `pause-after` client flag turns `%output` into. |\n\
         | `09-quoting` | tmux's command lexer, probed: unquoted `#{{...}}` swallowed as a comment, the same quoted, `${{VAR}}` expanded inside double quotes, `\\\"` and `\\\\` unescaped, and single quotes not stopping format expansion. |\n\
         | `10-notification-zoo` | As many notification names as one session can be made to emit: `%sessions-changed`, `%unlinked-window-add`, `%session-renamed`, the paste-buffer pair, `%message` (inside a reply block, which the manual says cannot happen), `%layout-change`, `%window-pane-changed`, `%pane-mode-changed`, `%client-session-changed`, and `%exit` under a dying server. |\n"
    )
}
