//! The crate's integration tests, one module per concern, compiled as a
//! single test binary: each separate integration-test file links its own
//! ~200MB copy of the dependency stack, and this workspace's suite once
//! weighed 21GB that way.

mod antialias;
mod font_parity;
mod grid_tests;
mod hotspot_tests;
mod pixel_properties;
mod preedit;
mod rio_grid_tests;
mod scrollback;
mod search_tests;
mod selection_tests;
mod system_fonts;
mod transcript;
