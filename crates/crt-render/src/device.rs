//! What the chain needs from the wgpu device, before anyone creates one.
//!
//! Two wgpu features matter to a librashader chain, and both have to be asked
//! for at device creation, which happens outside this crate (`term::gpu::Gpu`
//! offscreen, the app's surface setup in production). Neither is available
//! everywhere, so both are requested only when the adapter offers them, and the
//! crate's behaviour degrades rather than failing.
//!
//! [`required_features`] is the authority on the set. `crates/app` calls it
//! directly. Two other crates create devices the chain runs on and cannot call
//! it, because this crate depends on them and the reverse would be a cycle:
//! `term::gpu` and `crt_burnin::headless` each restate the same filter, each
//! says so at the restatement, and `tests/device_features.rs` asserts all three
//! agree on a real adapter.
//!
//! - `PIPELINE_CACHE`. librashader calls `create_pipeline_cache` whenever its
//!   `enable_cache` option is set and does *not* first check the device
//!   supports it. On a device without the feature that is a wgpu validation
//!   panic from inside a rayon worker, not a `Result` the caller can handle.
//!   The cache stays off in [`chain`](crate::chain) until someone
//!   measures that it pays, but the feature is requested up front so turning it
//!   on is a one-line change rather than a crash.
//!
//! - `FLOAT32_FILTERABLE`. librashader's `float_framebuffer` maps to
//!   `Rgba16Float`, not fp32. True fp32 accumulation is reachable by giving
//!   the pass `#pragma format R32G32B32A32_SFLOAT` with `float_framebuffer`
//!   *off*, but binding an fp32 texture to a filtering sampler without this
//!   feature is a validation panic rather than a quiet fallback. Requesting it
//!   where it exists keeps the fp32 option open for that parity work; the
//!   shipping preset pins the accumulator to fp16 (`MOUNT.md`), and
//!   `crt_burnin::Precision` has no consumer outside `crt-burnin` yet.

/// The features the chain wants, filtered to those this adapter has.
///
/// Pass the result as `required_features` when creating the device. Anything
/// missing from the return value is a capability the chain then has to do
/// without, which is a decision for the code that uses it, not a failure here.
pub fn required_features(adapter: &wgpu::Adapter) -> wgpu::Features {
    let wanted = wgpu::Features::PIPELINE_CACHE | wgpu::Features::FLOAT32_FILTERABLE;
    adapter.features() & wanted
}

/// Whether a device can carry an fp32 accumulator through a filtering sampler.
pub fn supports_fp32_accumulator(device: &wgpu::Device) -> bool {
    device
        .features()
        .contains(wgpu::Features::FLOAT32_FILTERABLE)
}

/// Whether librashader's shader cache is safe to enable on this device.
pub fn supports_pipeline_cache(device: &wgpu::Device) -> bool {
    device.features().contains(wgpu::Features::PIPELINE_CACHE)
}
