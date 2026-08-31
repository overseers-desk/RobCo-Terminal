//! One `wgpu::Instance` serves the whole process: a second one is a second
//! `VkInstance`, and NVIDIA's Wayland driver answered that with a
//! process-killing segfault when one window's surface was destroyed while
//! another window lived. `app::gpu::create_instance` is the one maker on
//! the display-connected path; `crates/gpu`'s offscreen harness builds its
//! own, headless and display-free, where the hazard cannot arise. The test
//! holds the boundary by reading the sources, so a constructor that grows
//! an instance of its own fails here before it reaches a driver.

use std::path::{Path, PathBuf};

fn rust_sources(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("crates tree readable") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            rust_sources(&path, found);
        } else if path.extension().is_some_and(|e| e == "rs") {
            found.push(path);
        }
    }
}

#[test]
fn wgpu_instance_has_one_maker_on_the_display_path() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let allowed = ["app/src/gpu.rs", "gpu/src/offscreen.rs"];
    let mut sources = Vec::new();
    for entry in std::fs::read_dir(&crates).expect("crates dir") {
        let src = entry.expect("dir entry").path().join("src");
        if src.is_dir() {
            rust_sources(&src, &mut sources);
        }
    }
    assert!(
        sources.len() > 10,
        "the walk found almost nothing; the test is blind"
    );
    let strays: Vec<_> = sources
        .iter()
        .filter(|path| {
            let text = std::fs::read_to_string(path).expect("source readable");
            text.contains("Instance::new")
                && !allowed
                    .iter()
                    .any(|ok| path.to_string_lossy().replace('\\', "/").ends_with(ok))
        })
        .collect();
    assert!(
        strays.is_empty(),
        "wgpu instances created outside the shared maker: {strays:?}"
    );
}
