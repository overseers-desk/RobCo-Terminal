//! The crate's integration tests, one module per concern, compiled as a
//! single test binary: each separate integration-test file links its own
//! ~200MB copy of the dependency stack, and this workspace's suite once
//! weighed 21GB that way.

mod bank_column;
mod channel_bank;
mod clipboard_keys;
mod frame_stats;
mod ime;
mod keyboard_scroll;
mod pointer;
mod pointer_live_settings;
mod profile_cli;
mod profile_pixels;
mod redraw_pacing;
mod seam_drag;
mod settings_live_reload;
mod shed_notice;
mod size_badge;
mod ssh_flow;
mod structure_subset;
mod tmux_flow;
