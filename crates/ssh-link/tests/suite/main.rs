//! The crate's integration tests, one module per concern, compiled as a
//! single test binary (the workspace rule: each separate integration-test
//! file links its own copy of the dependency stack).

mod flow;
