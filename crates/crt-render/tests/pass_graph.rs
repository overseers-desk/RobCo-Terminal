//! What this suite verifies: the term target rendered through the chain, offscreen,
//! with every claim read back from pixels.
//!
//! Four properties, in this order:
//!
//! 1. a `term::gpu::Target` renders through the chain at all;
//! 2. a parameter change takes effect on the next frame with no chain reload,
//!    and the reload counter proves the "no reload" half rather than the
//!    absence of a log line;
//! 3. a structural change does rebuild the chain;
//! 4. the degauss hook visibly changes the picture, and stops changing it when
//!    its 200 ms are up.
//!
//! Two more were added when the real bloom and frame shader bodies replaced
//! scaffold stubs in the graph: each of those two passes puts
//! something other than transparent black on the glass, measured by readback.
//!
//! Determinism: the settings the tests drive have every animated axis at zero
//! (noise, jitter, flicker, sync, glowing line), and the clock is a
//! [`Pacing`](crt::Pacing) advanced by hand rather than by `Instant::now()`, so
//! the same run gives the same bytes.

mod support;

use std::time::{Duration, Instant};

use config::Config;
use crt::{Applied, Chain, Degauss, DegaussState, Geometry, Pacing, Params, Structure};

const W: u32 = 128;
const H: u32 = 128;

/// A settings snapshot with nothing that moves on its own, and nothing over the
/// picture: the tests below measure where the terminal image lands and how
/// bright it is, so anything that draws on top of it is turned off rather than
/// subtracted afterwards.
///
/// The frame is three keys, not one. `frame_size` is chassis-or-screen
/// governed and the shipped default stands a chassis, so zeroing the
/// screen's own moulding leaves the chassis bezel drawing; and
/// `ambient_light` alone is enough to switch the frame pass on, because
/// `frameEnabled` is `ambientLight > 0 || frameSize > 0 || screenCurvature >
/// 0` and the pass paints a glass sheen across the whole screen for it.
fn still_config() -> Config {
    let mut cfg = Config::default();
    cfg.chassis.frame_size = 0.0;
    let s = &mut cfg.screen;
    s.flickering = 0.0;
    s.horizontal_sync = 0.0;
    s.static_noise = 0.0;
    s.jitter = 0.0;
    s.glowing_line = 0.0;
    s.burn_in = 0.0;
    s.bloom = 0.0;
    s.rgb_shift = 0.0;
    s.screen_curvature = 0.0;
    s.frame_size = 0.0;
    s.frame_shininess = 0.0;
    s.ambient_light = 0.0;
    s.chroma_color = 0.0;
    // 0.0 on the slider is `lint(0.5, 1.5, 0.0)` = a half-brightness screen.
    s.brightness = 0.0;
    s.font_color = "#ffffff".into();
    s.background_color = "#000000".into();
    // White on black is what these tests measure against, and at any
    // contrast below 1.0 that is not what reaches the shader: the two
    // profile colours mix into each other by `0.7 + contrast * 0.3`,
    // so the shipped 0.8 lifts the background off black by six percent and
    // every "the picture is dark here" reading becomes a reading of that.
    // Contrast 1.0 makes the mix the identity; saturation 0 leaves the white
    // white rather than mixing it with itself.
    s.contrast = 1.0;
    s.saturation_color = 0.0;
    cfg
}

fn geometry() -> Geometry {
    Geometry {
        output_width: W as f32,
        output_height: H as f32,
        virtual_width: 64.0,
        virtual_height: 64.0,
        total_font_scaling: 1.0,
        device_pixel_ratio: 1.0,
    }
}

#[test]
fn the_term_target_renders_through_the_chain() {
    let h = support::Harness::new("render", W, H).expect("gpu");
    h.draw_picture();

    let cfg = still_config();
    let mut chain = Chain::from_config(&h.gpu.device, &h.gpu.queue, &h.dir, &cfg).expect("chain");
    let mut pacing = Pacing::new(Instant::now());

    let time = pacing.tick_by(Duration::from_millis(16));
    chain.set_params(&Params::build(&cfg, &geometry(), time, DegaussState::IDLE));
    let img = h.render(&mut chain, time);

    // The picture is a bright bar across the middle third of a black field, so
    // the chain's output should be lit in the middle and dark at the edges.
    let rows = support::lit_rows(&img);
    assert!(
        !rows.is_empty(),
        "the chain produced no lit pixels at all: the term target did not reach it"
    );
    let (first, last) = (rows[0], rows[rows.len() - 1]);
    assert!(
        first > 0 && last < H - 1,
        "the lit band runs from row {first} to row {last} of {H}, which is the \
         whole image rather than the middle third"
    );
    println!(
        "chain output: {} lit rows, {first}..={last} of {H}, mean luma {:.4}",
        rows.len(),
        support::mean_luma(&img)
    );
}

#[test]
fn a_parameter_change_takes_effect_next_frame_without_a_reload() {
    let h = support::Harness::new("params", W, H).expect("gpu");
    h.draw_picture();

    let mut cfg = still_config();
    let mut chain = Chain::from_config(&h.gpu.device, &h.gpu.queue, &h.dir, &cfg).expect("chain");
    let loads_at_start = chain.loads();
    let mut pacing = Pacing::new(Instant::now());
    let geom = geometry();

    let t0 = pacing.tick_by(Duration::from_millis(16));
    chain.set_params(&Params::build(&cfg, &geom, t0, DegaussState::IDLE));
    let dim = h.render(&mut chain, t0);

    // `brightness` is a parameter-level key: it is not in
    // `app::settings::STRUCTURAL_KEYS`, so the chain must take it as a uniform.
    // 0.0 -> 0.5 on the slider is 0.5x -> 1.0x on the screen, and the test bar
    // sits low enough that neither end clamps.
    cfg.screen.brightness = 0.5;
    let applied = chain
        .apply_settings(&h.gpu.device, &h.gpu.queue, &cfg)
        .expect("apply");
    assert_eq!(
        applied,
        Applied::Parameters,
        "a brightness change was classified as structural"
    );

    let t1 = pacing.tick_by(Duration::from_millis(16));
    chain.set_params(&Params::build(&cfg, &geom, t1, DegaussState::IDLE));
    let bright = h.render(&mut chain, t1);

    assert_eq!(
        chain.loads(),
        loads_at_start,
        "the chain was reloaded {} time(s) for a parameter change",
        chain.loads() - loads_at_start
    );

    let (dim_mean, bright_mean) = (support::mean_luma(&dim), support::mean_luma(&bright));
    assert!(
        bright_mean > dim_mean * 1.5,
        "raising brightness from 0.0 to 0.5 moved mean luma only {dim_mean:.4} \
         -> {bright_mean:.4}, so the uniform did not reach the shader"
    );
    println!(
        "parameter change: mean luma {dim_mean:.4} -> {bright_mean:.4} on the \
         next frame, chain loads still {}",
        chain.loads()
    );
}

#[test]
fn a_structural_change_rebuilds_the_chain() {
    let h = support::Harness::new("structural", W, H).expect("gpu");
    h.draw_picture();

    let mut cfg = still_config();
    let mut chain = Chain::from_config(&h.gpu.device, &h.gpu.queue, &h.dir, &cfg).expect("chain");
    assert_eq!(chain.loads(), 1, "the first load counts as one");

    // A parameter-level key first, so the counter's stillness is a measurement
    // rather than a default.
    cfg.screen.static_noise = 0.4;
    assert_eq!(
        chain
            .apply_settings(&h.gpu.device, &h.gpu.queue, &cfg)
            .expect("apply"),
        Applied::Parameters
    );
    assert_eq!(chain.loads(), 1);

    // `general.burn_in_quality` is in `STRUCTURAL_KEYS`: it is the accumulator's
    // framebuffer size, which no uniform can change. Its default is 0.5, so the
    // change has to be to something else for this to test anything.
    let before = chain.structure();
    assert_eq!(cfg.general.burn_in_quality, 0.5);
    cfg.general.burn_in_quality = 0.25;
    assert_eq!(
        chain
            .apply_settings(&h.gpu.device, &h.gpu.queue, &cfg)
            .expect("apply"),
        Applied::Rebuilt
    );
    assert_eq!(chain.loads(), 2, "the structural change did not reload");
    assert_ne!(chain.structure(), before);

    // And the preset on disk carries the new scale, since that is what the
    // rebuild had to be for.
    let text = std::fs::read_to_string(chain.preset_path()).expect("preset");
    assert!(
        text.contains("scale0 = \"0.250000\""),
        "the rewritten preset does not carry the new accumulator scale:\n{text}"
    );

    // The chain still renders after the rebuild.
    let mut pacing = Pacing::new(Instant::now());
    let t = pacing.tick_by(Duration::from_millis(16));
    chain.set_params(&Params::build(&cfg, &geometry(), t, DegaussState::IDLE));
    let img = h.render(&mut chain, t);
    assert!(!support::lit_rows(&img).is_empty());
    println!(
        "structural change: burn_in_quality 0.5 -> 0.25 took the chain from 1 \
         load to {}, and the rebuilt chain still draws",
        chain.loads()
    );
}

#[test]
fn the_degauss_hook_alters_the_output_for_its_duration() {
    let h = support::Harness::new("degauss", W, H).expect("gpu");
    // A full lit field, so the squeeze has edges to move; see `draw_field`.
    h.draw_field();

    let cfg = still_config();
    let mut chain = Chain::from_config(&h.gpu.device, &h.gpu.queue, &h.dir, &cfg).expect("chain");
    let geom = geometry();
    let epoch = Instant::now();
    let mut pacing = Pacing::new(epoch);
    let mut degauss = Degauss::new();

    let frame = |chain: &mut Chain, pacing: &mut Pacing, state: DegaussState| {
        let t = pacing.tick_by(Duration::from_millis(20));
        chain.set_params(&Params::build(&cfg, &geom, t, state));
        h.render(chain, t)
    };

    let idle = frame(&mut chain, &mut pacing, DegaussState::IDLE);
    let idle_rows = support::lit_rows(&idle);
    let idle_mean = support::mean_luma(&idle);

    assert_eq!(
        idle_rows.len() as u32,
        H,
        "the resting field should light every row"
    );

    // Trigger and sample at the keyframe itself, where the mockup fixes the
    // values: brightness 2.6 and scaleY 0.97.
    degauss.trigger(epoch);
    let peak = degauss.sample(epoch);
    assert!(peak.is_active(), "the hook reported nothing running");
    assert_eq!(peak.brightness, 2.6);
    assert_eq!(peak.scale_y, 0.97);
    let mid = frame(&mut chain, &mut pacing, peak);

    let mid_rows = support::lit_rows(&mid);
    let mid_mean = support::mean_luma(&mid);

    // The brightness half: the picture is measurably brighter.
    assert!(
        mid_mean > idle_mean * 1.5,
        "degauss did not brighten the picture: mean luma {idle_mean:.4} idle \
         vs {mid_mean:.4} during the transient"
    );
    // The squeeze half: a 3% squeeze about the centre of a 128-row image opens
    // a band of `0.5 * 0.03 * 128` = 1.92 rows at each end, which rounds to two
    // rows top and bottom.
    assert_eq!(
        mid_rows.len() as u32,
        H - 4,
        "degauss did not squeeze the picture by the two rows at each end that \
         scaleY 0.97 costs: {} lit rows idle vs {}",
        idle_rows.len(),
        mid_rows.len()
    );
    assert_eq!(
        mid_rows[0], 2,
        "the top of the squeezed picture is at row {}, not row 2",
        mid_rows[0]
    );

    // Halfway through it is still running, and still squeezed.
    let half = degauss.sample(epoch + Duration::from_millis(100));
    assert!(half.is_active());
    assert!(half.brightness < peak.brightness && half.brightness > 1.0);
    assert!(half.scale_y > peak.scale_y && half.scale_y < 1.0);

    // Past 200 ms the hook is idle again and the picture is the one it started
    // as, byte for byte.
    let after_state = degauss.sample(epoch + Duration::from_millis(200));
    assert_eq!(after_state, DegaussState::IDLE);
    assert!(!degauss.is_running(epoch + Duration::from_millis(200)));
    let after = frame(&mut chain, &mut pacing, after_state);
    let diff = idle.diff(&after);
    assert_eq!(
        diff.differing,
        0,
        "the picture did not return to its resting state after 200 ms: {}",
        diff.describe()
    );

    println!(
        "degauss: at the keyframe, mean luma {idle_mean:.4} -> {mid_mean:.4} \
         and lit rows {} -> {} (two blanked at each end); back to a \
         byte-identical frame at 200 ms",
        idle_rows.len(),
        mid_rows.len()
    );
}

/// Mean luma over a band of rows, so a claim about the dark part of the picture
/// is not diluted by the lit part.
fn mean_luma_rows(img: &term::gpu::Image, rows: std::ops::Range<u32>) -> f64 {
    let mut sum = 0u64;
    let mut count = 0u64;
    for y in rows {
        for x in 0..img.width {
            let px = img.pixel(x, y);
            sum += px[0].max(px[1]).max(px[2]) as u64;
            count += 1;
        }
    }
    sum as f64 / count as f64 / 255.0
}

/// The bloom passes contribute a non-black picture.
///
/// Measured where only a blur can reach: the dark rows well above the bar. The
/// static pass adds `clamp(bloomOnScreen * Bloom * bloomAlpha, 0, 0.5)` to a
/// pixel that is otherwise black there, so any light in that band came out of
/// `BloomSource` and nowhere else. The scaffold stub this pass replaced emitted
/// transparent black, which would leave the band exactly as dark as with the
/// bloom setting at zero.
#[test]
fn the_bloom_passes_light_pixels_the_terminal_left_dark() {
    let h = support::Harness::new("bloom", W, H).expect("gpu");
    h.draw_picture();

    let mut cfg = still_config();
    let mut chain = Chain::from_config(&h.gpu.device, &h.gpu.queue, &h.dir, &cfg).expect("chain");
    let mut pacing = Pacing::new(Instant::now());
    let geom = geometry();

    // The bar covers rows 48..80 of 128; this band is 16 rows clear of it.
    const DARK: std::ops::Range<u32> = 0..32;

    let t0 = pacing.tick_by(Duration::from_millis(16));
    chain.set_params(&Params::build(&cfg, &geom, t0, DegaussState::IDLE));
    let off = h.render(&mut chain, t0);
    let off_dark = mean_luma_rows(&off, DARK);

    cfg.screen.bloom = 0.5;
    assert_eq!(
        chain
            .apply_settings(&h.gpu.device, &h.gpu.queue, &cfg)
            .expect("apply"),
        Applied::Parameters,
        "the bloom *strength* is a uniform; only bloom_quality is structural"
    );
    let t1 = pacing.tick_by(Duration::from_millis(16));
    chain.set_params(&Params::build(&cfg, &geom, t1, DegaussState::IDLE));
    let on = h.render(&mut chain, t1);
    let on_dark = mean_luma_rows(&on, DARK);

    assert!(
        on_dark > off_dark + 0.01,
        "the rows above the bar are as dark with bloom 0.5 ({on_dark:.4}) as \
         with bloom 0.0 ({off_dark:.4}): the bloom passes emitted nothing"
    );
    println!(
        "bloom: rows {}..{} of {H} go from mean luma {off_dark:.4} to \
         {on_dark:.4} when the setting moves 0.0 -> 0.5, radius {:.1}",
        DARK.start,
        DARK.end,
        Params::build(&cfg, &geom, t1, DegaussState::IDLE)
            .get("radius")
            .expect("the bloom radius uniform")
    );
}

/// The frame pass contributes a non-black picture.
///
/// `FrameEnabled` is the one uniform that gates nothing but this pass's
/// composite (`frameColor = texture(FrameSource, uv) * FrameEnabled` in the
/// dynamic pass), so rendering the same settings with it at 1 and at 0 isolates
/// the pass's own output. The scaffold stub emitted transparent black, which
/// composites to nothing whichever way the switch is thrown.
#[test]
fn the_frame_pass_draws_a_moulding_over_the_glass() {
    let h = support::Harness::new("frame", W, H).expect("gpu");
    h.draw_picture();

    let mut cfg = still_config();
    // A moulding to draw, and a room to light it: the frame's own two inputs.
    cfg.chassis.frame_size = 0.45;
    cfg.screen.ambient_light = 0.3;
    let mut chain = Chain::from_config(&h.gpu.device, &h.gpu.queue, &h.dir, &cfg).expect("chain");
    let mut pacing = Pacing::new(Instant::now());
    let geom = geometry();

    let t0 = pacing.tick_by(Duration::from_millis(16));
    let params = Params::build(&cfg, &geom, t0, DegaussState::IDLE);
    assert_eq!(params.get("FrameEnabled"), Some(1.0));
    chain.set_params(&params);
    let with_frame = h.render(&mut chain, t0);

    let mut without = params.clone();
    without.set("FrameEnabled", 0.0);
    let t1 = pacing.tick_by(Duration::from_millis(16));
    chain.set_params(&without);
    let no_frame = h.render(&mut chain, t1);

    let diff = with_frame.diff(&no_frame);
    assert!(
        diff.differing > 0,
        "the picture is byte-identical with the frame composite on and off: \
         the frame pass emitted transparent black"
    );

    // And the moulding is where a moulding goes. The frame's tint is opaque
    // outside the rounded screen rectangle, so the very corner is the pass's
    // own colour and nothing else's.
    let corner = with_frame.pixel(1, 1);
    assert!(
        corner[0].max(corner[1]).max(corner[2]) > 8,
        "the corner of the framed picture is {corner:?}, i.e. still black"
    );
    println!(
        "frame: {} of {} pixels move when FrameEnabled goes 1 -> 0, max channel \
         delta {}; corner pixel {corner:?}",
        diff.differing,
        W * H,
        diff.max_channel_delta
    );
}

#[test]
fn the_preset_is_the_documented_six_pass_graph() {
    let mut cfg = Config::default();
    // The bare tube, so every pass answers with the body in `PASSES`; the
    // frame-source slot's other occupant is the next test's business.
    cfg.general.chassis_shown = false;
    let text = Structure::from_config(&cfg).preset_text();
    for (i, pass) in crt::preset::PASSES.iter().enumerate() {
        assert!(
            text.contains(&format!("shader{i} = \"{}\"", pass.file)),
            "pass {i} ({}) is not at its documented index",
            pass.alias
        );
        assert!(text.contains(&format!("alias{i} = \"{}\"", pass.alias)));
    }
    // The two directives the burn-in slot cannot lose.
    assert!(text.contains("alias0 = \"Burn\""));
    assert!(text.contains("float_framebuffer0 = \"true\""));
    // The noise LUT.
    assert!(text.contains("NoiseSource_wrap_mode = repeat"));
}

/// The frame-source slot's two occupants, and the rebuild between them.
///
/// A window standing inside a chassis wears the shell's bezel where a bare
/// tube wears the screen's own moulding. Here that is a structural change,
/// because it is the preset text that moves, so it has to rebuild rather
/// than push a uniform.
#[test]
fn the_chassis_bezel_takes_the_frame_slot_and_rebuilds_to_get_there() {
    use crt::preset::{CHASSIS_FRAME, FRAME_PASS, PASSES};

    let bare = Structure::from_config(&{
        let mut cfg = Config::default();
        cfg.general.chassis_shown = false;
        cfg
    });
    let cabinet = Structure::from_config(&Config::default());
    assert!(
        Config::default().general.chassis_shown,
        "the shipped profile stands in a chassis, which is what makes the \
         bezel the default occupant"
    );
    assert_ne!(
        bare, cabinet,
        "the two differ only in `general.chassis_shown`, and that has to be \
         structural or the slot would never be re-mounted"
    );

    let alias = format!("alias{FRAME_PASS} = \"{}\"", PASSES[FRAME_PASS].alias);
    for (name, structure, want) in [
        ("bare tube", bare, PASSES[FRAME_PASS].file),
        ("in a chassis", cabinet, CHASSIS_FRAME.file),
    ] {
        let text = structure.preset_text();
        assert!(
            text.contains(&format!("shader{FRAME_PASS} = \"{want}\"")),
            "{name}: the frame slot does not name {want}:\n{text}"
        );
        // Same slot, same alias: the passes around it sample `FrameSource`
        // either way and cannot tell which body wrote it.
        assert!(text.contains(&alias), "{name}: the slot lost its alias");
    }

    // And the switch survives materialisation onto disk, which is what the
    // chain actually loads: both bodies have to be written out under the name
    // their preset refers to.
    let h = support::Harness::new("frame-slot", W, H).expect("gpu");
    h.draw_picture();
    let mut cfg = Config::default();
    cfg.general.chassis_shown = false;
    let mut chain = Chain::from_config(&h.gpu.device, &h.gpu.queue, &h.dir, &cfg).expect("chain");
    assert_eq!(chain.loads(), 1);

    cfg.general.chassis_shown = true;
    chain
        .apply_settings(&h.gpu.device, &h.gpu.queue, &cfg)
        .expect("the bezel mounts in the frame slot");
    assert_eq!(
        chain.loads(),
        2,
        "showing the chassis did not rebuild the chain"
    );
    assert_eq!(chain.structure(), Structure::from_config(&cfg));

    // It draws: a bezel that mounted but emitted nothing would pass every
    // assertion above.
    let mut pacing = Pacing::new(Instant::now());
    let t = pacing.tick_by(Duration::from_millis(16));
    let mut params = Params::build(&cfg, &geometry(), t, DegaussState::IDLE);
    chain.set_params(&params);
    let with_bezel = h.render(&mut chain, t);
    params.set("FrameEnabled", 0.0);
    let t2 = pacing.tick_by(Duration::from_millis(16));
    chain.set_params(&params);
    let without = h.render(&mut chain, t2);
    let diff = with_bezel.diff(&without);
    assert!(
        diff.differing > 0,
        "the bezel composites to nothing: the metal mounted but drew black"
    );
    println!(
        "chassis bezel: {} of {} pixels move when FrameEnabled goes 1 -> 0",
        diff.differing,
        W * H
    );
}

/// The shell's casting reaches the glass, not just the parameter list.
///
/// With a chassis shown -- the shipped default -- the frame slot carries
/// `frame_metal.slang`, which names 36 uniforms. The chain used to push only
/// the five it shares by name with `terminal_frame` and leave the other
/// 31 at their `#pragma parameter` defaults: a mid-grey plate with a pale ridge
/// standing where the annunciator's aged gunmetal (`#26211c`, ridge `#6e5c48`)
/// should be. Nothing failed, and nothing said so; the bezel simply came out
/// the wrong colour.
///
/// So this is a readback rather than a parameter assertion (`contracts.rs`
/// makes that one). It renders the shipped bezel twice, once with the casting's
/// own colour and once with the shader's default in its place, and asks whether
/// the presented picture can tell.
#[test]
fn a_bezel_colour_change_shows_on_the_presented_frame() {
    let h = support::Harness::new("bezel-colour", W, H).expect("gpu");
    h.draw_picture();

    let mut cfg = still_config();
    // The chassis's own bezel, at the depth the shipped profile cuts it, in a
    // lit room: a frame with no size and no light draws nothing to colour.
    assert!(cfg.general.chassis_shown);
    cfg.chassis.frame_size = 0.45;
    cfg.screen.ambient_light = 0.3;

    let mut chain = Chain::from_config(&h.gpu.device, &h.gpu.queue, &h.dir, &cfg).expect("chain");
    let mut pacing = Pacing::new(Instant::now());

    let t0 = pacing.tick_by(Duration::from_millis(16));
    let mut params = Params::build(&cfg, &geometry(), t0, DegaussState::IDLE);

    // The casting the chain now pushes.
    let cast = [
        params.get("bezelColorR").expect("the casting is pushed"),
        params.get("bezelColorG").unwrap(),
        params.get("bezelColorB").unwrap(),
    ];
    // ...which is not what the shader would have used on its own.
    let stock = [0.32_f32, 0.32, 0.34];
    assert!(
        cast.iter().zip(stock).any(|(a, b)| (a - b).abs() > 0.05),
        "the shell's bezel colour {cast:?} is the shader's own default, so this \
         test could not tell a pushed value from an unpushed one"
    );

    chain.set_params(&params);
    let with_casting = h.render(&mut chain, t0);

    for (name, value) in [
        ("bezelColorR", stock[0]),
        ("bezelColorG", stock[1]),
        ("bezelColorB", stock[2]),
    ] {
        params.set(name, value);
    }
    let t1 = pacing.tick_by(Duration::from_millis(16));
    chain.set_params(&params);
    let with_default = h.render(&mut chain, t1);

    let diff = with_casting.diff(&with_default);
    assert!(
        diff.differing > 0,
        "the bezel colour reaches no pixel of the presented frame: the uniform \
         is pushed but the body in the slot is not reading it"
    );

    // And it moves the frame, not the picture: the bezel plate is the border
    // of the image, and the terminal image in the middle of it has no business
    // taking the plate's colour.
    let (x, y, _, _) = diff.first.expect("a differing pixel");
    let inset = W / 4;
    assert!(
        x < inset || x >= W - inset || y < inset || y >= H - inset,
        "the first pixel the bezel colour moved is ({x},{y}), which is inside \
         the middle half of the glass rather than out on the moulding"
    );
    let centre = (W / 2, H / 2);
    assert_eq!(
        with_casting.pixel(centre.0, centre.1),
        with_default.pixel(centre.0, centre.1),
        "the bezel's colour tinted the centre of the picture"
    );
    println!(
        "bezel colour: {} of {} pixels move between the casting {cast:?} and the \
         shader default {stock:?}, max channel delta {}",
        diff.differing,
        W * H,
        diff.max_channel_delta
    );
}
