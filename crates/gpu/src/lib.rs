//! The wgpu device concerns every crate in this tree shares.
//!
//! One crate owns three things that were otherwise restated wherever a device
//! is made:
//!
//! - [`required_features`], the feature set a device the CRT chain runs on has
//!   to be created with. Every caller that creates such a device calls this:
//!   the application's surface setup, the offscreen [`Gpu`] below, and the
//!   test harness. Both features have to be asked for at creation or they are
//!   gone for the life of the device.
//! - [`Target`], an offscreen colour attachment at any format, and the padded
//!   readback that turns one back into pixels. This is not test scaffolding:
//!   the grid is drawn into a `Target` before the CRT chain runs, which is the
//!   seam the pass graph hangs off.
//! - [`harness`], behind the feature of the same name: the machine-wide device
//!   lock and the locked device the GPU-backed tests run on.
//!
//! This crate is a leaf. It depends on wgpu and pollster and on nothing else in
//! the workspace, so any crate that makes a device can reach it whatever else
//! it depends on.
//!
//! # Why these two features
//!
//! - `PIPELINE_CACHE`. librashader calls `create_pipeline_cache` whenever its
//!   `enable_cache` option is set and does *not* first check the device
//!   supports it. On a device without the feature that is a wgpu validation
//!   panic from inside a rayon worker, not a `Result` the caller can handle.
//!   The cache stays off in `crt::chain` until someone measures that it pays,
//!   but the feature is requested up front so turning it on is a one-line
//!   change rather than a crash.
//!
//! - `FLOAT32_FILTERABLE`. librashader's `float_framebuffer` maps to
//!   `Rgba16Float`, not fp32. True fp32 accumulation is reachable by giving a
//!   pass `#pragma format R32G32B32A32_SFLOAT` with `float_framebuffer` *off*,
//!   but binding an fp32 texture to a filtering sampler without this feature is
//!   a validation panic rather than a quiet fallback. Requesting it where it
//!   exists keeps that option open; the shipping preset pins the burn-in
//!   accumulator to fp16.
//!
//! Neither is available everywhere, so both are requested only when the adapter
//! offers them and behaviour degrades rather than failing.

mod offscreen;

#[cfg(feature = "harness")]
pub mod harness;

pub use offscreen::{color_texture, read_back, Diff, Gpu, Image, Target, TARGET_FORMAT};

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
