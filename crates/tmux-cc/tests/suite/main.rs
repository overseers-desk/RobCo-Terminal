//! The crate's integration tests, one module per concern, compiled as a
//! single test binary: each separate integration-test file links its own
//! ~200MB copy of the dependency stack, and this workspace's suite once
//! weighed 21GB that way.

// The live test drives a real tmux on a real pty; tmux runs on no
// Windows, so it compiles on Unix alone. `support` splits the same way
// inside: its server and gateway are Unix, its transcript decoders are
// not, and the replays run everywhere.
mod support;
#[cfg(unix)]
mod live_tmux;
mod transcripts;
