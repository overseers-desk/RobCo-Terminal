//! The find line `Ctrl+Shift+F` raises, and the walk through the scrollback
//! it drives.
//!
//! The line itself is [`crate::prompt::Line`], the same editor an SSH
//! question is answered into, painted on the channel's own grid through the
//! same [`crate::prompt::paint`]. So the query wears the phosphor and the
//! curvature like everything else, and there is no second widget anywhere:
//! a find line is a question the terminal asks itself.
//!
//! The search is rio-vt's. `Crosswords::search_next` already runs a regex
//! over the grid the emulation core holds, in both directions, wrapping once
//! when it reaches the end -- the whole of what a find line needs. This
//! module is the seam either side of it: `regex::escape` and `(?i)` turning
//! typed text into a literal case-insensitive pattern going in, and rio's
//! screen-relative `Pos` turning into an absolute [`MarkedRange`] coming
//! out, the conversion [`term::selection::rio`] makes for a selection.
//!
//! # The floor
//!
//! The query is on the grid, because painting it there is what put it in
//! front of the eye. So the grid contains the text being looked for, and a
//! search that ignored the fact would hand back the find line reading itself
//! aloud. [`Find::floor`] is the first line the find line occupies, and a
//! hit at or below it is discarded and stepped over rather than shown.

use term::rio_vt::crosswords::pos::{Boundary, Column, Direction, Line, Pos, Side};
use term::rio_vt::crosswords::search::{Match, RegexSearch};
use term::rio_vt::crosswords::Crosswords;
use term::rio_vt::event::EventListener;

use term::MarkedRange;

use crate::channels::BankId;

/// What the glass says before the answer. The leading newline gives the
/// query a row of its own, so raising the line does not write over the
/// prompt the shell had already drawn.
pub const PROMPT: &str = "\nFind: ";

/// How many hits on the find line's own rows are stepped over before the
/// search gives up. The query appears there once, and can wrap onto a
/// second row; four is that with room to spare, and it is a bound rather
/// than an estimate, so a pathological grid cannot spin here.
const SELF_HITS: usize = 4;

/// The find line while it stands.
pub struct Find {
    /// The query as it is being typed. Echoing, unlike a passphrase: the
    /// whole point of a find line is that you can see what you asked for.
    pub line: crate::prompt::Line,
    /// The bank and channel the line was raised on. A find line belongs to
    /// the scrollback it is searching, so turning the knob leaves it behind
    /// rather than aiming it at another channel's history.
    pub on: (BankId, u32),
    /// The query the DFAs below were built from, so a keystroke that did
    /// not change the query does not rebuild them.
    pattern: String,
    regex: Option<RegexSearch>,
    /// Where a search with nothing found yet starts: the cursor as it stood
    /// before the line was painted over it, in absolute coordinates.
    caret: (usize, usize),
    /// The first absolute line the find line itself occupies. See the
    /// module doc.
    floor: usize,
    /// The hit on the glass, if a step has found one.
    mark: Option<MarkedRange>,
}

impl Find {
    /// A line raised on `on`, searching from `caret`, with the query drawn
    /// at `floor`.
    pub fn new(on: (BankId, u32), caret: (usize, usize), floor: usize) -> Self {
        Self {
            line: crate::prompt::Line::new(true),
            on,
            pattern: String::new(),
            regex: None,
            caret,
            floor,
            mark: None,
        }
    }

    /// Where the query itself now sits, once it has been painted.
    ///
    /// A line that opens with a remembered query paints the prompt and the
    /// query in two goes, so the floor is only known after the second: the
    /// rows to step over are the rows the whole line ended up on.
    pub fn set_floor(&mut self, floor: usize) {
        self.floor = floor;
    }

    /// The hit the glass is painting, or `None` before the first Enter and
    /// after a miss.
    pub fn mark(&self) -> Option<MarkedRange> {
        self.mark
    }

    /// What has been typed so far.
    pub fn query(&self) -> &str {
        self.line.shown()
    }

    /// One Enter's worth of searching: the next hit after the last one, or
    /// after the caret when there is no last one, wrapping once.
    ///
    /// `forward` is Enter and down the history; Shift+Enter is the other
    /// way. A miss leaves the mark where it was rather than clearing it:
    /// the user asked for another hit and there is not one, which is not a
    /// reason to take the hit they are looking at off the glass.
    pub fn step<U: EventListener>(
        &mut self,
        term: &Crosswords<U>,
        forward: bool,
    ) -> Option<MarkedRange> {
        let query = self.line.shown();
        if query.is_empty() {
            return None;
        }
        // rio-vt builds its own case-insensitivity from whether the pattern
        // has an uppercase letter in it, which is smart case rather than the
        // case-insensitive find both GNOME Terminal and Konsole open with.
        // The inline flag is what says so outright.
        let pattern = format!("(?i){}", regex::escape(query));
        if pattern != self.pattern {
            // A pattern that will not compile is not a state to report: it
            // is `regex::escape`'s output, so the only way here is a
            // resource limit inside the DFA builder.
            self.regex = match RegexSearch::new(&pattern) {
                Ok(regex) => Some(regex),
                Err(e) => {
                    log::warn!("could not build the search for {query:?}: {e}");
                    None
                }
            };
            self.pattern = pattern;
            self.mark = None;
        }

        let floor = self.floor;
        let from = match self.mark {
            Some(mark) => {
                let cell = if forward { mark.end } else { mark.start };
                let pos = pos_of(cell, term.history_size());
                // Past the hit the eye is on, or the same one comes back.
                if forward {
                    pos.add(term, Boundary::Grid, 1)
                } else {
                    pos.sub(term, Boundary::Grid, 1)
                }
            }
            None => pos_of(self.caret, term.history_size()),
        };
        let direction = if forward {
            Direction::Right
        } else {
            Direction::Left
        };
        // Which end of a hit is compared against the origin: the near one,
        // whichever way the search is going.
        let side = if forward { Side::Left } else { Side::Right };

        let regex = self.regex.as_mut()?;
        let mut origin = from;
        for _ in 0..SELF_HITS {
            let hit = term.search_next(regex, origin, direction, side, None)?;
            let range = marked(term, &hit);
            if range.start.1 < floor {
                self.mark = Some(range);
                return Some(range);
            }
            origin = if forward {
                hit.end().add(term, Boundary::Grid, 1)
            } else {
                hit.start().sub(term, Boundary::Grid, 1)
            };
        }
        None
    }
}

/// The cursor, in the absolute coordinates everything above [`term::grid`]
/// speaks.
pub fn caret<U: EventListener>(term: &Crosswords<U>) -> (usize, usize) {
    let pos = term.grid.cursor.pos;
    (pos.col.0, absolute(pos.row, term.history_size()))
}

/// A hit as the renderer is told it: absolute lines, and never a block,
/// because a run of text found in the scrollback wraps the way the text it
/// was found in wraps.
fn marked<U: EventListener>(term: &Crosswords<U>, hit: &Match) -> MarkedRange {
    let history = term.history_size();
    MarkedRange {
        start: (hit.start().col.0, absolute(hit.start().row, history)),
        end: (hit.end().col.0, absolute(hit.end().row, history)),
        block: false,
    }
}

fn absolute(row: Line, history: usize) -> usize {
    (row.0 + history as i32).max(0) as usize
}

fn pos_of(cell: (usize, usize), history: usize) -> Pos {
    Pos::new(Line(cell.1 as i32 - history as i32), Column(cell.0))
}
