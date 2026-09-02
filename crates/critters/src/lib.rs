//! The critters: every so often a drawn figure crosses the glass and leaves.
//!
//! A layer above the terminal being emulated, so nothing is interrupted: text
//! scrolls behind a critter, a keystroke does not chase it away, a selection
//! copied across one yields what the session wrote, and the terminal never
//! learns one was there. It is not a screen saver and does not wait for you
//! to stop typing.
//!
//! A critter is uninvited, so it may land on the line somebody is reading.
//! What makes that acceptable is that it does not stay; [`art`] carries the
//! rule and the number that keeps it, and nothing here needs to know what is
//! on the screen or whether anybody is typing.
//!
//! Given the time and the size of the screen this answers with characters and
//! where to put them, or with nothing. The caller stamps them into cells of
//! its own colour scheme, so a critter wears the phosphor without this crate
//! holding an opinion about colour.
//!
//! ```no_run
//! use std::time::{Duration, Instant};
//! let mut critters = critters::Critters::new(1, true, Duration::from_secs(900), true, [true; critters::ART.len()]);
//! if critters.tick(Instant::now(), 80, 24) {
//!     for &(row, col, ch) in critters.cells() {
//!         let _ = (row, col, ch);
//!     }
//! }
//! ```

pub mod art;
pub mod pieces;
mod rng;
mod schedule;

pub use art::{Art, Crossing, ART, TRANSPARENT};
pub use schedule::Critters;
