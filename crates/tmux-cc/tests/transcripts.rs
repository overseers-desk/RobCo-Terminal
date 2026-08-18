//! The codec against recorded tmux 3.5a.
//!
//! Two layers. The first holds over *every* transcript at once: nothing
//! unknown, nothing malformed, no guard mismatch, the pairing queue coming out
//! in order, every payload surviving a round trip through the octal escaping.
//! The second reads each recording for the thing it was recorded to show.
//!
//! No tmux runs here: these are bytes on disk. `tests/live_tmux.rs` is the one
//! that needs a server.

mod support;

use std::path::PathBuf;

use support::{control_stream, envelope_closed, find, replay};
use tmux_cc::escape::{escape_octal, unescape_octal};
use tmux_cc::{Event, Notification};

/// Every recording, in the order the recorder writes them.
const TRANSCRIPTS: &[&str] = &[
    "01-fresh-session",
    "02-second-window",
    "03-rename",
    "04-output-octal",
    "05-kill-window",
    "06-detach",
    "07a-before-the-kill",
    "07b-reattach",
    "08-error-and-extended-output",
    "09-quoting",
    "10-notification-zoo",
];

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/transcripts")
}

fn transcript(name: &str) -> Vec<u8> {
    std::fs::read(dir().join(format!("{name}.txt"))).expect("read transcript")
}

fn commands(name: &str) -> Vec<String> {
    let text = std::fs::read_to_string(dir().join(format!("{name}.cmds"))).expect("read cmds");
    text.lines().map(|l| l.to_string()).collect()
}

/// Decode a whole fixture with its own command list replayed into the queue.
fn events(name: &str) -> Vec<Event> {
    replay(&transcript(name), commands(name).len())
}

fn notifications(name: &str) -> Vec<Notification> {
    events(name)
        .into_iter()
        .filter_map(|e| match e {
            Event::Notification(n) => Some(n),
            _ => None,
        })
        .collect()
}

fn replies(name: &str) -> Vec<tmux_cc::Block> {
    events(name)
        .into_iter()
        .filter_map(|e| match e {
            Event::Reply { block, .. } => Some(block),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------- everywhere

#[test]
fn every_transcript_is_a_control_stream() {
    for name in TRANSCRIPTS {
        let raw = transcript(name);
        assert!(
            !control_stream(&raw).is_empty(),
            "{name}: no DCS 1000p envelope"
        );
        // Every one but the killed client closed its envelope; that one is the
        // point of the fixture (no `%exit`, no `ST`).
        let closed = envelope_closed(&raw);
        if *name == "07a-before-the-kill" {
            assert!(!closed, "07a: the killed client should not have said ST");
        } else {
            assert!(closed, "{name}: envelope never closed");
        }
    }
}

#[test]
fn every_notification_in_every_transcript_parses() {
    for name in TRANSCRIPTS {
        for notification in notifications(name) {
            match notification {
                Notification::Unknown { name: what, rest } => panic!(
                    "{name}: no variant for {what} ({})",
                    String::from_utf8_lossy(&rest)
                ),
                Notification::Malformed { name: what, rest } => panic!(
                    "{name}: {what} did not fit its shape ({})",
                    String::from_utf8_lossy(&rest)
                ),
                _ => {}
            }
        }
    }
}

#[test]
fn nothing_foreign_and_no_guard_ever_mismatched() {
    for name in TRANSCRIPTS {
        for event in events(name) {
            match event {
                Event::Foreign(line) => {
                    panic!("{name}: foreign line {}", String::from_utf8_lossy(&line))
                }
                Event::GuardMismatch { began, closed } => {
                    panic!("{name}: %begin {began:?} closed by {closed:?}")
                }
                _ => {}
            }
        }
    }
}

#[test]
fn the_pairing_queue_comes_out_in_order() {
    for name in TRANSCRIPTS {
        let sent = commands(name);
        let mut expected = 0u64;
        let mut paired = 0usize;
        for event in events(name) {
            if let Event::Reply { id, .. } = event {
                assert_eq!(
                    id,
                    tmux_cc::CommandId(expected),
                    "{name}: replies arrived out of order"
                );
                expected += 1;
                paired += 1;
            }
        }
        assert!(
            paired <= sent.len(),
            "{name}: {paired} replies for {} commands",
            sent.len()
        );
        // A transcript that detaches has heard every answer; one that was
        // killed has not, and that is the only excuse.
        if *name != "07a-before-the-kill" {
            assert_eq!(
                paired,
                sent.len(),
                "{name}: {paired} replies for {} commands",
                sent.len()
            );
        }
    }
}

#[test]
fn the_flags_field_sorts_the_burst_from_the_replies() {
    // The law the pairing rests on, held across every recording: a block that
    // answers one of this client's commands carries flags 1, and every block
    // that answers nobody carries 0.
    let mut bursts = 0;
    for name in TRANSCRIPTS {
        for event in events(name) {
            match event {
                Event::Reply { block, .. } => {
                    assert_eq!(block.flags, Some(1), "{name}: a reply with flags != 1")
                }
                Event::Unsolicited { block } => {
                    assert_eq!(
                        block.flags,
                        Some(0),
                        "{name}: an unpaired block with flags 1"
                    );
                    bursts += 1;
                }
                _ => {}
            }
        }
    }
    // Every attach opens with one, so this is not vacuous.
    assert_eq!(bursts, TRANSCRIPTS.len());
}

#[test]
fn every_output_payload_round_trips_through_the_octal_escaping() {
    // Straight off the wire: for each `%output`/`%extended-output` line in
    // each transcript, decoding and re-encoding the payload has to give back
    // the bytes tmux wrote. Any byte the codec escaped differently from tmux
    // shows up here.
    let mut payloads = 0;
    for name in TRANSCRIPTS {
        let stream = control_stream(&transcript(name));
        for line in stream.split(|&b| b == b'\n') {
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            let payload = if line.starts_with(b"%output ") {
                line.splitn(3, |&b| b == b' ').nth(2)
            } else if line.starts_with(b"%extended-output ") {
                find(line, b" : ").map(|at| &line[at + 3..])
            } else {
                None
            };
            let Some(payload) = payload else { continue };
            assert_eq!(
                escape_octal(&unescape_octal(payload)),
                payload,
                "{name}: payload did not round trip"
            );
            payloads += 1;
        }
    }
    assert!(payloads >= 8, "only {payloads} payloads seen");
}

// ------------------------------------------------------------ one at a time

#[test]
fn the_fresh_session_bootstraps_and_leaves() {
    let events = events("01-fresh-session");
    let blocks = replies("01-fresh-session");
    // host, list-panes, list-windows, refresh-client, detach-client.
    assert_eq!(blocks.len(), 5);
    assert_eq!(blocks[0].text().len(), 1, "the host name came back empty");
    assert_eq!(blocks[1].text().len(), 1, "one pane in a fresh session");
    assert!(blocks[1].text()[0].starts_with("@0 %0 1 "));
    assert!(blocks[3].body.is_empty(), "refresh-client answers nothing");

    let names: Vec<_> = notifications("01-fresh-session");
    assert!(names
        .iter()
        .any(|n| matches!(n, Notification::SessionChanged { .. })));
    assert!(matches!(
        events.last(),
        Some(Event::Notification(Notification::Exit { reason: None }))
    ));
}

#[test]
fn asking_for_a_window_brings_one_that_the_notification_cannot_name() {
    let notes = notifications("02-second-window");
    let added: Vec<_> = notes
        .iter()
        .filter_map(|n| match n {
            Notification::WindowAdd { window } => Some(window.as_str().to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(added, vec!["@1"]);
    // %window-add carries no pane, which is why a listing follows it. The
    // second list-panes reply is the one that names @1's pane.
    let blocks = replies("02-second-window");
    assert_eq!(blocks[0].text().len(), 1);
    let listing = blocks[2].text();
    assert_eq!(listing.len(), 2);
    assert!(listing[1].starts_with("@1 %1 1 "));
}

#[test]
fn a_name_is_decoded_out_of_its_c_escapes() {
    let renamed: Vec<(String, String)> = notifications("03-rename")
        .into_iter()
        .filter_map(|n| match n {
            Notification::WindowRenamed { window, name } => {
                Some((window.as_str().to_string(), name))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        renamed,
        vec![
            // Two consecutive spaces survive: a name is the rest of the line,
            // not re-joined fields.
            ("@0".to_string(), "two  spaces".to_string()),
            ("@1".to_string(), "back\\slash".to_string()),
            ("@1".to_string(), "tab\there bell\x07end".to_string()),
            ("@1".to_string(), "utf8 \u{4f60}\u{597d} end".to_string()),
            // The two bytes that are not UTF-8 came octal-escaped and decode
            // back to themselves; lossy UTF-8 then shows them as replacements,
            // which is the honest answer for a name that is not text.
            ("@1".to_string(), "high \u{fffd}\u{fffd} end".to_string()),
        ]
    );

    // The manual gives %session-renamed one argument. tmux gives two.
    let session_renames: Vec<_> = notifications("03-rename")
        .into_iter()
        .filter_map(|n| match n {
            Notification::SessionRenamed { session, name } => {
                Some((session.as_str().to_string(), name))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        session_renames,
        vec![("$0".to_string(), "sess\\back".to_string())]
    );
}

#[test]
fn all_256_bytes_come_back_off_the_wire() {
    let all: Vec<u8> = (0..=255u8).collect();
    let found = notifications("04-output-octal")
        .into_iter()
        .any(|n| match n {
            Notification::Output { data, .. } => find(&data, &all).is_some(),
            _ => false,
        });
    assert!(found, "the all-bytes payload did not decode to itself");

    // And the backslash, in the other window, from the other escape.
    let backslash = notifications("04-output-octal")
        .into_iter()
        .any(|n| match n {
            Notification::Output { data, .. } => find(&data, b"back\\slash\r\n").is_some(),
            _ => false,
        });
    assert!(backslash, "the escaped backslash did not decode");
}

#[test]
fn a_window_closing_always_says_unlinked() {
    let notes = notifications("05-kill-window");
    let unlinked: Vec<_> = notes
        .iter()
        .filter_map(|n| match n {
            Notification::UnlinkedWindowClose { window } => Some(window.as_str().to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(unlinked, vec!["@1", "@2"]);
    // The name the manual documents for this never appears. Killed from the
    // server and killed through the codec both arrive as "unlinked".
    assert!(
        !notes
            .iter()
            .any(|n| matches!(n, Notification::WindowClose { .. })),
        "tmux 3.5a emitted %window-close after all -- the divergence has moved"
    );
}

#[test]
fn the_server_can_throw_the_client_off() {
    let notes = notifications("06-detach");
    assert!(matches!(
        notes.last(),
        Some(Notification::Exit { reason: None })
    ));
}

#[test]
fn a_reattach_is_told_nothing_about_the_windows_that_are_already_there() {
    // Issue #4's ground. The burst announces no window at all, so a client
    // that learns its windows from notifications comes up empty; the listing
    // is the only source of truth on attach.
    let notes = notifications("07b-reattach");
    assert!(
        !notes.iter().any(|n| matches!(
            n,
            Notification::WindowAdd { .. } | Notification::UnlinkedWindowAdd { .. }
        )),
        "the re-attach announced a window: {notes:?}"
    );
    let blocks = replies("07b-reattach");
    assert_eq!(
        blocks[0].text().len(),
        3,
        "the session should carry the three windows the killed client built"
    );
    assert_eq!(blocks[1].text().len(), 3);

    // And the client that died said nothing on its way out.
    let first = notifications("07a-before-the-kill");
    assert!(
        !first.iter().any(|n| matches!(n, Notification::Exit { .. })),
        "the SIGKILLed client managed an %exit"
    );
}

#[test]
fn a_refused_command_comes_back_as_an_error_block() {
    let blocks = replies("08-error-and-extended-output");
    assert!(blocks[0].error);
    assert!(blocks[0].text()[0].starts_with("parse error: unknown command"));
    assert!(blocks[1..].iter().all(|b| !b.error));

    let extended: Vec<_> = notifications("08-error-and-extended-output")
        .into_iter()
        .filter_map(|n| match n {
            Notification::ExtendedOutput {
                pane, age, data, ..
            } => Some((pane.as_str().to_string(), age, data)),
            _ => None,
        })
        .collect();
    assert!(!extended.is_empty(), "the pause-after form never arrived");
    // The pane is the assertion; the age is not. It is the milliseconds tmux
    // held the output for, and a re-record on a busier machine records a
    // different number for the same protocol.
    assert!(extended.iter().all(|(pane, _, _)| pane == "%0"));
    // The pane runs `cat` with the tty still echoing, so the line goes by
    // twice. Whether tmux packs both into one notification or sends two is a
    // matter of when the reads landed, so the assertion is on the stream, not
    // on where its boundaries fell.
    let data: Vec<u8> = extended.iter().flat_map(|(_, _, d)| d.clone()).collect();
    assert_eq!(data, b"extended\r\nextended\r\n");
}

#[test]
fn an_unquoted_format_is_swallowed_as_a_comment() {
    // The lexer law behind `command::quote_format`, in tmux's own bytes: the
    // same command quoted and unquoted answers two different things, and the
    // unquoted one does not error.
    let blocks = replies("09-quoting");
    let unquoted = &blocks[0].text()[0];
    let quoted = &blocks[1].text()[0];
    assert!(
        !blocks[0].error,
        "unquoted #{{...}} errored; it is worse -- it does not"
    );
    assert_ne!(unquoted, quoted);
    assert!(
        unquoted.contains(':'),
        "expected display-message's default status line, got {unquoted:?}"
    );

    // ${VAR} expands inside double quotes; `\"` and `\\` come through.
    assert_eq!(blocks[2].text()[0], "dollar expanded end");
    assert_eq!(blocks[3].text()[0], "quote \" backslash \\ end");
    // Single quotes stop the lexer, not the format expansion.
    assert_eq!(blocks[4].text()[0], format!("single {quoted} quoted"));
}

#[test]
fn the_zoo_holds_one_of_each() {
    let notes = notifications("10-notification-zoo");
    let mut seen: Vec<&str> = Vec::new();
    for note in &notes {
        seen.push(match note {
            Notification::SessionChanged { .. } => "session-changed",
            Notification::SessionRenamed { .. } => "session-renamed",
            Notification::SessionsChanged => "sessions-changed",
            Notification::UnlinkedWindowAdd { .. } => "unlinked-window-add",
            Notification::UnlinkedWindowRenamed { .. } => "unlinked-window-renamed",
            Notification::WindowRenamed { .. } => "window-renamed",
            Notification::WindowPaneChanged { .. } => "window-pane-changed",
            Notification::LayoutChange { .. } => "layout-change",
            Notification::PaneModeChanged { .. } => "pane-mode-changed",
            Notification::PasteBufferChanged { .. } => "paste-buffer-changed",
            Notification::PasteBufferDeleted { .. } => "paste-buffer-deleted",
            Notification::ClientSessionChanged { .. } => "client-session-changed",
            Notification::ClientDetached { .. } => "client-detached",
            Notification::Exit { .. } => "exit",
            other => panic!("unexpected in the zoo: {other:?}"),
        });
    }
    for wanted in [
        "session-changed",
        "session-renamed",
        "sessions-changed",
        "unlinked-window-add",
        "unlinked-window-renamed",
        "window-renamed",
        "window-pane-changed",
        "layout-change",
        "pane-mode-changed",
        "paste-buffer-changed",
        "paste-buffer-deleted",
        "client-session-changed",
        "client-detached",
        "exit",
    ] {
        assert!(seen.contains(&wanted), "no {wanted} in the zoo");
    }

    // A buffer name is *not* vis-encoded, and neither is a message: both
    // arrive with the single backslash they were given. The window and
    // session names in the same transcript are, which is the whole reason the
    // two decodings are separate.
    assert!(notes.iter().any(|n| matches!(
        n,
        Notification::PasteBufferChanged { name } if name == "buf\\back"
    )));

    // %message arrives inside a reply block, which the manual says cannot
    // happen; so it is a body line here, not a notification.
    let bodies: Vec<String> = replies("10-notification-zoo")
        .iter()
        .flat_map(|b| b.text())
        .collect();
    assert!(
        bodies.iter().any(|line| line == "%message msg \\ back"),
        "%message was not a body line: {bodies:?}"
    );
    assert!(!notes
        .iter()
        .any(|n| matches!(n, Notification::Message { .. })));
}
