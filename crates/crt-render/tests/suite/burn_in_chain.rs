//! The burn-in accumulator, measured where it actually runs.
//!
//! `crt-burnin`'s own suite proves the physics on a chain of its own: a
//! standalone preset, or three passes with a copy either side. This proves the
//! same physics survive the mount that ships: five passes, the accumulator at
//! index 0 rendering at `general.burn_in_quality`, its ghost read back out by
//! the dynamic pass at the far end and composited before anything is visible.
//! Everything between the two is what could go wrong and would not show up as
//! an error: an alias that stops creating a feedback framebuffer, a decay
//! parameter that never reaches a shader whose push block the reflector laid
//! out differently, a composite that drops the freshness mask.
//!
//! So the readback here is the chain's *output*, not the accumulator's
//! framebuffer, and every number below has the whole graph in it: the
//! accumulator's ramp, the `0.65` trim the composite applies to the ghost, and
//! the `(1.0 - alpha)` mask on the consuming side.
//!
//! The output is read as `Rgba32Float` rather than through
//! `gpu::Target`. An 8-bit target quantises at 1/255, which is a third of
//! a frame's decay here, so the ramp it produced could not be told from a
//! different one.

use crate::support;

use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use config::Config;
use crt::{Chain, DegaussState, Geometry, Pacing, Params};
use crt_burnin::decay::decay_step;

const W: u32 = 128;
const H: u32 = 128;
/// 60 Hz, which is what `decay_step` is being asked to predict.
const DT: Duration = Duration::from_micros(16_667);
/// The slider's top: a 1.6 s fade. The picture is a 0.30 grey bar, not a white
/// one, so the ghost starts a third of the way up and a faster fade would reach
/// black before the twelve frames the ramp is measured over.
const BURN_IN: f64 = 1.0;
const FRAMES: usize = 14;
/// The trim `terminal_dynamic` applies to the ghost.
const GHOST_TRIM: f32 = 0.65;
/// The `rgb2grey` weights, which do not sum to one. Every pixel leaving
/// the dynamic pass has been through `convertWithChroma`, and on a white font
/// over a black background that is `mix(black, white, rgb2grey(c))`, so a grey
/// ghost comes out 3% darker than it went in.
const GREY: f32 = 0.21 + 0.72 + 0.04;
/// What the chain does to the accumulator's own value on the way to the
/// readback. Everything below is measured through this.
const SCALE: f32 = GHOST_TRIM * GREY;
/// One fp16 ULP where the ghost starts, in [0.25, 0.5). It halves as the ghost
/// falls past 0.25, so this is the conservative end.
const FP16_ULP: f32 = 1.0 / 8192.0;

/// One test on the device at a time.
///
/// Each test here stands up its own wgpu device and its own five-pass chain,
/// and three of those at once on a software Vulkan ICD is a segfault inside the
/// driver rather than a test failure. `crt-burnin`'s own suite serialises for
/// the same reason. It costs nothing: the chain loads dominate either way.
fn serial() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// A settings snapshot with nothing that moves on its own except the ghost, and
/// nothing drawn over it: the frame pass lays a glass sheen across the whole
/// screen whenever `ambientLight`, the chassis-or-screen frame size, or the
/// curvature is non-zero, and that sheen is an additive term the ramp measured
/// here would carry into every reading.
fn config() -> Config {
    let mut cfg = Config::default();
    cfg.chassis.frame_size = 0.0;
    let s = &mut cfg.screen;
    s.ambient_light = 0.0;
    s.flickering = 0.0;
    s.horizontal_sync = 0.0;
    s.static_noise = 0.0;
    s.jitter = 0.0;
    s.glowing_line = 0.0;
    s.bloom = 0.0;
    s.rgb_shift = 0.0;
    s.screen_curvature = 0.0;
    s.frame_size = 0.0;
    s.frame_shininess = 0.0;
    s.chroma_color = 0.0;
    s.brightness = 0.0;
    s.font_color = "#ffffff".into();
    s.background_color = "#000000".into();
    // ...and the profile's own colour arithmetic taken out of the way.
    // The two colours mix into each other by `0.7 + contrast * 0.3`, so at
    // the shipped contrast of 0.8 the background this reads the ghost
    // against is six percent of white rather than black, and
    // `saturationColor` mixes the font colour towards white that it already
    // is. Contrast 1.0 makes the mix the identity.
    //
    // The white still arrives as 255/256, not 1.0: that is `strToColor`, and
    // every level below carries the same 0.4%.
    s.contrast = 1.0;
    s.saturation_color = 0.0;
    s.burn_in = BURN_IN;
    cfg
}

fn geometry() -> Geometry {
    Geometry {
        output_width: W as f32,
        output_height: H as f32,
        // A density of 2 puts `RasterizationIntensity` at the bottom of its
        // smoothstep, so the rasterization mask cannot chew on the ghost.
        virtual_width: 64.0,
        virtual_height: 64.0,
        total_font_scaling: 1.0,
        device_pixel_ratio: 1.0,
    }
}

/// One frame of the run: draw the picture, push the uniforms, run the chain
/// into a float target, and return the centre pixel's green channel.
///
/// The centre of the image is the middle of the bar, which is lit on frame 0
/// and dark afterwards: the ghost cell.
struct Run {
    levels: Vec<f32>,
    decays: Vec<f32>,
    loads: u64,
}

fn run(mask_on: bool, frames: usize) -> Run {
    let _guard = serial();
    let h = support::Harness::new(
        if mask_on {
            "burnin-mask"
        } else {
            "burnin-nomask"
        },
        W,
        H,
    )
    .expect("gpu");
    let cfg = config();
    let mut chain = Chain::from_config(&h.gpu.device, &h.gpu.queue, &h.dir, &cfg).expect("chain");
    chain.burn_in().set_mask(mask_on);

    let output = h.gpu.make_output(W, H);
    let view = output.create_view(&wgpu::TextureViewDescriptor::default());
    let mut pacing = Pacing::new(Instant::now());

    let mut levels = Vec::with_capacity(frames);
    let mut decays = Vec::with_capacity(frames);
    for f in 0..frames {
        if f == 0 {
            h.draw_picture();
        } else {
            h.draw_dark();
        }

        let time = pacing.tick_by(DT);
        chain.set_params(&Params::build(&cfg, &geometry(), time, DegaussState::IDLE));

        let mut encoder = h
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("burn-in chain frame"),
            });
        chain
            .frame(
                &h.input.texture,
                &view,
                (W, H),
                crt_burnin::headless::OUTPUT_FORMAT,
                &mut encoder,
                time,
            )
            .expect("chain frame");
        let index = h.gpu.queue.submit([encoder.finish()]);
        h.gpu
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(index),
                timeout: None,
            })
            .expect("poll after a chain frame");

        let px = h.gpu.read_output(&output, W, H).expect("readback");
        levels.push(px[crt_burnin::headless::px_index(W, W / 2, H / 2)][1]);
        decays.push(chain.last_burn_in_decay().expect("a decay was pushed"));
    }

    Run {
        levels,
        decays,
        loads: chain.loads(),
    }
}

fn fmt(samples: &[f32]) -> String {
    samples
        .iter()
        .map(|v| format!("{v:.5}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn the_mounted_accumulator_decays_at_the_rate_the_settings_set() {
    let step = decay_step(BURN_IN, DT.as_secs_f64());
    let r = run(true, FRAMES);

    // The ghost's origin is the picture's own level, twice quantised on the way
    // here (the 8-bit grid target, then the fp16 accumulator) and trimmed by
    // the composite. Measured rather than asserted to the last bit, because
    // what this test is about is the rate; that it is the *picture's* level and
    // not something else is the assert below.
    let origin = r.levels[1];
    let expected = |n: usize| origin - (n as f32 - 1.0) * step * SCALE;
    let errs: Vec<f32> = (2..FRAMES)
        .map(|n| ((r.levels[n] - expected(n)) / expected(n)).abs())
        .collect();
    let worst = errs.iter().fold(0.0f32, |a, b| a.max(*b));
    let worst12 = errs[..10].iter().fold(0.0f32, |a, b| a.max(*b));
    let drops: Vec<f32> = r.levels.windows(2).skip(1).map(|w| w[0] - w[1]).collect();
    let rate = drops
        .iter()
        .map(|d| ((d - step * SCALE) / (step * SCALE)).abs())
        .fold(0.0f32, f32::max);

    println!(
        "burn_in={BURN_IN} fade={}s dt={:.6}s",
        1.6,
        DT.as_secs_f64()
    );
    println!(
        "decay pushed per frame : {step:.6} (chain: {:.6})",
        r.decays[1]
    );
    println!("chain output at centre  : {}", fmt(&r.levels));
    println!("per-frame drops         : {}", fmt(&drops));
    println!("worst ramp error over 12 frames : {:.4}%", worst12 * 100.0);
    println!(
        "worst ramp error over {FRAMES} frames : {:.4}%",
        worst * 100.0
    );
    println!("worst per-frame rate error      : {:.4}%", rate * 100.0);

    // The ghost is the picture, at the trim and the grey conversion the
    // composite applies. One 8-bit step of the grid target is the slack.
    let picture = support::BAR_LEVEL * SCALE;
    assert!(
        (origin - picture).abs() < SCALE / 255.0,
        "the ghost starts at {origin:.5}, not at the picture's {picture:.5}: \
         the accumulator is holding something other than the frame it was fed"
    );
    assert!(
        r.levels[FRAMES - 1] > 0.0,
        "the run is too long: the ghost hit black at frame {}",
        FRAMES - 1
    );
    // The ramp is held to the accumulator's own resolution rather than to
    // the standard 0.5%, and that is not a loosened bound, it is the same
    // bound restated for a dimmer ghost. That bound was set from a ghost
    // starting at 1.0 that fell to 0.86 over fourteen frames; this one starts
    // at 0.30 and falls to 0.16, so the same absolute rounding is a larger
    // share of it. The rounding itself is what to hold: each frame's
    // subtraction lands on the fp16 grid
    // and the error can accumulate by at most one ULP per frame.
    for (i, err) in errs.iter().enumerate() {
        let n = i + 2;
        let bound = (n as f32 - 1.0) * FP16_ULP * SCALE;
        let deviation = (r.levels[n] - expected(n)).abs();
        assert!(
            deviation <= bound,
            "frame {n} is {deviation:.6} off the set ramp ({err:.4}% of it),              more than the {bound:.6} that {} frames of fp16 rounding can              explain",
            n - 1
        );
    }
    // And the rate itself, which is where a systematic bias shows up even when
    // the endpoints happen to line up. One fp16 ULP is 1.2% of a frame's drop
    // here, and twice that is the honest bound; the measurement sits inside
    // one.
    let rate_bound = 2.0 * FP16_ULP / step;
    assert!(
        rate < rate_bound,
        "rate error {:.4}% exceeds the fp16 quantisation bound {:.4}%",
        rate * 100.0,
        rate_bound * 100.0
    );

    // The decay reached the shader through a chain that was never rebuilt.
    assert_eq!(r.loads, 1, "the chain reloaded during a steady run");
    assert!(
        (r.decays[1] - step).abs() < 1e-6,
        "the chain pushed {} where the clock says {step}",
        r.decays[1]
    );
    assert_eq!(
        r.decays[0], 0.0,
        "the first frame has no previous frame to decay against"
    );
}

#[test]
fn the_freshness_mask_survives_the_mount() {
    let step = decay_step(BURN_IN, DT.as_secs_f64());
    let on = run(true, 4);
    let off = run(false, 4);

    println!("mask on  : {}", fmt(&on.levels));
    println!("mask off : {}", fmt(&off.levels));
    println!(
        "one frame of decay, through the composite: {:.5}",
        step * SCALE
    );

    // Frame 1 is the frame the mask pays for: the bar went dark, but it was lit
    // on the frame before, so the accumulator's `prevMask` cancels that frame's
    // decay exactly once. With the mask off it has already fallen a step.
    let gap = on.levels[1] - off.levels[1];
    assert!(
        gap > step * SCALE * 0.9,
        "mask on and mask off differ by {gap:.5} at frame 1, less than the one \
         step ({:.5}) they should: the alpha channel is not reaching the \
         accumulator's next frame through this preset",
        step * SCALE
    );
    // And they are back in step by frame 2, one frame apart on the same ramp.
    assert!(
        (on.levels[2] - off.levels[1]).abs() < step * SCALE * 0.1,
        "after the grace frame the two runs are not on the same ramp: {} vs {}",
        fmt(&on.levels),
        fmt(&off.levels)
    );
}

#[test]
fn switching_burn_in_off_takes_the_ghost_with_it() {
    // The strength is a uniform for exactly this: `burn_in = 0` has to
    // leave no ghost without a chain rebuild, which would reset every other
    // pass's state along with it.
    let _guard = serial();
    let h = support::Harness::new("burnin-off", W, H).expect("gpu");
    let mut cfg = config();
    cfg.screen.burn_in = 0.0;
    let mut chain = Chain::from_config(&h.gpu.device, &h.gpu.queue, &h.dir, &cfg).expect("chain");

    let output = h.gpu.make_output(W, H);
    let view = output.create_view(&wgpu::TextureViewDescriptor::default());
    let mut pacing = Pacing::new(Instant::now());

    let mut levels = Vec::new();
    for f in 0..4 {
        if f == 0 {
            h.draw_picture();
        } else {
            h.draw_dark();
        }
        let time = pacing.tick_by(DT);
        chain.set_params(&Params::build(&cfg, &geometry(), time, DegaussState::IDLE));
        let mut encoder = h
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("burn-in off frame"),
            });
        chain
            .frame(
                &h.input.texture,
                &view,
                (W, H),
                crt_burnin::headless::OUTPUT_FORMAT,
                &mut encoder,
                time,
            )
            .expect("chain frame");
        let index = h.gpu.queue.submit([encoder.finish()]);
        h.gpu
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(index),
                timeout: None,
            })
            .expect("poll");
        let px = h.gpu.read_output(&output, W, H).expect("readback");
        levels.push(px[crt_burnin::headless::px_index(W, W / 2, H / 2)][1]);
    }

    println!("burn_in=0: {}", fmt(&levels));
    assert_eq!(chain.loads(), 1, "switching burn-in off rebuilt the chain");
    assert_eq!(
        chain.last_burn_in_decay(),
        Some(1.0),
        "burn-in off must push a full decay every frame"
    );
    for (f, v) in levels.iter().enumerate().skip(1) {
        assert!(
            *v < 1e-3,
            "frame {f} still shows {v} of picture with burn-in switched off"
        );
    }
}
