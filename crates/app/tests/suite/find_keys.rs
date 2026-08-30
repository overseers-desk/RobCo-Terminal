//! `Ctrl+Shift+F` and the find line, end to end through the surface the
//! binary runs.
//!
//! The same harness as `clipboard_keys.rs`: a real `/bin/sh` on a real pty,
//! a headless `TerminalSurface`, and keys pushed in the way winit pushes
//! them. What is pinned is the whole of the feature from the outside --
//! that the chord raises the line rather than reaching the child, that the
//! query lands on the glass, that Enter and Shift+Enter walk the hits in
//! the scrollback's own coordinates, and that Escape takes both the line
//! and its mark away.

use std::time::{Duration, Instant};

use app::window::TerminalSurface;
use term::{CellSize, SessionConfig, Viewport};
use winit::keyboard::{Key, ModifiersState, NamedKey};

const CELL_W: f32 = 9.0;
const CELL_H: f32 = 18.0;
const WINDOW_W: u32 = 720;
const WINDOW_H: u32 = 400;

const CTRL_SHIFT: ModifiersState = ModifiersState::CONTROL.union(ModifiersState::SHIFT);
const NONE: ModifiersState = ModifiersState::empty();

/// Three lines with the same word on each, then a shell that reads lines
/// back inside brackets, so a key that reached the child says so on the
/// screen.
fn scripted() -> SessionConfig {
    SessionConfig {
        program: Some("/bin/sh".to_string()),
        args: vec![
            "-c".to_string(),
            "printf 'ping one\\nping two\\nping three\\n'; \
             while IFS= read -r l; do echo \"[$l]\"; done"
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
    }
    rate: None,
}

fn surface() -> TerminalSurface {
    let viewport = Viewport::new(WINDOW_W, WINDOW_H, 1.0, CellSize::new(CELL_W, CELL_H));
    TerminalSurface::headless(&scripted(), viewport)
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

fn typed(surface: &mut TerminalSurface, text: &str) {
    for c in text.chars() {
        character(surface, &c.to_string(), NONE);
    }
}

fn named(surface: &mut TerminalSurface, key: NamedKey, modifiers: ModifiersState) {
    surface.key_input(&Key::Named(key), None, modifiers);
}

fn screen(surface: &TerminalSurface) -> String {
    surface.viewport_text().join("\n")
}

#[test]
fn the_find_line_walks_the_hits_and_escape_takes_it_down() {
    let mut surface = surface();
    wait_for(&mut surface, "ping three");
    assert_eq!(surface.marked_range(), None, "nothing is marked yet");

    character(&mut surface, "F", CTRL_SHIFT);
    typed(&mut surface, "ping");
    assert_eq!(surface.find_query(), Some("ping"));
    assert!(
        screen(&surface).contains("Find: ping"),
        "the query is not on the glass\n{}",
        screen(&surface)
    );

    // Enter, from the caret as it stood when the line was raised. Every hit
    // is above that, so the walk wraps once and lands on the first line.
    named(&mut surface, NamedKey::Enter, NONE);
    let first = surface.marked_range().expect("Enter found nothing");
    assert_eq!((first.start, first.end), ((0, 0), (3, 0)));
    assert!(!first.block);

    // Shift+Enter goes the other way, and wraps the other way with it: back
    // past the first hit is the last one. The last one is the third line
    // and not the query on the find line, which is the floor's whole job.
    named(&mut surface, NamedKey::Enter, ModifiersState::SHIFT);
    let back = surface.marked_range().expect("Shift+Enter found nothing");
    assert_eq!((back.start, back.end), ((0, 2), (3, 2)));

    named(&mut surface, NamedKey::Enter, ModifiersState::SHIFT);
    let back = surface.marked_range().expect("and again");
    assert_eq!((back.start, back.end), ((0, 1), (3, 1)));

    named(&mut surface, NamedKey::Escape, NONE);
    assert_eq!(surface.find_query(), None, "the line is still standing");
    assert_eq!(
        surface.marked_range(),
        None,
        "the mark is still on the glass"
    );
}

#[test]
fn a_query_that_is_not_there_marks_nothing() {
    let mut surface = surface();
    wait_for(&mut surface, "ping three");

    character(&mut surface, "F", CTRL_SHIFT);
    typed(&mut surface, "pong");
    named(&mut surface, NamedKey::Enter, NONE);

    assert_eq!(surface.marked_range(), None);
    assert_eq!(
        surface.find_query(),
        Some("pong"),
        "a miss leaves the line standing, to be corrected"
    );
}

/// The chord is the window's, so the child never sees it -- nor anything
/// typed into the line it raises.
#[test]
fn ctrl_shift_f_reaches_the_child_as_nothing() {
    let mut surface = surface();
    wait_for(&mut surface, "ping three");

    character(&mut surface, "a", NONE);
    character(&mut surface, "F", CTRL_SHIFT);
    typed(&mut surface, "ping");
    named(&mut surface, NamedKey::Enter, NONE);
    named(&mut surface, NamedKey::Escape, NONE);
    character(&mut surface, "b", NONE);
    named(&mut surface, NamedKey::Enter, NONE);

    assert_eq!(
        wait_for(&mut surface, "["),
        "[ab]",
        "the chord, the query or its Enter put something on the wire"
    );
}
