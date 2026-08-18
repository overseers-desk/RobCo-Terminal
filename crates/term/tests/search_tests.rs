//! Scrollback search: exact hit positions, in both directions and across the
//! wrap. The positions are the point: a search that finds the right line
//! but the wrong column highlights the wrong thing.

use term::grid::ScriptedGrid;
use term::search::{literal_pattern, search};
use term::selection::Selection;

const COLS: usize = 40;

fn search_grid() -> ScriptedGrid {
    ScriptedGrid::with_history(
        COLS,
        3,
        &[
            "error: first failure",  // 0, history
            "warning: nothing here", // 1, history
            "error: second failure", // 2, history
            "all good now",          // 3, screen
            "error: third failure",  // 4, screen
            "done",                  // 5, screen
        ],
    )
}

#[test]
fn search_forwards_finds_the_next_hit_after_the_caret() {
    let g = search_grid();
    let re = literal_pattern("error", true);
    let hit = search(&g, &re, true, 0, 1).expect("a hit");
    assert_eq!((hit.start_line, hit.start_column), (2, 0));
    assert_eq!(
        (hit.end_line, hit.end_column),
        (2, 4),
        "the end addresses the last character of the match, not one past it"
    );
}

#[test]
fn search_forwards_wraps_round_to_the_top() {
    let g = search_grid();
    let re = literal_pattern("error", true);
    let hit = search(&g, &re, true, 0, 5).expect("a hit after the wrap");
    assert_eq!((hit.start_line, hit.start_column), (0, 0));
}

#[test]
fn search_backwards_finds_the_previous_hit() {
    let g = search_grid();
    let re = literal_pattern("error", true);
    let hit = search(&g, &re, false, 0, 4).expect("a hit");
    assert_eq!(
        (hit.start_line, hit.start_column),
        (2, 0),
        "the nearest match above the caret, not the first one in the buffer"
    );
}

#[test]
fn search_finds_a_hit_that_is_not_at_column_zero() {
    let g = search_grid();
    let re = literal_pattern("failure", true);
    let hit = search(&g, &re, true, 0, 0).expect("a hit");
    assert_eq!((hit.start_line, hit.start_column), (0, 13));
    assert_eq!((hit.end_line, hit.end_column), (0, 19));
}

#[test]
fn search_misses_cleanly() {
    let g = search_grid();
    let re = literal_pattern("no such text anywhere", true);
    assert!(search(&g, &re, true, 0, 0).is_none());
    assert!(search(&g, &re, false, 0, 0).is_none());
}

#[test]
fn search_is_case_sensitive_only_when_asked() {
    let g = ScriptedGrid::new(COLS, &["Segmentation Fault"]);
    assert!(search(&g, &literal_pattern("fault", true), true, 0, 0).is_none());
    let hit = search(&g, &literal_pattern("fault", false), true, 0, 0).expect("a hit");
    assert_eq!((hit.start_line, hit.start_column), (0, 13));
}

#[test]
fn search_takes_a_real_regex_too() {
    let g = search_grid();
    let re = regex::Regex::new(r"\b\w+ failure").expect("pattern");
    let hit = search(&g, &re, true, 0, 3).expect("a hit");
    assert_eq!((hit.start_line, hit.start_column), (4, 7));
    assert_eq!((hit.end_line, hit.end_column), (4, 19));
}

#[test]
fn search_hit_positions_address_the_grid_the_selection_reads() {
    // The point of exact positions: feed them to a selection and the text
    // that comes back must be the text that was searched for.
    let g = search_grid();
    let re = literal_pattern("second", true);
    let hit = search(&g, &re, true, 0, 0).expect("a hit");

    let mut s = Selection::new(COLS);
    s.set_start(hit.start_column, hit.start_line, false);
    s.set_end(hit.end_column, hit.end_line);
    assert_eq!(s.selected_text(&g, true).as_deref(), Some("second"));
}

#[test]
fn a_match_that_runs_across_a_line_break_reports_both_lines() {
    let g = ScriptedGrid::new(COLS, &["ends with foo", "bar starts"]);
    let re = regex::Regex::new(r"foo\nbar").expect("pattern");
    let hit = search(&g, &re, true, 0, 0).expect("a hit");
    assert_eq!((hit.start_line, hit.start_column), (0, 10));
    assert_eq!((hit.end_line, hit.end_column), (1, 2));
}
