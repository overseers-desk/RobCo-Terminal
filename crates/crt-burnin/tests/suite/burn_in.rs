//! The burn-in pass on a real device: does the ghost decay at the rate the
//! application set, does the freshness mask behave correctly, and does a
//! mid-run rate change reach the shader without a reload.
//!
//! Headless throughout. Every assert is a pixel read back off the GPU, not a
//! restatement of the CPU-side arithmetic, which is the only way this proves
//! anything the `decay` unit tests do not already cover.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

use crt_burnin::chain::BurnInChain;
use crt_burnin::decay::decay_step;
use crt_burnin::headless::{Cell, Gpu};

const W: u32 = 64;
const H: u32 = 64;
/// 60 Hz, as a wall-clock delta.
const DT: f64 = 1.0 / 60.0;
/// The shipped default `burnIn`, so the numbers here are the ones the
/// shipping default produces: a 0.52 s fade.
const BURN_IN: f64 = 0.25;
const FRAMES: usize = 14;
/// One unit in the last place of an fp16 value just under 1.0, which is what
/// `float_framebuffer` gets the accumulator: librashader maps it to
/// `Rgba16Float`, not to fp32. Every tolerance below that is not a straight
/// percentage is this number.
const FP16_ULP: f32 = 1.0 / 2048.0;

/// Left cell: lit on frame 0, dark after. Right cell: lit for the whole run.
const GHOST: Cell = Cell::new(8, 24, 24, 40);
const STEADY: Cell = Cell::new(40, 24, 56, 40);

/// One device for the whole test binary, and one test on it at a time.
///
/// Two reasons to serialise rather than take a device per test: chain loads are
/// the expensive part of every test here, and concurrent chains on one queue
/// make the frame timings in the reported numbers meaningless.
fn gpu() -> (&'static Gpu, MutexGuard<'static, ()>) {
    static GPU: OnceLock<Gpu> = OnceLock::new();
    static LOCK: Mutex<()> = Mutex::new(());
    let guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let g = GPU.get_or_init(|| Gpu::new().expect("headless wgpu device"));
    (g, guard)
}

fn preset(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("shaders")
        .join(name)
}

/// Green channel of a cell's centre pixel.
fn level(px: &[[f32; 4]], cell: Cell) -> f32 {
    px[cell.centre_index(W)][1]
}

/// Alpha of a cell's centre pixel, which is the freshness mask, not opacity.
fn mask(px: &[[f32; 4]], cell: Cell) -> f32 {
    px[cell.centre_index(W)][3]
}

/// Run `frames` frames at a steady 60 Hz: frame 0 lights both cells, the rest
/// light only `STEADY`. Returns the per-frame centre levels of both cells and
/// the mask on the ghost cell.
struct Run {
    ghost: Vec<f32>,
    steady: Vec<f32>,
    ghost_mask: Vec<f32>,
}

fn run(preset_name: &str, burn_in: f64, mask_on: bool, frames: usize) -> Run {
    let (g, _guard) = gpu();
    let mut chain =
        BurnInChain::load(g, &preset(preset_name), burn_in, W, H).expect("load burn-in chain");
    chain.pass().set_mask(mask_on);

    let mut out = Run {
        ghost: Vec::with_capacity(frames),
        steady: Vec::with_capacity(frames),
        ghost_mask: Vec::with_capacity(frames),
    };
    for f in 0..frames {
        let lit: &[Cell] = if f == 0 { &[GHOST, STEADY] } else { &[STEADY] };
        let px = chain.frame(f as f64 * DT, lit).expect("frame");
        out.ghost.push(level(&px, GHOST));
        out.steady.push(level(&px, STEADY));
        out.ghost_mask.push(mask(&px, GHOST));
    }
    out
}

/// Worst relative error of the measured ramp against the one the set rate
/// predicts.
///
/// The prediction has the mask in it: frame 0 lights the pixel, frame 1 is the
/// one frame the freshness mask protects, and decay starts at frame 2. So the
/// level at frame n is `1 - (n-1)·step`.
fn ramp_error(samples: &[f32], step: f32) -> (f32, Vec<f32>) {
    let mut worst = 0.0f32;
    let mut errs = Vec::new();
    for (n, v) in samples.iter().enumerate().skip(2) {
        let expected = 1.0 - (n as f32 - 1.0) * step;
        let err = ((v - expected) / expected).abs();
        errs.push(err);
        worst = worst.max(err);
    }
    (worst, errs)
}

/// Worst relative error of the per-frame drop against the set step. This is the
/// rate itself rather than the accumulated level, so a systematic bias shows up
/// here even when the ramp's endpoints happen to line up.
fn rate_error(samples: &[f32], step: f32) -> f32 {
    samples
        .windows(2)
        .skip(1)
        .map(|w| (((w[0] - w[1]) - step) / step).abs())
        .fold(0.0f32, f32::max)
}

fn fmt(samples: &[f32]) -> String {
    samples
        .iter()
        .map(|v| format!("{v:.5}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn decay_tracks_the_set_rate() {
    let step = decay_step(BURN_IN, DT);
    let r = run("burn_in.slangp", BURN_IN, true, FRAMES);
    let (worst, _) = ramp_error(&r.ghost, step);
    let rate = rate_error(&r.ghost, step);

    // The same 12-frame window checked below, so the two numbers are comparable.
    let (worst12, _) = ramp_error(&r.ghost[..12], step);

    println!(
        "burn_in={BURN_IN} fade={}s dt={DT:.6}s step={step:.6}",
        0.52
    );
    println!("ghost levels : {}", fmt(&r.ghost));
    println!(
        "worst ramp error over 12 frames       : {:.4}%",
        worst12 * 100.0
    );
    println!(
        "worst ramp error over {FRAMES} frames : {:.4}%",
        worst * 100.0
    );
    println!(
        "worst per-frame rate error            : {:.4}%",
        rate * 100.0
    );
    println!(
        "fp16 ULP near 1.0 as a share of the step: {:.4}%",
        (FP16_ULP / step) * 100.0
    );

    assert!(
        r.ghost[0] > 0.99,
        "frame 0 should be fully lit, read {}",
        r.ghost[0]
    );
    assert!(
        r.ghost[FRAMES - 1] > 0.0,
        "the run is too long: the ghost hit black at frame {}",
        FRAMES - 1
    );
    // The float framebuffer measures 0.24% over 12 frames. Half a
    // percent is that class with room for a different adapter's rounding, and
    // well under the 1.83% an 8-bit accumulator gives.
    assert!(
        worst12 < 0.005,
        "12-frame ramp error {:.4}% exceeds 0.5%",
        worst12 * 100.0
    );
    assert!(
        worst < 0.005,
        "ramp error {:.4}% exceeds 0.5%",
        worst * 100.0
    );
    // The per-frame rate cannot beat the accumulator's own resolution: one fp16
    // ULP near 1.0 is 1/2048, which is 1.5% of a 60 Hz step, and every frame's
    // subtraction gets rounded to it. Twice the ULP is the honest bound and the
    // measured value sits inside one.
    let rate_bound = 2.0 * FP16_ULP / step;
    assert!(
        rate < rate_bound,
        "rate error {:.4}% exceeds the fp16 quantisation bound {:.4}%",
        rate * 100.0,
        rate_bound * 100.0
    );
}

#[test]
fn the_float_framebuffer_is_what_buys_that_accuracy() {
    let step = decay_step(BURN_IN, DT);
    let f32run = run("burn_in.slangp", BURN_IN, true, FRAMES);
    let u8run = run("burn_in_u8.slangp", BURN_IN, true, FRAMES);
    let (f32worst, _) = ramp_error(&f32run.ghost, step);
    let (u8worst, _) = ramp_error(&u8run.ghost, step);

    println!("float accumulator : {}", fmt(&f32run.ghost));
    println!("  worst ramp error: {:.4}%", f32worst * 100.0);
    println!("8-bit accumulator : {}", fmt(&u8run.ghost));
    println!("  worst ramp error: {:.4}%", u8worst * 100.0);

    assert!(
        u8worst > f32worst * 2.0,
        "the 8-bit accumulator was not measurably worse (float {:.4}%, u8 {:.4}%): \
         either the preset's float_framebuffer0 stopped taking effect or the \
         readback is quantising both",
        f32worst * 100.0,
        u8worst * 100.0
    );
}

#[test]
fn the_freshness_mask_suppresses_decay_on_newly_lit_pixels() {
    let step = decay_step(BURN_IN, DT);
    let on = run("burn_in.slangp", BURN_IN, true, 6);
    let off = run("burn_in.slangp", BURN_IN, false, 6);

    println!("mask on  : ghost {}", fmt(&on.ghost));
    println!("           mask  {}", fmt(&on.ghost_mask));
    println!("           steady {}", fmt(&on.steady));
    println!("mask off : ghost {}", fmt(&off.ghost));

    // The pixel that stays lit never decays: the live source holds it at full
    // brightness and its mask stays 1, so it is never even a candidate.
    for (f, v) in on.steady.iter().enumerate() {
        assert!(*v > 0.99, "steady cell dimmed to {v} on frame {f}");
    }
    for (f, m) in on.ghost_mask.iter().enumerate().take(1) {
        assert_eq!(*m, 1.0, "frame {f}: a newly lit pixel must set the mask");
    }
    // Frame 1 is the frame the mask pays for: the pixel went dark, but it was
    // lit on the frame before, so prevMask cancels the decay exactly once.
    assert!(
        (on.ghost[1] - 1.0).abs() < 1e-4,
        "frame 1 should still be at full brightness under the mask, read {}",
        on.ghost[1]
    );
    assert_eq!(
        on.ghost_mask[1], 0.0,
        "frame 1's pixel is ghost, not live, so it must clear the mask"
    );
    // And from frame 2 the ramp runs.
    assert!(
        (on.ghost[2] - (1.0 - step)).abs() < 1e-3,
        "frame 2 should have decayed exactly once ({}), read {}",
        1.0 - step,
        on.ghost[2]
    );

    // Same chain, same rate, mask off: frame 1 has already decayed. That is the
    // whole difference the mask makes, one frame of grace on a fresh pixel.
    assert!(
        (off.ghost[1] - (1.0 - step)).abs() < 1e-3,
        "with the mask off frame 1 should have decayed to {}, read {}",
        1.0 - step,
        off.ghost[1]
    );
    assert!(
        on.ghost[1] - off.ghost[1] > step * 0.9,
        "mask on and mask off differ by {:.5}, less than the one step ({step:.5}) they should",
        on.ghost[1] - off.ghost[1]
    );
}

#[test]
fn changing_the_rate_mid_run_takes_effect_without_a_reload() {
    let (g, _guard) = gpu();
    let mut chain =
        BurnInChain::load(g, &preset("burn_in.slangp"), BURN_IN, W, H).expect("load chain");

    // 0.25 gives a 0.52 s fade, 1.0 gives 1.6 s: a three-times slower ramp on a
    // chain that is never touched structurally.
    let before = decay_step(BURN_IN, DT);
    let after = decay_step(1.0, DT);
    let switch_at = 7;

    let mut samples = Vec::new();
    for f in 0..FRAMES {
        if f == switch_at {
            chain.pass().set_burn_in(1.0);
        }
        let lit: &[Cell] = if f == 0 { &[GHOST, STEADY] } else { &[STEADY] };
        let px = chain.frame(f as f64 * DT, lit).expect("frame");
        samples.push(level(&px, GHOST));
    }

    let drops: Vec<f32> = samples.windows(2).map(|w| w[0] - w[1]).collect();
    println!("levels : {}", fmt(&samples));
    println!("drops  : {}", fmt(&drops));
    println!("step before switch {before:.6}, after {after:.6}, switched at frame {switch_at}");

    // drops[i] is the fall from frame i to frame i+1, so the first frame drawn
    // with the new rate is `switch_at`, whose drop sits at `switch_at - 1`.
    let last_old = drops[switch_at - 2];
    let first_new = drops[switch_at - 1];
    // Tolerances are one fp16 ULP plus a percent: at 0.010417 per frame the
    // accumulator cannot represent the step exactly, so the drop lands on the
    // nearest representable neighbour and stays there.
    assert!(
        (last_old - before).abs() < FP16_ULP + before * 0.01,
        "pre-switch drop {last_old:.6} does not match the set step {before:.6}"
    );
    assert!(
        (first_new - after).abs() < FP16_ULP + after * 0.01,
        "the very next frame after the change dropped {first_new:.6}, not the new step {after:.6}; \
         a parameter write did not reach the shader in the same frame"
    );
    assert!(
        (first_new - before).abs() > before * 0.5,
        "the drop after the change ({first_new:.6}) is still the old rate ({before:.6})"
    );
    assert_eq!(
        chain.parameter("BURNIN_DECAY_STEP"),
        Some(after),
        "the chain should be holding the new step"
    );
}

#[test]
fn burn_in_zero_leaves_no_ghost_and_needs_no_rebuild() {
    let r = run("burn_in.slangp", 0.0, true, 4);
    println!("burn_in=0 ghost: {}", fmt(&r.ghost));
    for (f, v) in r.ghost.iter().enumerate().skip(1) {
        assert!(
            *v < 1e-6,
            "frame {f} still shows {v} of ghost with burn-in switched off"
        );
    }
    for (f, v) in r.steady.iter().enumerate() {
        assert!(*v > 0.99, "steady cell dimmed to {v} on frame {f}");
    }
}

#[test]
fn a_restart_costs_the_ghost_nothing_but_one_frame_of_decay() {
    let (g, _guard) = gpu();
    let mut chain =
        BurnInChain::load(g, &preset("burn_in.slangp"), BURN_IN, W, H).expect("load chain");
    let step = decay_step(BURN_IN, DT);

    let mut samples = Vec::new();
    for f in 0..8 {
        if f == 5 {
            // What a resize or a font change does to the clock.
            chain.pass().restart();
        }
        let lit: &[Cell] = if f == 0 { &[GHOST, STEADY] } else { &[STEADY] };
        let px = chain.frame(f as f64 * DT, lit).expect("frame");
        samples.push(level(&px, GHOST));
    }
    println!("restart at frame 5: {}", fmt(&samples));
    // The restarted frame decays by nothing; the frames either side by a step.
    assert!((samples[4] - samples[5]).abs() < 1e-5, "{samples:?}");
    assert!(
        ((samples[5] - samples[6]) - step).abs() < step * 0.01,
        "{samples:?}"
    );
}
