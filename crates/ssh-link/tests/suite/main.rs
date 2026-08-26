//! The crate's integration tests, one module per concern, compiled as a
//! single test binary (the workspace rule: each separate integration-test
//! file links its own copy of the dependency stack).

// The far side's agent serves on a Unix socket; until a named-pipe test
// agent exists, this suite compiles on Unix alone.
#[cfg(unix)]
mod flow;
