//! When a critter comes, which one, and where it walks.
//!
//! A state machine over a clock the caller owns, in the shape `app::overlay`
//! and `crt::Pacing` already use here, so a test drives a thousand hours
//! through it in a millisecond.
//!
//! One interval brings one critter. Intervals are counted from local
//! midnight, so an interval dividing the hour lands on the clock's own marks:
//! at the shipped quarter hour, :00, :15, :30 and :45, and catching one tells
//! the time. With `jitter` the arrival is placed at a random point inside its
//! interval instead of at the start, so it stays unpredictable while still
//! coming once an interval.
//!
//! An interval joined late brings nothing. That is the whole of what happens
//! when nobody is at the terminal: the moment passed with no one to see it,
//! and a moment nobody saw is missed rather than owed. Without that rule an
//! arrival waits at the moment you come back, and since a window counts as
//! unattended as soon as it loses the keyboard, every return is greeted.
//!
//! Any row, the cursor's included. A shell writes at the bottom and a
//! full-screen program everywhere, so there is no quiet half to prefer, and
//! the speed rule in [`crate::art`] is what makes a free choice of row safe.

use std::time::{Duration, Instant};

use chrono::Timelike;

use crate::art::{Art, Crossing, ART};
use crate::rng;

/// How late an interval may be joined and still bring a critter. Frames
/// arrive every 16 to 167 ms, so an interval watched from its start is joined
/// well inside this; one joined later than this was not being watched.
const JOINED: Duration = Duration::from_secs(1);

/// Which interval the local clock is in, and how far into it, counting from
/// midnight. Local rather than UTC: the mark has to be the one on the clock
/// in the room.
fn interval_now(every: Duration) -> (u64, Duration) {
    let now = chrono::Local::now();
    let secs =
        u64::from(now.hour()) * 3600 + u64::from(now.minute()) * 60 + u64::from(now.second());
    let every = every.as_secs().max(1);
    (secs / every, Duration::from_secs(secs % every))
}

/// One window's critters.
pub struct Critters {
    rng: u64,
    enabled: bool,
    /// One critter per interval of this.
    every: Duration,
    /// Whether the arrival is placed at random inside its interval rather
    /// than at the start of it.
    jitter: bool,
    /// The pieces left switched on, by index in [`ART`]. All of them off is
    /// the same silence as `enabled` false, and reached the same way.
    allowed: [bool; ART.len()],
    /// The interval this has planned for.
    planned: Option<u64>,
    /// How far into that interval the critter is due, or `None` where the
    /// interval brings none: it was joined late, or its critter has been.
    due: Option<Duration>,
    /// The crossing in progress, and the instant it began.
    active: Option<(Crossing, Instant)>,
    /// The cells as of the last tick, row-major.
    cells: Vec<(usize, usize, char)>,
    /// The cells as of the tick before, swapped with them each tick so
    /// [`Critters::tick`] compares rather than guesses.
    prev: Vec<(usize, usize, char)>,
}

impl Critters {
    /// A window's own. Two given the same seed and the same ticks paint the
    /// same cells for ever, which is what the tests stand on.
    pub fn new(
        seed: u64,
        enabled: bool,
        every: Duration,
        jitter: bool,
        allowed: [bool; ART.len()],
    ) -> Self {
        Self {
            rng: seed,
            enabled,
            every,
            jitter,
            allowed,
            planned: None,
            due: None,
            active: None,
            cells: Vec::new(),
            prev: Vec::new(),
        }
    }

    /// The settings, re-applied when the config file changes.
    pub fn configure(
        &mut self,
        enabled: bool,
        every: Duration,
        jitter: bool,
        allowed: [bool; ART.len()],
    ) {
        self.enabled = enabled;
        self.every = every;
        self.jitter = jitter;
        self.allowed = allowed;
    }

    /// Advance to `now` on a screen this size, and say whether the cells
    /// differ from the last tick.
    pub fn tick(&mut self, now: Instant, cols: usize, rows: usize) -> bool {
        std::mem::swap(&mut self.cells, &mut self.prev);
        self.cells.clear();

        if !self.enabled || cols == 0 || rows == 0 {
            // A screen with no cells cannot host a crossing.
            self.active = None;
        } else {
            self.advance(now, cols, rows);
        }

        if let Some((crossing, _)) = self.active {
            crossing.paint(cols, rows, &mut self.cells);
        }
        // The cells and not a count: a piece stepping one column across the
        // middle of the glass paints the same number in the same rows.
        self.cells != self.prev
    }

    /// Take whatever is crossing off the glass, and say whether the glass
    /// changed by it.
    ///
    /// The interval it came in is already spent, so nothing is owed for it.
    /// The cells go and the caller paints the frame taking them off, which is
    /// what INVARIANTS.md asks.
    pub fn withdraw(&mut self) -> bool {
        self.active = None;
        let had = !self.cells.is_empty();
        self.prev.clear();
        self.prev.extend_from_slice(&self.cells);
        self.cells.clear();
        had
    }

    /// Start a crossing if one is due, or carry the one in hand forward.
    fn advance(&mut self, now: Instant, cols: usize, rows: usize) {
        if let Some((crossing, began)) = self.active.as_mut() {
            let ms = now.saturating_duration_since(*began).as_millis();
            crossing.step = (ms / u128::from(crossing.art.step_ms.max(1))) as u32;
            if crossing.done(cols) {
                self.active = None;
            }
            return;
        }
        let (index, into) = interval_now(self.every);
        if self.planned != Some(index) {
            self.planned = Some(index);
            self.due = (into < JOINED).then(|| self.offset());
        }
        if self.due.is_some_and(|due| into >= due) {
            self.active = self.cast(rows).map(|c| (c, now));
            self.due = None;
        }
    }

    /// Where in its interval this one arrives: the start, or anywhere inside
    /// it when jittered.
    fn offset(&mut self) -> Duration {
        if !self.jitter {
            return Duration::ZERO;
        }
        Duration::from_millis(rng::below(
            &mut self.rng,
            self.every.as_millis().max(1) as u64,
        ))
    }

    /// Choose a piece, a facing and a row. Uniform over the pieces still
    /// switched on, so retiring the locomotive makes the others likelier
    /// rather than leaving a hole.
    fn cast(&mut self, rows: usize) -> Option<Crossing> {
        let on: Vec<&'static Art> = ART
            .iter()
            .zip(self.allowed)
            .filter_map(|(art, on)| on.then_some(art))
            .collect();
        if on.is_empty() {
            return None;
        }
        let art = on[rng::below(&mut self.rng, on.len() as u64) as usize];
        let facing_left = match art.faces() {
            (true, true) => rng::below(&mut self.rng, 2) == 1,
            (true, false) => false,
            (false, true) => true,
            (false, false) => return None,
        };
        let height = i32::from(art.height);
        let top = if height <= rows as i32 {
            // It fits: stand it whole, anywhere it fits whole.
            rng::below(&mut self.rng, (rows as i32 - height + 1) as u64) as i32
        } else {
            // Taller than the glass: centred, both ends off the screen.
            rng::below(&mut self.rng, rows as u64) as i32 - height / 2
        };
        Some(Crossing {
            art,
            facing_left,
            top,
            step: 0,
        })
    }

    /// The cells to paint, row-major. Empty when the glass is its own again.
    pub fn cells(&self) -> &[(usize, usize, char)] {
        &self.cells
    }

    /// The name of the piece crossing, for a test and for a log line.
    pub fn crossing(&self) -> Option<&'static str> {
        self.active.map(|(c, _)| c.art.name)
    }

    /// The next column of a crossing in hand, the moment this interval's
    /// critter is due, or the start of the next interval. The caller folds it
    /// into the deadline it already keeps.
    pub fn wake_at(&self, now: Instant) -> Option<Instant> {
        if !self.enabled {
            return None;
        }
        if let Some((crossing, began)) = self.active {
            let step = u64::from(crossing.step + 1) * u64::from(crossing.art.step_ms.max(1));
            return Some(began + Duration::from_millis(step));
        }
        let (_, into) = interval_now(self.every);
        let ahead = match self.due {
            Some(due) if due > into => due - into,
            _ => self.every.saturating_sub(into),
        };
        Some(now + ahead)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_quarter_hours_are_where_the_clock_says() {
        // Counting from midnight, an interval dividing the hour puts every
        // boundary on a mark of the clock: 96 quarter hours in a day, and the
        // one in hand is where the wall clock says it is.
        let quarter = Duration::from_secs(900);
        let (index, into) = interval_now(quarter);
        assert!(index < 96, "{index} is not an interval of today");
        assert!(into < quarter);
        let clock = chrono::Local::now();
        let minutes = u64::from(clock.hour()) * 60 + u64::from(clock.minute());
        assert_eq!(index, minutes / 15);
    }

    #[test]
    fn an_interval_of_nothing_is_a_second() {
        let (_, into) = interval_now(Duration::ZERO);
        assert!(into < Duration::from_secs(1));
    }
}
