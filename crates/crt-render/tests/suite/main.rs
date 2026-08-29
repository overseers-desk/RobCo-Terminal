//! The crate's integration tests, one module per concern, compiled as a
//! single test binary: each separate integration-test file links its own
//! ~200MB copy of the dependency stack, and this workspace's suite once
//! weighed 21GB that way.

mod support;
mod bank_frame_render;
mod bloom;
mod burn_in;
mod burn_in_chain;
mod contracts;
mod frame_metal;
mod glyph_survival;
mod mount;
mod pass_graph;
mod terminal_frame;
mod user_lut;
