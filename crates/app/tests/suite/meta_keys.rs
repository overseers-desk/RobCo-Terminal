//! Alt as Meta, end to end through the surface the binary runs.
//!
//! `app::input` pins which bytes each key produces, but not that they leave
//! the window: the escape is put on in two places, one of them below the
//! keytab and neither reachable from a pure function. The child here reads a
//! line and prints it back through `cat -v`, so an escape that arrived shows
//! up as `^[` inside brackets. The wait watches for the closing bracket,
//! because the tty echoes an escape as `^[` as well and never carries one.

use std::time::{Duration, Instant};

use app::window::TerminalSurface;
use term::{CellSize, SessionConfig, Viewport};
use winit::keyboard::{Key, ModifiersState, NamedKey};

const NONE: ModifiersState = ModifiersState::empty();
const ALT: ModifiersState = ModifiersState::ALT;
/// The channel chord's own modifier, which is not Alt on a Mac.
#[cfg(target_os = "macos")]
const CHORD: ModifiersState = ModifiersState::SUPER;
#[cfg(not(target_os = "macos"))]
const CHORD: ModifiersState = ModifiersState::ALT;
/// What `Alt+.` leaves the child holding: the escape and the key, or on a
/// Mac the key by itself, Option being the character-composing key there.
#[cfg(target_os = "macos")]
const ALT_DOT: &str = "[.]";
#[cfg(not(target_os = "macos"))]
const ALT_DOT: &str = "[^[.]";

/// A shell that reads a line and shows what it read with its control
/// characters spelled out.
fn scripted() -> SessionConfig {
    SessionConfig {
        program: Some("/bin/sh".to_string()),
        args: vec![
            "-c".to_string(),
            "echo ready; while IFS= read -r l; do printf '[%s]\\n' \"$l\" | cat -v; done"
                .to_string(),
            String::new(),
        ],
        working_directory: None,
        env: vec![
            ("TERM".to_string(), "xterm-256color".to_string()),
            ("ENV".to_string(), String::new()),
        ],
        scrollback: 200,
        grapheme_clustering: false,
        rate: None,
    }
}

fn surface() -> TerminalSurface {
    let viewport = Viewport::new(720, 400, 1.0, CellSize::new(9.0, 18.0));
    let mut surface = TerminalSurface::headless(&scripted(), viewport);
    wait_for(&mut surface, "ready");
    surface
}

fn wait_for(surface: &mut TerminalSurface, text: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        surface.pump();
        if let Some(line) = surface.viewport_text().iter().find(|l| l.contains(text)) {
            return line.trim().to_string();
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!(
        "timed out waiting for {text:?}\n--- screen ---\n{}",
        surface.viewport_text().join("\n")
    );
}

fn character(surface: &mut TerminalSurface, c: &str, modifiers: ModifiersState) {
    surface.key_input(&Key::Character(c.into()), Some(c), modifiers);
}

fn enter(surface: &mut TerminalSurface) {
    surface.key_input(&Key::Named(NamedKey::Enter), None, NONE);
}

/// `Alt+.` is readline's last argument, and this is the whole of what makes
/// it one: the escape in front of the key.
#[test]
fn alt_and_a_key_reach_the_child_as_an_escape_and_the_key() {
    let mut surface = surface();
    character(&mut surface, ".", ALT);
    enter(&mut surface);
    assert_eq!(wait_for(&mut surface, "]"), ALT_DOT);
}

/// The digits are the appliance's own, drawn bank or none, so nothing of
/// them reaches the child: not the escape the key would otherwise carry, and
/// not the digit either.
#[test]
fn a_digit_chord_leaves_nothing_behind_for_the_child() {
    let mut surface = surface();
    character(&mut surface, "1", CHORD);
    character(&mut surface, "x", NONE);
    enter(&mut surface);
    assert_eq!(wait_for(&mut surface, "]"), "[x]");
}
