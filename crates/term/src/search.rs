//! Scrollback search.
//!
//! The algorithm is a wrapping two-phase scan. Searching forwards, it looks
//! from the caret to the end of history first and then from the beginning of
//! history back round to the caret; searching backwards, the same two ranges
//! in the other order. Whichever phase hits first wins, so a search always
//! finds the *nearest* match in the direction asked for and wraps exactly
//! once.
//!
//! History is read in blocks of at most 10,000 lines, decoded to one string
//! per block with the offset of each line recorded, and the regex run over
//! that string. Match offsets come back as string positions and are turned
//! into `(column, line)` by finding which line's recorded offset the position
//! falls after.

use regex::Regex;

use crate::grid::{write_lines_recording_positions, GridView};

/// At most this many lines are decoded into one string at a time, so a
/// search never has to hold all of scrollback in memory at once.
const BLOCK_LINES: usize = 10_000;

/// Where a search landed, in absolute grid coordinates.
///
/// `end_column`/`end_line` address the last character *of* the match, not
/// one past it -- that's what a highlight wants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchHit {
    pub start_column: usize,
    pub start_line: usize,
    pub end_column: usize,
    pub end_line: usize,
}

/// Search the whole grid from a caret, wrapping once.
///
/// `start_line` is absolute and `start_column` is the caret's column on it.
pub fn search(
    grid: &impl GridView,
    re: &Regex,
    forwards: bool,
    start_column: usize,
    start_line: usize,
) -> Option<SearchHit> {
    if re.as_str().is_empty() {
        return None;
    }
    // `HistorySearch` passed `lineCount()` as the end line, one past the last
    // valid index, and relied on `Screen::writeToStream` tolerating it. We
    // clamp instead: reading a line that does not exist is a panic here, and
    // an off-by-one in the bound cannot change which match is found.
    let last_line = grid.total_lines().saturating_sub(1);

    if forwards {
        search_range(grid, re, true, start_column, start_line, None, last_line)
            .or_else(|| search_range(grid, re, true, 0, 0, Some(start_column), start_line))
    } else {
        search_range(grid, re, false, 0, 0, Some(start_column), start_line)
            .or_else(|| search_range(grid, re, false, start_column, start_line, None, last_line))
    }
}

/// One phase of the wrap: `HistorySearch::search(startColumn, startLine,
/// endColumn, endLine)`. `end_column == None` is the `-1` sentinel, "to the
/// end of the last line".
fn search_range(
    grid: &impl GridView,
    re: &Regex,
    forwards: bool,
    start_column: usize,
    start_line: usize,
    end_column: Option<usize>,
    end_line: usize,
) -> Option<SearchHit> {
    if end_line < start_line {
        return None;
    }
    let last_line = grid.total_lines().saturating_sub(1);
    let end_line = end_line.min(last_line);
    let lines_to_read = end_line - start_line + 1;
    let mut lines_read = 0usize;

    loop {
        let block_size = BLOCK_LINES.min(lines_to_read - lines_read);
        if block_size == 0 {
            return None;
        }

        // Forwards walks the range from its start; backwards walks it from
        // its end, so that the first block examined is the one nearest the
        // caret in the direction of travel.
        let block_start_line = if forwards {
            start_line + lines_read
        } else {
            (end_line + 1) - lines_read - block_size
        };
        let chunk_end_line = block_start_line + block_size - 1;

        let (text, line_positions) =
            write_lines_recording_positions(grid, block_start_line, chunk_end_line);

        // The block string ends with a newline and so with an empty line;
        // that trailing chunk gets a recorded position too, and is ignored.
        let lines_in_string = line_positions.len().saturating_sub(1);
        let end_position = match end_column {
            Some(col) if lines_in_string > 0 => line_positions[lines_in_string - 1] + col,
            _ => text.len(),
        };

        // `startColumn` is used directly as a string offset. That is correct
        // here, not a shortcut needing an excuse: the first line of the block
        // starts at offset 0, and the caret's line is always the block's
        // first line in the forwards phase.
        let hit = if forwards {
            re.find_at(&text, start_column.min(text.len()))
                .filter(|m| m.start() < end_position)
        } else {
            let limit = end_position.saturating_sub(1);
            re.find_iter(&text)
                .take_while(|m| m.start() <= limit)
                .last()
                .filter(|m| m.start() >= start_column)
        };

        if let Some(m) = hit {
            let match_start = m.start();
            let match_end = m.end().saturating_sub(1);

            let start_line_in_string = find_line_number(&line_positions, match_start);
            let end_line_in_string = find_line_number(&line_positions, match_end);

            // Translated against the block's own first line, not by
            // `start_line + lines_read`: that formula gives the same number
            // in the forwards phase but is wrong by a block in the backwards
            // one; the divergence is invisible below 10,000 lines of history
            // and wrong above it.
            return Some(SearchHit {
                start_column: match_start - line_positions[start_line_in_string],
                start_line: block_start_line + start_line_in_string,
                end_column: match_end - line_positions[end_line_in_string],
                end_line: block_start_line + end_line_in_string,
            });
        }

        lines_read += block_size;
    }
}

/// `HistorySearch::findLineNumberInString`: the last line whose recorded
/// offset is at or before `position`.
fn find_line_number(line_positions: &[usize], position: usize) -> usize {
    let mut line = 0usize;
    while line + 1 < line_positions.len() && line_positions[line + 1] <= position {
        line += 1;
    }
    line
}

/// Build the regex a plain-text search box means: the typed text as a
/// literal, optionally ignoring case.
pub fn literal_pattern(text: &str, case_sensitive: bool) -> Regex {
    let escaped = regex::escape(text);
    let pattern = if case_sensitive {
        escaped
    } else {
        format!("(?i){escaped}")
    };
    Regex::new(&pattern).expect("an escaped literal is always a valid pattern")
}
