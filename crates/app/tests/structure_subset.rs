//! `crt::Structure` and `app::settings::STRUCTURAL_KEYS` are two independent
//! encodings of "what forces a filter-chain rebuild": the render layer
//! derives and compares `Structure`s itself on every settings change, and
//! `app::settings::classify()` is only informational (logging). They are not
//! tied together by construction, so this test holds the one relation that
//! has to be true regardless: every config field `Structure::from_config`
//! reads must be named in `STRUCTURAL_KEYS`, or a config change the render
//! layer treats as structural would go unlogged as one.
//!
//! The field list below is hand-kept in step with `crt::preset::Structure`'s
//! fields (there is no reflection to derive it from); adding a field to
//! `Structure` means adding its config key here too.

const STRUCTURE_FIELDS: &[&str] = &[
    "general.window_scaling",
    "general.bloom_quality",
    "general.burn_in_quality",
    // Not a framebuffer size like the other three: it decides which of the
    // frame-source slot's two bodies the preset names
    // (`crt::preset::CHASSIS_FRAME`).
    "general.chassis_shown",
];

#[test]
fn structure_fields_are_a_subset_of_structural_keys() {
    let structural_keys = app::settings::structural_keys();
    for field in STRUCTURE_FIELDS {
        assert!(
            structural_keys.contains(field),
            "`crt::preset::Structure::from_config` reads `{field}`, but it is \
             missing from `app::settings::STRUCTURAL_KEYS`"
        );
    }
}

/// The frame's size is computed twice from the same settings, once on each
/// side of a crate boundary neither can cross: `crt::params` bends the picture
/// by it, `app::settings` un-bends a pointer position by it (through
/// `term::distortion`). If they ever disagree, a click lands somewhere other
/// than where the character under the cursor is drawn, and nothing about the
/// picture looks wrong -- which is why this is a test and not a comment.
///
/// The two sides have disagreed before: the render side had neither the
/// `* 0.05` scale nor the chassis-or-screen split the formula requires, so
/// it was drawing a moulding four and a half times too deep while the
/// pointer inverted the right one.
#[test]
fn both_crates_derive_the_same_frame_size() {
    use config::Config;
    use crt::{DegaussState, Geometry, Params};
    use std::time::{Duration, Instant};

    let geom = Geometry {
        output_width: 1448.0,
        output_height: 1086.0,
        virtual_width: 724.0,
        virtual_height: 543.0,
        total_font_scaling: 0.75,
        device_pixel_ratio: 1.0,
    };
    let normalized = term::distortion::normalized_screen_scale(
        f64::from(geom.output_width),
        f64::from(geom.output_height),
    );

    // Both the shipped default (a chassis stands) and the bare tube, since the
    // key the two sides read is not the same key in the two cases.
    for chassis_shown in [true, false] {
        let mut cfg = Config::default();
        cfg.general.chassis_shown = chassis_shown;
        cfg.chassis.frame_size = 0.45;
        cfg.screen.frame_size = 0.1;

        let mut pacing = crt::Pacing::new(Instant::now());
        let time = pacing.tick_by(Duration::from_millis(16));
        let uniform = Params::build(&cfg, &geom, time, DegaussState::IDLE)
            .get("FrameSize")
            .expect("the FrameSize uniform");
        let pointer = app::settings::distortion_frame_size(&cfg) * normalized;

        assert!(
            (f64::from(uniform) - pointer).abs() < 1e-6,
            "with chassis_shown={chassis_shown} the shader bends by {uniform} \
             and the pointer un-bends by {pointer}"
        );
    }
}
