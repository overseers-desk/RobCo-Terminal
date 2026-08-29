//! What a piece of art is, and where it stands while it crosses.
//!
//! The art itself is in [`crate::pieces`], which holds string literals and
//! nothing else so that a transcription can be proofread against the program
//! it came from without machinery in the way. This module is the shape of the
//! table and the arithmetic of a crossing.
//!
//! # The box
//!
//! Every piece is transcribed inside a box `width` by `height`, and every
//! frame of it is that box. A frame is one string with newlines rather than a
//! list of rows: it keeps the nesting one level shallower, and it lets the
//! literal in `pieces.rs` be read as the picture it is.
//!
//! [`TRANSPARENT`] is the character that lets what is behind show through,
//! and a line shorter than the box is transparent to the box's right edge.
//!
//! Whether a **space** shows through is the piece's own business
//! ([`Art::solid`]), because the sources disagree and each is right about its
//! own art. The sea pieces are drawn as outlines and rendered with spaces
//! transparent, so the session's text swims behind a whale rather than being
//! erased by its belly; that is how those figures have always looked. The
//! locomotive and Pac-Man are drawn as solid things and written to the screen
//! a whole line at a time, spaces and all, so a boiler is a boiler and the
//! text goes behind it. Reading either kind the other way spoils it.
//!
//! # Speed
//!
//! [`Art::step_ms`] is milliseconds per column of travel, and it is the one
//! number that keeps the promise the whole feature rests on: a critter is
//! uninvited, so it may not sit on a line somebody is reading. A cell is
//! covered for `width * step_ms`, and every piece in the table holds that
//! near a second. It follows that wide art moves fast, which is also how a
//! locomotive ought to look beside a swan.
//!
//! Travel is a function of elapsed time and never an increment per call. The
//! caller's redraw cadence is the user's to set (`general.effects_frame_skip`
//! runs 1 to 10, so 16 ms to 167 ms), and at the slow end a piece advances
//! nine columns between two calls. That is correct rather than something to
//! be smoothed: the piece is where the clock says it is.

/// The character that means "let what is behind show through", spelled as
/// asciiquarium spells it, and for its reason: it is a character none of this
/// art wants to draw, and unlike a space it is visible in a literal.
pub const TRANSPARENT: char = '?';

/// One piece of art: what it is, the box it is drawn in, how fast it walks,
/// and its frames each way round.
#[derive(Debug, PartialEq, Eq)]
pub struct Art {
    /// The piece's name, which is also its key in the `[critters]` table and
    /// the label on its row in the settings window.
    pub name: &'static str,
    /// The box every frame is transcribed inside.
    pub width: u16,
    pub height: u16,
    /// Milliseconds per column of travel.
    pub step_ms: u16,
    /// Whether a space paints a blank cell rather than showing what is
    /// behind. See the module's note: the sea pieces are outlines, the
    /// locomotive and Pac-Man are solid.
    pub solid: bool,
    /// Columns of travel per animation frame. The locomotive's rods turn once
    /// a column, as `sl` draws them; the whale's spout rises over four.
    pub frame_steps: u16,
    /// The frames as drawn travelling right, empty if it was never drawn
    /// that way.
    pub right: &'static [&'static str],
    /// The frames as drawn travelling left, empty if it was never drawn that
    /// way.
    ///
    /// Nothing here is mirrored by machine: an asymmetric figure reversed by
    /// arithmetic reads as a mistake, so a piece its author drew one way
    /// round crosses that way round. Two of the eight are one-way, and both
    /// happen to go left -- Pac-Man is being chased and the locomotive is
    /// `sl`'s, which has always run right to left.
    pub left: &'static [&'static str],
}

impl Art {
    /// Whether this piece may be sent leftwards, and whether rightwards.
    pub fn faces(&self) -> (bool, bool) {
        (!self.right.is_empty(), !self.left.is_empty())
    }

    /// The frames for a facing. Empty only if the caller asked for a facing
    /// the piece was never drawn in, which [`crate::Critters`] does not do.
    fn frames(&self, facing_left: bool) -> &'static [&'static str] {
        if facing_left {
            self.left
        } else {
            self.right
        }
    }
}

/// One crossing in progress.
///
/// Public and `Copy` so a test can build one and walk it by hand, with no
/// clock and no scheduler anywhere in the picture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crossing {
    pub art: &'static Art,
    pub facing_left: bool,
    /// The screen row the box's top line lands on. Signed, and allowed to put
    /// the box off either edge: a piece taller than the screen shows the band
    /// of itself that fits, which is what a train seen through a window is.
    pub top: i32,
    /// Columns travelled since the box stood one column clear of the edge it
    /// came in by.
    pub step: u32,
}

impl Crossing {
    /// Steps in a whole crossing: on from one edge, off past the other.
    pub fn steps(&self, cols: usize) -> u32 {
        cols as u32 + u32::from(self.art.width)
    }

    /// The box's leftmost column, off the screen at both ends of a crossing.
    pub fn column(&self, cols: usize) -> i32 {
        let step = self.step as i32;
        if self.facing_left {
            cols as i32 - step
        } else {
            step - i32::from(self.art.width)
        }
    }

    /// Whether the box is clear of the far edge.
    pub fn done(&self, cols: usize) -> bool {
        self.step >= self.steps(cols)
    }

    /// The rows of the screen this crossing can touch, first and last
    /// inclusive, or `None` when the box is wholly above or below the glass.
    pub fn band(&self, rows: usize) -> Option<(usize, usize)> {
        if rows == 0 {
            return None;
        }
        let last_row = self.top + i32::from(self.art.height) - 1;
        if last_row < 0 || self.top >= rows as i32 {
            return None;
        }
        Some((
            self.top.max(0) as usize,
            (last_row.min(rows as i32 - 1)) as usize,
        ))
    }

    /// Append every cell this crossing wants painted, row-major, clipped to
    /// the screen, transparent cells left out.
    ///
    /// Deterministic: the same crossing on the same size of screen appends
    /// the same cells in the same order, which is what the tests hold it to.
    pub fn paint(&self, cols: usize, rows: usize, out: &mut Vec<(usize, usize, char)>) {
        if self.done(cols) {
            return;
        }
        let frames = self.art.frames(self.facing_left);
        if frames.is_empty() {
            return;
        }
        let per_frame = u32::from(self.art.frame_steps).max(1);
        let frame = frames[((self.step / per_frame) as usize) % frames.len()];
        let left = self.column(cols);
        for (r, line) in frame.lines().enumerate() {
            let row = self.top + r as i32;
            if row < 0 || row >= rows as i32 {
                continue;
            }
            for (c, ch) in line.chars().enumerate() {
                if ch == TRANSPARENT || (ch == ' ' && !self.art.solid) {
                    continue;
                }
                let col = left + c as i32;
                if col < 0 || col >= cols as i32 {
                    continue;
                }
                out.push((row as usize, col as usize, ch));
            }
        }
    }
}

/// The eight, in the order they are drawn from.
///
/// Each row's `step_ms` is set by the rule under **Speed** above, which is
/// why the locomotive is fast and the swan is not.
pub static ART: [Art; 8] = [
    Art {
        name: "dolphins",
        width: 14,
        height: 4,
        step_ms: 70,
        frame_steps: 4,
        solid: false,
        right: &crate::pieces::DOLPHINS_RIGHT,
        left: &crate::pieces::DOLPHINS_LEFT,
    },
    Art {
        name: "ducks",
        width: 31,
        height: 3,
        step_ms: 32,
        frame_steps: 6,
        solid: false,
        right: &crate::pieces::DUCKS_RIGHT,
        left: &crate::pieces::DUCKS_LEFT,
    },
    Art {
        name: "swan",
        width: 11,
        height: 7,
        step_ms: 90,
        frame_steps: 1,
        solid: false,
        right: &crate::pieces::SWAN_RIGHT,
        left: &crate::pieces::SWAN_LEFT,
    },
    Art {
        name: "whale",
        width: 19,
        height: 7,
        step_ms: 52,
        frame_steps: 4,
        solid: false,
        right: &crate::pieces::WHALE_RIGHT,
        left: &crate::pieces::WHALE_LEFT,
    },
    Art {
        name: "ship",
        width: 27,
        height: 6,
        step_ms: 37,
        frame_steps: 1,
        solid: false,
        right: &crate::pieces::SHIP_RIGHT,
        left: &crate::pieces::SHIP_LEFT,
    },
    Art {
        name: "monster",
        width: 66,
        height: 5,
        step_ms: 15,
        frame_steps: 3,
        solid: false,
        right: &crate::pieces::MONSTER_RIGHT,
        left: &crate::pieces::MONSTER_LEFT,
    },
    Art {
        name: "pacman",
        width: 36,
        height: 5,
        step_ms: 28,
        frame_steps: 2,
        solid: true,
        right: &[],
        left: &crate::pieces::PACMAN_LEFT,
    },
    Art {
        name: "locomotive",
        width: 54,
        height: 10,
        step_ms: 18,
        frame_steps: 1,
        solid: true,
        right: &[],
        left: &crate::pieces::D51_LEFT,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Every frame fits the box it says it is drawn in. A transcription that
    /// grew a column would otherwise be found by eye, months later, as a
    /// duck with a clipped beak.
    #[test]
    fn every_frame_fits_its_box() {
        for art in &ART {
            for (facing, frames) in [("right", art.right), ("left", art.left)] {
                for (n, frame) in frames.iter().enumerate() {
                    let lines: Vec<&str> = frame.lines().collect();
                    assert!(
                        lines.len() <= art.height as usize,
                        "{} {facing} frame {n} is {} rows, box is {}",
                        art.name,
                        lines.len(),
                        art.height
                    );
                    for line in lines {
                        assert!(
                            line.chars().count() <= art.width as usize,
                            "{} {facing} frame {n} has a line of {}, box is {}",
                            art.name,
                            line.chars().count(),
                            art.width
                        );
                    }
                }
            }
        }
    }

    /// The frames each piece was drawn with, asserted so that a transcription
    /// cannot quietly lose one.
    #[test]
    fn the_pieces_have_the_frames_they_were_drawn_with() {
        let counts: Vec<(&str, usize, usize)> = ART
            .iter()
            .map(|a| (a.name, a.right.len(), a.left.len()))
            .collect();
        assert_eq!(
            counts,
            vec![
                ("dolphins", 2, 2),
                ("ducks", 3, 3),
                ("swan", 1, 1),
                ("whale", 12, 12),
                ("ship", 1, 1),
                ("monster", 4, 4),
                ("pacman", 0, 4),
                ("locomotive", 0, 6),
            ]
        );
    }

    /// The atlas is built with printable ASCII in hand, so a piece made of it
    /// costs no glyph nobody else was going to ask for. It also catches the
    /// transcription slip that is hardest to see: a non-breaking space, or a
    /// tab, pasted in from a terminal.
    #[test]
    fn every_glyph_is_printable_ascii() {
        for art in &ART {
            for frames in [art.right, art.left] {
                for frame in frames {
                    for ch in frame.chars() {
                        assert!(
                            ch == '\n' || (' '..='~').contains(&ch),
                            "{} carries {ch:?}",
                            art.name
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn every_piece_goes_at_least_one_way() {
        for art in &ART {
            let (right, left) = art.faces();
            assert!(right || left, "{} goes nowhere", art.name);
        }
    }

    /// The rule under **Speed**, asserted of every row of the table.
    #[test]
    fn no_piece_stands_on_a_cell_for_longer_than_a_second() {
        for art in &ART {
            let covered = u32::from(art.width) * u32::from(art.step_ms);
            assert!(
                (900..=1100).contains(&covered),
                "{} covers a cell for {covered} ms",
                art.name
            );
        }
    }

    #[test]
    fn a_transparent_cell_is_left_alone_and_a_space_is_the_pieces_business() {
        static OUTLINE: Art = Art {
            name: "outline",
            width: 3,
            height: 1,
            step_ms: 1,
            frame_steps: 1,
            solid: false,
            right: &["a b"],
            left: &[],
        };
        static SOLID: Art = Art {
            name: "solid",
            width: 3,
            height: 1,
            step_ms: 1,
            frame_steps: 1,
            solid: true,
            right: &["a b"],
            left: &[],
        };
        let cells = |art: &'static Art| {
            let mut out = Vec::new();
            Crossing {
                art,
                facing_left: false,
                top: 0,
                step: 3,
            }
            .paint(10, 1, &mut out);
            out
        };
        assert_eq!(cells(&OUTLINE), vec![(0, 0, 'a'), (0, 2, 'b')]);
        assert_eq!(cells(&SOLID), vec![(0, 0, 'a'), (0, 1, ' '), (0, 2, 'b')]);
    }
}
