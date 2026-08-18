//! The features the chain asks for at device creation, and the three places
//! that ask.
//!
//! `crt::device::required_features` is the authority. Two other crates create
//! devices the chain runs on and neither can call it, because `crt-render`
//! depends on both: `term::gpu`, which makes the offscreen device the grid is
//! drawn on, and `crt_burnin::headless`, the measurement rig. Each restates the
//! set, and this test is what stops the restatements drifting: it is the one
//! place in the workspace that can see all three.
//!
//! It is a real adapter rather than a hand-built feature set because the
//! functions are filters, not constants. Asserting they return the same thing
//! on the machine's own adapter tests the filtering too, and on an adapter with
//! neither feature it still catches one of them asking for something the other
//! two do not.

/// The adapter this machine actually has, or `None` where there is no GPU at
/// all. Nothing here needs a device, only the feature list.
fn adapter() -> Option<wgpu::Adapter> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
        ..Default::default()
    }))
    .ok()
}

#[test]
fn every_device_the_chain_runs_on_asks_for_the_same_features() {
    let Some(adapter) = adapter() else {
        panic!("no wgpu adapter: this suite needs one (a software Vulkan ICD will do)");
    };

    let wanted = crt::device::required_features(&adapter);
    assert_eq!(
        term::gpu::required_features(&adapter),
        wanted,
        "term::gpu asks for a different feature set than crt::device, so the \
         offscreen device the grid is drawn on is not the device the chain was \
         measured on"
    );
    assert_eq!(
        crt_burnin::headless::required_features(&adapter),
        wanted,
        "the headless harness asks for a different feature set than \
         crt::device, so what the tests measure is not what ships"
    );

    println!(
        "adapter {:?} ({:?}): the chain asks for {wanted:?}",
        adapter.get_info().name,
        adapter.get_info().backend,
    );
}

#[test]
fn the_wanted_set_is_a_filter_and_never_more_than_the_adapter_has() {
    let Some(adapter) = adapter() else {
        panic!("no wgpu adapter");
    };
    let wanted = crt::device::required_features(&adapter);

    // Asking for a feature the adapter lacks is not a degraded device, it is a
    // `request_device` that fails, and then there is no terminal at all. Both
    // capabilities are optional to the chain: without `FLOAT32_FILTERABLE` the
    // accumulator stays at fp16 (0.26% over a fade), and without
    // `PIPELINE_CACHE` librashader's shader cache stays off, which it is
    // anyway.
    assert!(
        adapter.features().contains(wanted),
        "the chain asked for {:?}, which this adapter does not have",
        wanted - adapter.features()
    );
    // And nothing beyond the two the crate documents.
    assert!(
        (wgpu::Features::PIPELINE_CACHE | wgpu::Features::FLOAT32_FILTERABLE).contains(wanted),
        "the chain asked for {wanted:?}, which is more than the two features \
         crt::device explains"
    );
}
