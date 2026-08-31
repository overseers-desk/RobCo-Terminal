//! When a critter comes, which one, and where it walks.
//!
//! A state machine over a clock the caller owns, in the shape
//! `app::overlay` and `crt::Pacing` already use here: nothing in this file
//! reads `Instant::now()`, so a test drives a thousand hours through it in a
//! millisecond and asserts every cell of every crossing.
//!
//! # The wait
//!
//! Drawn from [`crate::rng::wait`], which says why it is drawn rather than
//! counted. The clock restarts when a piece walks off rather than when it
//! walked on, so two never overlap and the gap between them is still
//! shapeless.
//!
//! # Where it walks
//!
//! Any row, the cursor's included. There is no quiet corner of a terminal to
//! prefer: a shell writes at the bottom, a full-screen program writes
//! everywhere, and a rule keeping critters out of the busy half would put
//! them all in one place. What makes a free choice of row safe is the speed
//! rule in [`crate::art`], not the choice itself.
//!
//! A piece that does not fit the screen is clipped rather than passed over.
//! A locomotive on a six-row window shows the band of itself that fits,
//! which is what a train seen through a window is.

use std::time::{Duration, Instant};

use crate::art::{Art, Crossing, ART};
use crate::rng;

/// One window's critters.
pub struct Critters {
    rng: u64,
    enabled: bool,
    mean: Duration,
    /// Which pieces the user has left switched on, by their index in
    /// [`ART`]. All of them off is the same silence as `enabled` false, and
    /// is reached the same way: nothing is cast.
    allowed: [bool; ART.len()],
    /// When the next crossing begins. `None` until the first tick, which is
    /// what makes a window that is never drawn never schedule anything.
    next: Option<Instant>,
    /// The crossing in progress, and the instant it began.
    active: Option<(Crossing, Instant)>,
    /// The cells as of the last tick, row-major.
    cells: Vec<(usize, usize, char)>,
    /// The cells as of the tick before, kept so [`Critters::tick`] can
    /// compare rather than guess. The two are swapped each tick, so this
    /// costs one allocation over the life of a window rather than one a
    /// frame.
    prev: Vec<(usize, usize, char)>,
}

impl Critters {
    /// A window's own, seeded. The seed is the whole of the randomness here:
    /// two given the same seed and the same ticks paint the same cells for
    /// ever, which is what the tests stand on. The caller seeds off the wall
    /// clock; nothing in this crate reads one.
    pub fn new(seed: u64, enabled: bool, mean: Duration, allowed: [bool; ART.len()]) -> Self {
        Self {
            rng: seed,
            enabled,
            mean,
            allowed,
            next: None,
            active: None,
            cells: Vec::new(),
            prev: Vec::new(),
        }
    }

    /// The settings, re-applied whenever the config file changes. Switching
    /// it off takes a crossing down at the next tick rather than freezing it
    /// where it stands.
    pub fn configure(&mut self, enabled: bool, mean: Duration, allowed: [bool; ART.len()]) {
        self.enabled = enabled;
        self.mean = mean;
        self.allowed = allowed;
    }

    /// Advance to `now` on a screen this size, and say whether the cells
    /// differ from the last tick.
    pub fn tick(&mut self, now: Instant, cols: usize, rows: usize) -> bool {
        std::mem::swap(&mut self.cells, &mut self.prev);
        self.cells.clear();

        if !self.enabled || cols == 0 || rows == 0 {
            // A screen with no cells cannot host a crossing, and must not
            // consume the one that was due: `next` is left standing.
            self.active = None;
        } else {
            self.advance(now, cols, rows);
        }

        if let Some((crossing, _)) = self.active {
            crossing.paint(cols, rows, &mut self.cells);
        }
        // The cells themselves, not a count of them: a piece that steps one
        // column while wholly on the glass paints the same number of cells in
        // the same rows, and a count would call that no change.
        self.cells != self.prev
    }

    /// Take whatever is crossing off the glass, and say whether the glass
    /// changed by it.
    ///
    /// What is due stays due: `next` is left where it is, so a piece the
    /// clock has already called for crosses when there is somebody to see
    /// it. This is the one way a crossing ends early, and it ends the way
    /// the invariant requires -- the cells go, and the caller paints the
    /// frame that takes them off.
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
                self.schedule(now);
            }
            return;
        }
        match self.next {
            // The first tick is the epoch: a window schedules from the
            // moment it is first drawn, not from some earlier zero.
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

    /// Choose a piece, a facing and a row.
    ///
    /// Uniform over the pieces still switched on, so retiring the locomotive
    /// makes the others correspondingly more likely rather than leaving a
    /// hole in the schedule where it used to be.
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
            // Taller than the glass: centre it on a row and let both ends go
            // off the screen.
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
        let wait = rng::wait(&mut self.rng, self.mean.as_secs_f64());
        self.next = Some(now + Duration::from_secs_f64(wait));
    }

    /// The cells to paint, row-major. Empty when the glass is its own again.
    pub fn cells(&self) -> &[(usize, usize, char)] {
        &self.cells
    }

    /// The name of the piece crossing, for a test and for a log line.
    pub fn crossing(&self) -> Option<&'static str> {
        self.active.map(|(c, _)| c.art.name)
    }

    /// When this next has something to say: the next column of a crossing in
    /// hand, or the instant the next one is due. The caller folds it into the
    /// deadline it is already keeping, so a terminal with nothing else to do
    /// sleeps through the quarter hour instead of polling it away.
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
