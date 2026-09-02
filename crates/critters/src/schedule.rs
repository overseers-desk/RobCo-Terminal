//! When a critter comes, which one, and where it walks.
//!
//! A state machine over a clock the caller owns, in the shape `app::overlay`
//! and `crt::Pacing` already use here, so a test drives a thousand hours
//! through it in a millisecond.
//!
//! [`Timing::Clock`] puts an arrival on each wall-clock mark: at the shipped
//! quarter hour, :00, :15, :30 and :45, so catching one tells the time. The
//! hour is always a mark, so an interval that does not divide it takes a
//! short last step rather than sliding round the clock. [`Timing::Random`]
//! draws its wait instead ([`crate::rng::wait`]), and nothing about one
//! arrival says when the next is. Either way the next is settled once the
//! glass is clear of the last, so two never overlap.
//!
//! Any row, the cursor's included. A shell writes at the bottom and a
//! full-screen program everywhere, so there is no quiet half to prefer, and
//! the speed rule in [`crate::art`] is what makes a free choice of row safe.

use std::time::{Duration, Instant};

use chrono::Timelike;

use crate::art::{Art, Crossing, ART};
use crate::rng;

/// What settles when the next critter comes. An interval and an average are
/// the same kind of number meaning opposite things, so it travels inside the
/// answer to which it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Timing {
    /// On every wall-clock multiple of this past the hour.
    Clock(Duration),
    /// At random, this long between arrivals on average.
    Random(Duration),
}

/// How long until the next mark, marks falling on every multiple of `every`
/// past the hour. The hour is always one: an interval that does not divide it
/// would otherwise walk its marks round the clock, and a mark that moves is
/// no mark.
fn until_mark(past_the_hour: Duration, every: Duration) -> Duration {
    let hour = Duration::from_secs(3600);
    let every = every.max(Duration::from_secs(1));
    let since = Duration::from_nanos((past_the_hour.as_nanos() % every.as_nanos()) as u64);
    (every - since).min(hour.saturating_sub(past_the_hour))
}

/// How far into the hour the local wall clock is: the mark has to be the one
/// on the clock in the room.
fn past_the_hour() -> Duration {
    let now = chrono::Local::now();
    Duration::from_secs(u64::from(now.minute()) * 60 + u64::from(now.second()))
}

/// One window's critters.
pub struct Critters {
    rng: u64,
    enabled: bool,
    timing: Timing,
    /// The pieces left switched on, by index in [`ART`]. All of them off is
    /// the same silence as `enabled` false, and reached the same way.
    allowed: [bool; ART.len()],
    /// When the next crossing begins. `None` until the first tick, so a
    /// window that is never drawn schedules nothing.
    next: Option<Instant>,
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
    pub fn new(seed: u64, enabled: bool, timing: Timing, allowed: [bool; ART.len()]) -> Self {
        Self {
            rng: seed,
            enabled,
            timing,
            allowed,
            next: None,
            active: None,
            cells: Vec::new(),
            prev: Vec::new(),
        }
    }

    /// The settings, re-applied when the config file changes.
    pub fn configure(&mut self, enabled: bool, timing: Timing, allowed: [bool; ART.len()]) {
        self.enabled = enabled;
        self.timing = timing;
        self.allowed = allowed;
    }

    /// Advance to `now` on a screen this size, and say whether the cells
    /// differ from the last tick.
    pub fn tick(&mut self, now: Instant, cols: usize, rows: usize) -> bool {
        std::mem::swap(&mut self.cells, &mut self.prev);
        self.cells.clear();

        if !self.enabled || cols == 0 || rows == 0 {
            // A screen with no cells cannot host a crossing, and does not
            // consume the one that was due: `next` is left standing.
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
    /// The turn is forfeited and the next settled from here: a window is
    /// unattended the moment it loses the keyboard, so an arrival banked for
    /// a return would greet every return. The cells go and the caller paints
    /// the frame taking them off, which is what INVARIANTS.md asks.
    pub fn withdraw(&mut self, now: Instant) -> bool {
        self.active = None;
        let had = !self.cells.is_empty();
        self.prev.clear();
        self.prev.extend_from_slice(&self.cells);
        self.cells.clear();
        self.schedule(now);
        had
    }

    /// Start a crossing if one is due, or carry the one in hand forward.
    fn advance(&mut self, now: Instant, cols: usize, rows: usize) {
        if let Some((crossing, began)) = self.active.as_mut() {
            let ms = now.saturating_duration_since(*began).as_millis();
            crossing.step = (ms / u128::from(crossing.art.step_ms.max(1))) as u32;
            if crossing.done(cols) {
                self.active = None;
                self.schedule(now);
            }
            return;
        }
        match self.next {
            // The first tick is the epoch: a window schedules from when it is
            // first drawn, not from some earlier zero.
            None => self.schedule(now),
            Some(at) if now >= at => {
                self.active = self.cast(rows).map(|c| (c, now));
                if self.active.is_none() {
                    self.schedule(now);
                }
            }
            Some(_) => {}
        }
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

    fn schedule(&mut self, now: Instant) {
        let delay = match self.timing {
            Timing::Clock(every) => until_mark(past_the_hour(), every),
            Timing::Random(mean) => {
                Duration::from_secs_f64(rng::wait(&mut self.rng, mean.as_secs_f64()))
            }
        };
        self.next = Some(now + delay);
    }

    /// The cells to paint, row-major. Empty when the glass is its own again.
    pub fn cells(&self) -> &[(usize, usize, char)] {
        &self.cells
    }

    /// The name of the piece crossing, for a test and for a log line.
    pub fn crossing(&self) -> Option<&'static str> {
        self.active.map(|(c, _)| c.art.name)
    }

    /// The next column of a crossing in hand, or the instant the next one is
    /// due. The caller folds it into the deadline it already keeps, so a
    /// terminal with nothing else to do sleeps through the wait.
    pub fn wake_at(&self) -> Option<Instant> {
        if !self.enabled {
            return None;
        }
        match self.active {
            Some((crossing, began)) => Some(
                began
                    + Duration::from_millis(
                        u64::from(crossing.step + 1) * u64::from(crossing.art.step_ms.max(1)),
                    ),
            ),
            None => self.next,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mins(m: u64) -> Duration {
        Duration::from_secs(m * 60)
    }

    fn secs(s: u64) -> Duration {
        Duration::from_secs(s)
    }

    #[test]
    fn the_quarter_hours_are_where_the_clock_says() {
        let quarter = mins(15);
        assert_eq!(until_mark(mins(0), quarter), mins(15));
        assert_eq!(until_mark(mins(7) + secs(30), quarter), mins(7) + secs(30));
        assert_eq!(until_mark(mins(14) + secs(59), quarter), secs(1));
        assert_eq!(until_mark(mins(45), quarter), mins(15));
        assert_eq!(until_mark(mins(59), quarter), mins(1));
    }

    /// An interval that does not divide the hour would walk its marks round
    /// the clock an hour at a time. The hour closes the gap with a short step.
    #[test]
    fn the_hour_is_always_a_mark() {
        let seven = mins(7);
        assert_eq!(until_mark(mins(0), seven), mins(7));
        assert_eq!(until_mark(mins(56), seven), mins(4));
        assert_eq!(until_mark(mins(59), seven), mins(1));
    }

    /// A hand-edited file can ask for an interval the settings window does
    /// not offer, and nothing below a second is an interval.
    #[test]
    fn an_interval_of_nothing_is_a_second() {
        assert_eq!(until_mark(mins(0), Duration::ZERO), secs(1));
    }
}
