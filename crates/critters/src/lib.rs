//! The critters: at long random intervals a drawn figure crosses the glass
//! and leaves.
//!
//! # What it is for
//!
//! A laugh from somebody who has seen it before, and nothing else. It is not
//! a screen saver: it comes while you are typing, it does not wait for you to
//! stop, and a keystroke does not chase it away. The old text-mode screen
//! savers had to hide when you touched the keyboard because they *were* the
//! screen, and interrupting one interrupted the work behind it. This is a
//! layer above the terminal being emulated, so there is nothing to interrupt:
//! text scrolls behind a critter, a selection copied across one yields what
//! the session wrote, and the terminal itself never learns one was there.
//!
//! # The promise it keeps
//!
//! A critter is uninvited. It may therefore land on the very line somebody is
//! reading, and the only thing that makes that acceptable is that it does not
//! stay: every piece is off any cell it touches within about a second, which
//! is the rule [`art::ART`] is tuned to and a test holds it to. It follows
//! that nothing here needs to know what is on the screen, or where the cursor
//! is, or whether anybody is typing.
//!
//! # What this crate is
//!
//! Arithmetic on a clock the caller owns. Given the time and the size of the
//! screen it answers with characters and where to put them, or with nothing,
//! and it depends on no renderer, no terminal and no clock. The caller stamps
//! the characters into cells it has made from its own colour scheme, so a
//! critter wears the phosphor without this crate holding an opinion about
//! colour.
//!
//! ```no_run
//! use std::time::{Duration, Instant};
//! let mut critters = critters::Critters::new(1, true, Duration::from_secs(900), [true; critters::ART.len()]);
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
