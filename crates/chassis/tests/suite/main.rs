//! The crate's integration tests, one module per concern, compiled as a
//! single test binary: each separate integration-test file links its own
//! ~200MB copy of the dependency stack, and this workspace's suite once
//! weighed 21GB that way.

mod bank_frame_geometry;
mod chassis_metal;
mod gpu_annunciator;
mod led_display;
mod led_matrix;
mod metrics_homes;
mod metrics_tables;
mod plate_metal;
mod region_layout;
mod shader_recipes;
mod tape_display;
mod tape_label;
