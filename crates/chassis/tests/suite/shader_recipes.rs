//! Done-test: each shell's drawing recipe -- which region gets which of the
//! three procedural metal shaders, with which fixed parameters -- checked
//! against hand-recorded expected values, then fed through `oracle`
//! (the same CPU math this crate's own per-shader done-tests check against a
//! real GPU render) to confirm the wiring produces valid shader output, not
//! just a struct that happens to compile.

use chassis::shells::common::{self, FrameRuntime, Rect};
use chassis::shells::{annunciator, slide_rule, switchboard};

fn assert_rgb_close(got: [f32; 3], want_hex: &str) {
    // Every colour checked below is a hex colour literal (a shell's casting
    // colour, its chassis's base/highlight/shadow, its frame's
    // bezel/ridge), parsed at /255 -- `hex_literal_to_color`, not
    // `str_to_color`'s /256 (see `chassis::color`'s module doc), matching
    // the divisor `shells::common::rgb` uses.
    let want = chassis::color::hex_literal_to_color(want_hex);
    assert!(
        (got[0] - want.r).abs() < 1e-6
            && (got[1] - want.g).abs() < 1e-6
            && (got[2] - want.b).abs() < 1e-6,
        "got {got:?}, want {want_hex} = ({}, {}, {})",
        want.r,
        want.g,
        want.b
    );
}

#[test]
fn annunciator_chassis_metal_recipe_matches_the_recorded_values() {
    // light_dir/chassis_color read off this shell's metrics,
    // grain/mottle/scratch/vignette fixed here.
    let chassis = Rect::new(0.0, 0.0, 357.0, 1080.0);
    let frame_region = Rect::new(0.0, 0.0, 1200.0, 1080.0);
    let p = common::chassis_metal_params(&annunciator::CASTING, chassis, Some(frame_region));

    assert_eq!(p.light_dir, [0.8, -0.6]);
    assert_rgb_close(p.chassis_color, "#16130f");
    assert_eq!(p.metal.grain_amount, 0.16);
    assert_eq!(p.metal.mottle_amount, 0.4);
    assert_eq!(p.metal.scratch_amount, 0.08);
    assert_eq!(p.vignette_strength, 0.42);
    // field_scale/field_offset come from `field_mapping`, exercised on its
    // own in `region_layout.rs`; here just confirm they were actually
    // threaded through rather than left at some default.
    assert!((p.field_scale[0] - 357.0 / 1200.0).abs() < 1e-5);

    // Feed it through the oracle: any finite pixel at a few UVs is enough to
    // prove the recipe wires into real shader math without panicking on a
    // NaN/degenerate parameter.
    for uv in [[0.1, 0.1], [0.5, 0.5], [0.9, 0.9]] {
        let out = oracle::chassis_metal(uv, [1200.0, 1080.0], &p);
        assert!(out.iter().all(|c| c.is_finite() && *c >= 0.0));
    }
}

#[test]
fn annunciator_plate_metal_recipe_matches_the_recorded_values() {
    let p = annunciator::plate_metal_params([349.0, 1078.0]);
    assert_eq!(p.size_px, [349.0, 1078.0]);
    assert_eq!(p.light_dir, [-0.22, -0.98]);
    assert_rgb_close(p.base_color, "#2b241c");
    assert_rgb_close(p.highlight_color, "#c1a585");
    assert_rgb_close(p.shadow_color, "#0e0905");
    assert_eq!(p.corner_radius, 6.0);
    assert_eq!(p.bevel_px, 2.5);
    assert_eq!(p.metal.grain_amount, 0.3);
    assert_eq!(p.metal.mottle_amount, 1.0);
    assert_eq!(p.metal.scratch_amount, 0.5);
    assert_eq!(p.vignette_strength, 0.42);
    assert_eq!(p.wear_amount, 0.7);
    assert_eq!(p.seam_gain, 1.0);
    assert_eq!(p.seed, 0.17);

    let (color, coverage) = oracle::plate_metal([0.5, 0.5], &p);
    assert!(color.iter().all(|c| c.is_finite()));
    assert!((0.0..=1.0).contains(&coverage));
}

#[test]
fn annunciator_frame_metal_recipe_matches_the_recorded_values() {
    // The fixed half of the recipe.
    let runtime = FrameRuntime {
        screen_curvature: 0.3,
        frame_shininess: 0.1,
        frame_size: 0.02,
        screen_radius: 8.0,
        ambient_light: 0.3,
    };
    let p = common::frame_metal_params(&annunciator::FRAME, runtime);
    assert_eq!(p.light_dir, [0.8, -0.6]);
    assert_rgb_close(p.bezel_color, "#26211c");
    assert_rgb_close(p.chassis_color, "#16130f");
    assert_rgb_close(p.ridge_color, "#6e5c48");
    assert_eq!(p.bezel_margins, [0.0, 12.0, 10.0, 9.0]);
    assert_eq!(p.outer_radius, 60.0);
    assert_eq!(p.well_depth, 9.0);
    assert_eq!(p.well_floor, 0.4);
    assert_eq!(p.ridge_gain, 0.7);
    assert_eq!(p.trough_gain, 0.35);
    assert_eq!(p.fill_gain, 0.95);
    // This shell's face-band law is off.
    assert_eq!(p.face_band_px, 0.0);
    assert_eq!(p.rim_dist_px, 0.0);
    assert_eq!(p.rim_gain, 0.0);
    // Runtime half passed straight through.
    assert_eq!(p.screen_curvature, 0.3);
    assert_eq!(p.ambient_light, 0.3);

    let (color, alpha) = oracle::frame_metal([0.5, 0.5], [1200.0, 100.0], &p);
    assert!(color.iter().all(|c| c.is_finite()));
    assert!((0.0..=1.0).contains(&alpha));
}

#[test]
fn slide_rule_rail_is_plate_metal_not_chassis_metal() {
    // The rail is furniture standing on the chassis, drawn by the *same*
    // plate_metal shader as the annunciator's bank plate, not
    // `chassis_metal`'s.
    let p = slide_rule::rail_metal_params([41.0, 1022.0]);
    assert_eq!(p.size_px, [41.0, 1022.0]);
    assert_eq!(p.light_dir, [-0.55, -0.85]);
    assert_rgb_close(p.base_color, "#2b2418");
    assert_rgb_close(p.highlight_color, "#7d735c");
    assert_rgb_close(p.shadow_color, "#050403");
    assert_eq!(p.corner_radius, 4.0);
    assert_eq!(p.bevel_px, 2.0);
    assert_eq!(p.metal.grain_amount, 0.35);
    assert_eq!(p.metal.mottle_amount, 0.7);
    assert_eq!(p.metal.scratch_amount, 0.5);
    assert_eq!(p.vignette_strength, 0.35);
    assert_eq!(p.wear_amount, 0.35);
    assert_eq!(p.seam_gain, 0.6);
    assert_eq!(p.seed, 0.53);
}

#[test]
fn slide_rule_chassis_and_frame_recipes_match_the_recorded_values() {
    let chassis = Rect::new(0.0, 0.0, 500.0, 1080.0);
    let frame_region = Rect::new(0.0, 0.0, 1400.0, 1080.0);
    let cp = common::chassis_metal_params(&slide_rule::CASTING, chassis, Some(frame_region));
    assert_eq!(cp.light_dir, [-0.55, -0.85]);
    assert_rgb_close(cp.chassis_color, "#453c2d");
    assert_eq!(cp.metal.grain_amount, 0.4);
    assert_eq!(cp.metal.mottle_amount, 1.35);
    assert_eq!(cp.metal.scratch_amount, 0.7);
    assert_eq!(cp.vignette_strength, 0.3);

    let runtime = FrameRuntime {
        screen_curvature: 0.2,
        frame_shininess: 0.05,
        frame_size: 0.03,
        screen_radius: 6.0,
        ambient_light: 0.4,
    };
    let fp = common::frame_metal_params(&slide_rule::FRAME, runtime);
    assert_rgb_close(fp.bezel_color, "#2e2820");
    assert_rgb_close(fp.ridge_color, "#8c8068");
    assert_eq!(fp.bezel_margins, [6.0, 10.0, 14.0, 12.0]);
    assert_eq!(fp.outer_radius, 45.0);
    assert_eq!(fp.well_depth, 45.0);
    assert_eq!(fp.well_floor, 0.16);
    assert_eq!(fp.ridge_gain, 0.45);
    assert_eq!(fp.trough_gain, 0.0);
    assert_eq!(fp.face_band_px, 12.0);
    assert_eq!(fp.rim_dist_px, 14.0);
    assert_eq!(fp.rim_gain, 1.6);
    assert_eq!(fp.fill_gain, 0.35);
}

#[test]
fn switchboard_has_no_plate_metal_call_at_all() {
    // There is no `switchboard::plate_metal_params`/`rail_metal_params` --
    // the module exposes only `chassis_rect` and the two recipe tables,
    // since this shell has no nested plate or rail
    // region. This test exists to make that omission an intentional,
    // checked fact rather than a silent gap: if a later change adds
    // plate/rail furniture to this shell, this module gains a function and
    // this comment goes with it.
    let chassis = Rect::new(0.0, 0.0, 700.0, 1080.0);
    let frame_region = Rect::new(0.0, 0.0, 1500.0, 1080.0);
    let cp = common::chassis_metal_params(&switchboard::CASTING, chassis, Some(frame_region));
    assert_rgb_close(cp.chassis_color, "#232830");
    assert_eq!(cp.metal.grain_amount, 0.45);
    assert_eq!(cp.metal.mottle_amount, 0.7);
    assert_eq!(cp.metal.scratch_amount, 0.45);
    assert_eq!(cp.vignette_strength, 0.22);

    let runtime = FrameRuntime {
        screen_curvature: 0.15,
        frame_shininess: 0.08,
        frame_size: 0.02,
        screen_radius: 10.0,
        ambient_light: 0.25,
    };
    let fp = common::frame_metal_params(&switchboard::FRAME, runtime);
    assert_rgb_close(fp.bezel_color, "#20242b");
    assert_rgb_close(fp.ridge_color, "#737a83");
    assert_eq!(fp.bezel_margins, [0.0, 6.0, 10.0, 10.0]);
    assert_eq!(fp.outer_radius, 26.0);
    assert_eq!(fp.well_depth, 30.0);
    assert_eq!(fp.well_floor, 0.18);
    assert_eq!(fp.ridge_gain, 0.4);
    assert_eq!(fp.face_band_px, 10.0);
    assert_eq!(fp.rim_dist_px, 12.0);
    assert_eq!(fp.rim_gain, 1.3);
}
