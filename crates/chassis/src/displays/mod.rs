//! Channel display kits, one directory a kit. Each kit composes the
//! already-proven glyph raster ([`term::fonts::led`]) with its own shader
//! (`shaders/{led_matrix,tape_label}/`, moved into this crate with the rest
//! of the chrome), and carries its own two contracts:
//!
//! * Metrics: the width/height quantisation a strip or well grows and
//!   shrinks by. Free functions in each kit's `metrics` module.
//! * Display appearance: the state-to-appearance mapping -- what
//!   `powered`/`bright`/text turn into as shader uniforms and raster calls.
//!   Free functions and constants; the layout, chrome, the bank's row
//!   furniture and the tape well are not part of this module and stay out.

pub mod led;
pub mod raster;
pub mod tape;
