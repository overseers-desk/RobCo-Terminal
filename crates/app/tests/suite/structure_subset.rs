//! `crt::preset::Structure` decides for itself, from a `Config`, what the
//! filter chain's shape is; `config::structural::STRUCTURAL` names the keys
//! whose change forces a rebuild. The relation that has to hold: every
//! config field `Structure::from_config` reads is a structural key, or a
//! change the render layer treats as structural would classify as a live
//! parameter push. The test below holds it by perturbation rather than by
//! a second field list: it moves every serialized leaf of the config and
//! checks that whenever `Structure` moves, the key is structural.

#[test]
fn structure_reads_only_structural_keys() {
    let base = config::Config::default();
    let base_structure = crt::preset::Structure::from_config(&base);
    // Word-enum fields need a valid variant differing from the default;
    // everything else perturbs generically by type.
    let alternates: &[(&str, &str)] = &[
        ("shell", "switchboard"),
        ("channel_indicator", "switch"),
        ("channel_display", "tape"),
        ("rasterization", "scanline_rasterization"),
        ("font_source", "system_fonts"),
        ("selection_model", "rio"),
        ("timing", "random"),
    ];
    let document = serde_json::to_value(&base).expect("config serializes");
    for (section, object) in document.as_object().expect("config is an object") {
        for (key, leaf) in object.as_object().expect("section is an object") {
            let mut moved_doc = document.clone();
            let perturbed = match leaf {
                serde_json::Value::Number(n) if n.is_f64() => {
                    serde_json::Value::from(n.as_f64().unwrap() + 0.5)
                }
                serde_json::Value::Number(n) => serde_json::Value::from(n.as_i64().unwrap() + 1),
                serde_json::Value::Bool(b) => serde_json::Value::from(!b),
                serde_json::Value::String(s) => match alternates.iter().find(|(k, _)| k == key) {
                    Some((_, alternate)) => serde_json::Value::from(*alternate),
                    None => serde_json::Value::from(format!("{s}x")),
                },
                // A list-shaped leaf (`[[ssh.host]]`) has no generic
                // perturbation, and the filter chain reads no list; the
                // day `Structure` reads one, its scalar neighbours still
                // hold the relation this test pins.
                serde_json::Value::Array(_) => continue,
                other => panic!("unexpected leaf shape at {section}.{key}: {other:?}"),
            };
            moved_doc[section][key] = perturbed;
            let moved: config::Config =
                serde_json::from_value(moved_doc).unwrap_or_else(|e| panic!("{section}.{key}: {e}"));
            if crt::preset::Structure::from_config(&moved) != base_structure {
                let dotted = format!("{section}.{key}");
                assert!(
                    config::structural::STRUCTURAL.contains(&dotted.as_str()),
                    "`Structure::from_config` reads `{dotted}`, but it is not a structural key"
                );
            }
        }
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
        let pointer = app::settings::unscaled_frame_size(&cfg) * normalized;

        assert!(
            (f64::from(uniform) - pointer).abs() < 1e-6,
            "with chassis_shown={chassis_shown} the shader bends by {uniform} \
             and the pointer un-bends by {pointer}"
        );
    }
}
