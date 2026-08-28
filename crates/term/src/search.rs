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
//! falls after and measuring the display width of the text between the two:
//! a column is where a character is drawn, which is neither its byte offset
//! nor its index once a line holds a wide character.

use regex::Regex;

use crate::grid::{char_width, string_width, write_lines_recording_positions, GridView};

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
            Some(col) if lines_in_string > 0 => {
                offset_at(&text, line_positions[lines_in_string - 1], col)
            }
            _ => text.len(),
        };

        // The caret's column is measured on the block's first line, which is
        // where the caret sits in both phases: forwards the block starts at
        // the caret's line, backwards the range ends there.
        let caret = offset_at(&text, 0, start_column);
        let hit = if forwards {
            re.find_at(&text, caret.min(text.len()))
                .filter(|m| m.start() < end_position)
        } else {
            let limit = end_position.saturating_sub(1);
            re.find_iter(&text)
                .take_while(|m| m.start() <= limit)
                .last()
                .filter(|m| m.start() >= caret)
        };

        if let Some(m) = hit {
            let match_start = m.start();
            // Where the match's last character starts, not the byte after
            // the match: a highlight wants the cell the match ends on, and
            // one byte back from the end lands inside a character whenever
            // that character is not ASCII.
            let match_end = text[..m.end()]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(match_start);

            let start_line_in_string = find_line_number(&line_positions, match_start);
            let end_line_in_string = find_line_number(&line_positions, match_end);

            // Translated against the block's own first line, not by
            // `start_line + lines_read`: that formula gives the same number
            // in the forwards phase but is wrong by a block in the backwards
            // one; the divergence is invisible below 10,000 lines of history
            // and wrong above it.
            return Some(SearchHit {
                start_column: column_at(&text, line_positions[start_line_in_string], match_start),
                start_line: block_start_line + start_line_in_string,
                end_column: column_at(&text, line_positions[end_line_in_string], match_end),
                end_line: block_start_line + end_line_in_string,
            });
        }

        lines_read += block_size;
    }
}

/// The grid column `offset` sits at, on the line starting at `line_start`.
///
/// The text is one character per cell the grid drew, so the column is the
/// display width of what precedes the offset on its line.
fn column_at(text: &str, line_start: usize, offset: usize) -> usize {
    string_width(&text[line_start..offset])
}

/// The inverse: where in `text` the caret sitting at `column` on the line
/// starting at `line_start` is, so a column from the glass can be handed to
/// a regex that counts bytes.
fn offset_at(text: &str, line_start: usize, column: usize) -> usize {
    let mut width = 0usize;
    for (i, c) in text[line_start..].char_indices() {
        if c == '\n' || width >= column {
            return line_start + i;
        }
        width += char_width(c).max(0) as usize;
    }
    text.len()
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
