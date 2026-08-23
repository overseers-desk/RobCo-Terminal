//! The crate's integration tests, one module per concern, compiled as a
//! single test binary: each separate integration-test file links its own
//! ~200MB copy of the dependency stack, and this workspace's suite once
//! weighed 21GB that way.

mod burn_in;
mod mount;

use std::sync::OnceLock;

use crt_burnin::headless::Gpu;

/// The one headless device this binary ever creates, shared by every module
/// in it.
///
/// A [`Gpu`] holds the machine-wide GPU lock for its whole life, and this one
/// is never dropped, so a second device built anywhere in this process would
/// block on a lock this process already holds and never be handed it. One
/// device per test binary is what keeps that from being a deadlock, and it is
/// the modules' shared front door rather than each module's own static.
pub fn gpu() -> &'static Gpu {
    static GPU: OnceLock<Gpu> = OnceLock::new();
    GPU.get_or_init(|| Gpu::new().expect("headless wgpu device"))
}
