//! Window geometry: the numbers the CLI/window contract measures.
//!
//! Four numbers define the contract:
//!
//! ```text
//! width: 1024
//! height: 768
//! crt_minimum_width: 320
//! minimum_width: bank_minimum + crt_minimum_width
//! minimum_height: 240
//! ```
//!
//! Only the first two live here. The last two are the chassis's, and were
//! once a second copy of it: the seam clamp and the window's
//! minimum-size hint are one rule seen from two sides, and the seam belongs
//! to `crates/chassis`. `chassis::layout::CRT_MINIMUM_WIDTH` and
//! `MINIMUM_HEIGHT` are the surviving pair, in logical pixels;
//! `chassis::layout::min_inner_size_physical` scales them for the
//! `Size::Physical` hint winit and `WM_NORMAL_HINTS` measure in, and
//! `app::shell` calls it at both hint sites.
//!
//! Contract item 4 measures the hint through `xprop -id <wid>
//! WM_NORMAL_HINTS`: the program-specified minimum width must equal the
//! channel bank's *least* pixel width -- `bank_minimum`, the bank at the
//! display's narrowest strip, not the width it happens to be drawn at --
//! plus the screen well's floor. That is why the bank minimum is an
//! argument rather than something read from anywhere: it changes at runtime
//! as the font or the screen well's margin is edited, so the shell
//! re-applies the hint whenever it moves.

/// The window's initial size.
pub const DEFAULT_SIZE: (u32, u32) = (1024, 768);
