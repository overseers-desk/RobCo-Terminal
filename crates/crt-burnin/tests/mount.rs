//! The mount contract, executed.
//!
//! `crt-render` will not use the preset shipped in `shaders/`: it assembles one
//! preset for the whole CRT chain and puts the accumulator somewhere in the
//! middle of it. What it will use is [`crt_burnin::write_shader`] and
//! [`crt_burnin::preset_pass_block`], so those have to produce something that
//! loads and ghosts, and the pass has to behave the same when it is not pass 0.
//!
//! The fp32 measurement lives here too, because it is the same machinery: a
//! generated preset is the only way to ask for a format the shipped one does not
//! request.

use std::path::{Path, PathBuf};

use crt_burnin::chain::BurnInChain;
use crt_burnin::decay::decay_step;
use crt_burnin::headless::{Cell, Gpu};
use crt_burnin::{
    preset_pass_block, preset_pass_block_with, write_shader, write_shader_with, Precision,
};

const W: u32 = 64;
const H: u32 = 64;
const DT: f64 = 1.0 / 60.0;
const BURN_IN: f64 = 0.25;
const GHOST: Cell = Cell::new(8, 24, 24, 40);
const STEADY: Cell = Cell::new(40, 24, 56, 40);

const PASSTHROUGH: &str = include_str!("../shaders/passthrough.slang");

fn gpu() -> &'static Gpu {
    use std::sync::OnceLock;
    static GPU: OnceLock<Gpu> = OnceLock::new();
    GPU.get_or_init(|| Gpu::new().expect("headless wgpu device"))
}

fn workdir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("crt-burnin-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("work dir");
    dir
}

/// Drive a chain for `frames` frames at 60 Hz and return the ghost cell's level
/// each frame. Frame 0 lights both cells, the rest only the steady one.
fn levels(preset: &Path, frames: usize) -> Vec<f32> {
    let mut chain = BurnInChain::load(gpu(), preset, BURN_IN, W, H).expect("load generated chain");
    (0..frames)
        .map(|f| {
            let lit: &[Cell] = if f == 0 { &[GHOST, STEADY] } else { &[STEADY] };
            let px = chain.frame(f as f64 * DT, lit).expect("frame");
            px[GHOST.centre_index(W)][1]
        })
        .collect()
}

#[test]
fn a_generated_preset_mounts_the_pass_after_another_one() {
    let dir = workdir("mount");
    write_shader(&dir).expect("write accumulator shader");
    std::fs::write(dir.join("passthrough.slang"), PASSTHROUGH).expect("write passthrough");

    // Three passes, with the accumulator in the middle: a copy stands in for the
    // terminal grid pass ahead of it and for the CRT passes behind it. This is
    // the shape `crt-render` will have, and it is the shape that would catch the
    // accumulator reading `Original` instead of `Source`.
    let preset = dir.join("mounted.slangp");
    let mut text = String::from("shaders = 3\n\nshader0 = passthrough.slang\nscale_type0 = source\nscale0 = 1.0\nfilter_linear0 = false\n\n");
    text.push_str(&preset_pass_block(1, "burn_in.slang"));
    text.push_str("\nshader2 = passthrough.slang\nscale_type2 = source\nscale2 = 1.0\nfilter_linear2 = false\n");
    std::fs::write(&preset, text).expect("write preset");

    let step = decay_step(BURN_IN, DT);
    let l = levels(&preset, 6);
    println!("mounted at pass 1 of 3: {l:?}");

    assert!(l[0] > 0.99, "frame 0 not lit: {l:?}");
    assert!(
        (l[1] - 1.0).abs() < 1e-4,
        "the freshness mask did not survive the mount: {l:?}"
    );
    assert!(
        (l[2] - (1.0 - step)).abs() < 1e-3,
        "decay after the mount is {:.5}, expected {:.5}",
        l[2],
        1.0 - step
    );
}

#[test]
fn fp32_accumulator_is_available_and_better_than_fp16() {
    if !gpu().float32_filterable {
        // Not a silent skip: without the feature this is not "slightly worse",
        // it is a wgpu validation error inside librashader's bind group. The
        // shipping preset asks for fp16 precisely so it never depends on this.
        println!(
            "adapter {} lacks FLOAT32_FILTERABLE: an fp32 accumulator is unavailable here",
            gpu().adapter_name
        );
        return;
    }
    let dir = workdir("fp32");
    std::fs::write(dir.join("passthrough.slang"), PASSTHROUGH).expect("write passthrough");
    // Both halves of the fp32 mount come from the crate, which is the point of
    // the test: `write_shader_with` injects the `#pragma format` and
    // `preset_pass_block_with` leaves `float_framebuffer` out, because that key
    // is an override that would beat the pragma.
    write_shader_with(&dir, Precision::Fp32).expect("write fp32 shader");

    let preset = dir.join("fp32.slangp");
    let mut text = String::from("shaders = 2\n\n");
    text.push_str(&preset_pass_block_with(0, "burn_in.slang", Precision::Fp32));
    text.push_str("\nshader1 = passthrough.slang\nscale_type1 = source\nscale1 = 1.0\nfilter_linear1 = false\n");
    std::fs::write(&preset, text).expect("write preset");

    let step = decay_step(BURN_IN, DT);
    let frames = 14;
    let fp32 = levels(&preset, frames);
    let fp16 = levels(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("shaders")
            .join("burn_in.slangp"),
        frames,
    );

    let err = |s: &[f32]| {
        s.iter()
            .enumerate()
            .skip(2)
            .map(|(n, v)| {
                let expected = 1.0 - (n as f32 - 1.0) * step;
                ((v - expected) / expected).abs()
            })
            .fold(0.0f32, f32::max)
    };

    println!("fp32 (#pragma format) : {fp32:?}");
    println!("  worst ramp error    : {:.4}%", err(&fp32) * 100.0);
    println!("fp16 (float_framebuffer): {fp16:?}");
    println!("  worst ramp error    : {:.4}%", err(&fp16) * 100.0);

    assert!(fp32[0] > 0.99, "fp32 chain produced no lit frame: {fp32:?}");
    assert!(
        err(&fp32) <= err(&fp16),
        "the fp32 accumulator was not better than fp16 ({:.4}% vs {:.4}%)",
        err(&fp32) * 100.0,
        err(&fp16) * 100.0
    );
}
