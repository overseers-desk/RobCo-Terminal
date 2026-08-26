//! The crate's integration tests, one module per concern, compiled as a
//! single test binary: each separate integration-test file links its own
//! ~200MB copy of the dependency stack, and this workspace's suite once
//! weighed 21GB that way.

// The support plumbing and the live test drive a real tmux on a real pty;
// tmux runs on no Windows, so both compile on Unix alone. The transcript
// replays are bytes against the codec and run everywhere.
#[cfg(unix)]
mod support;
#[cfg(unix)]
mod live_tmux;
mod transcripts;
