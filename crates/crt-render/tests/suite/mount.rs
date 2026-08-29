//! The mount contract, executed.
//!
//! This crate's own chain will not use the preset shipped in
//! `shaders/burn_in/`: it assembles one preset for the whole CRT chain and puts the accumulator somewhere in the
//! middle of it. What it will use is the shader source and
//! [`crt::burn_in::preset_pass_block`], so those have to produce something that
//! loads and ghosts, and the pass has to behave the same when it is not pass 0.
//!
//! The shipping preset pins the accumulator to fp16, so there is no second
//! precision to generate a preset for.

use std::path::{Path, PathBuf};

use crt::burn_in::decay::decay_step;
use crt::burn_in::{preset_pass_block, BURN_IN_SLANG};
use crt::harness::BurnInChain;
use gpu::harness::{Cell, Locked};

const W: u32 = 64;
const H: u32 = 64;
const DT: f64 = 1.0 / 60.0;
const BURN_IN: f64 = 0.25;
const GHOST: Cell = Cell::new(8, 24, 24, 40);
const STEADY: Cell = Cell::new(40, 24, 56, 40);

const PASSTHROUGH: &str = include_str!("../../shaders/burn_in/passthrough.slang");

fn workdir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("robco-burn-in-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("work dir");
    dir
}

/// Drive a chain for `frames` frames at 60 Hz and return the ghost cell's level
/// each frame. Frame 0 lights both cells, the rest only the steady one.
fn levels(preset: &Path, frames: usize) -> Vec<f32> {
    let gpu = Locked::new().expect("headless wgpu device");
    let mut chain =
        BurnInChain::load(&gpu, preset, BURN_IN, W, H).expect("load generated chain");
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
    std::fs::write(dir.join("burn_in.slang"), BURN_IN_SLANG).expect("write accumulator shader");
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
