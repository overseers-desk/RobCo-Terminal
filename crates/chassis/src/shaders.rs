//! The chassis's shader source, compiled in.
//!
//! Two families, and the split is the chain's edge. The bezel is the one
//! piece of chassis inside the curvature, so it is a `.slang` mounted in the
//! chain's frame slot, and it ships with a `.slangp` beside it under
//! `shaders/metal/` for the per-pass test in `robco-crt-render`'s suite to
//! mount. Everything drawn *after* the chain is WGSL under `shaders/wgsl/`,
//! run by a native pipeline that needs no preset, no cache directory and no
//! runtime materialisation.
//!
//! The WGSL family is function bodies: no bindings, no entry points, one
//! function per thing that can be drawn. A host concatenates them with its
//! own glue, which is what lets the same text serve a per-piece instanced
//! pass and a single-quad measurement rig.
//!
//! A shipped binary carries its shaders rather than looking for a source
//! tree, which is what these constants are for.

/// `frame_metal.slang`: the bezel the screen well is set into.
///
/// This one has a chain seat: the shell's frame and the bare screen frame
/// pick between them for the same frame-source slot, so a host drawing a
/// chassis mounts this where `terminal_frame` would otherwise go, with the
/// uniforms [`crate::frame::frame_params`] builds. See [`crate::frame`]'s
/// module doc for why the bezel is the one piece of chassis that belongs
/// inside the curvature rather than composited over it.
pub const FRAME_METAL_SLANG: &str = include_str!("../shaders/metal/frame_metal.slang");

/// `wgsl/common.wgsl`: the metal-surface math shared by everything drawn
/// after the CRT chain, the twin of `metal_common.slang`. It declares no
/// bindings and no entry points, so a host concatenates it ahead of a shader
/// body and its own glue.
pub const COMMON_WGSL: &str = include_str!("../shaders/wgsl/common.wgsl");

/// `wgsl/chassis_metal.wgsl`: the casting under the bank column, as one
/// function over a `ChassisParams` value. The parameter block is
/// [`crate::params::ChassisMetalParams::record`]'s layout.
pub const CHASSIS_METAL_WGSL: &str = include_str!("../shaders/wgsl/chassis_metal.wgsl");

/// `wgsl/plate_metal.wgsl`: the raised plate a shell screws over the casting.
/// Parameter block from [`crate::params::PlateMetalParams::record`]; which region a shell
/// screws it over is [`crate::shells::plate_region`].
pub const PLATE_METAL_WGSL: &str = include_str!("../shaders/wgsl/plate_metal.wgsl");

/// `wgsl/led_matrix.wgsl`: the lamp grid one channel strip is made of, over
/// the grid raster [`crate::furniture::led_grid`] composes. Parameter block
/// from [`crate::params::LedMetalParams::record`].
pub const LED_MATRIX_WGSL: &str = include_str!("../shaders/wgsl/led_matrix.wgsl");

/// `wgsl/vector.wgsl`: the furniture that is drawn rather than shaded, as
/// one function per [`crate::paint::Op`] kind over a `VectorParams` value.
/// The parameter block is `app::chrome`'s vector record; the host also
/// declares the gradient-stop and polygon-point arrays these read their runs
/// out of.
pub const VECTOR_WGSL: &str = include_str!("../shaders/wgsl/vector.wgsl");

/// `wgsl/tape_label.wgsl`: the embossed punch tape the switchboard's strips
/// carry instead of lamps. Parameter block from
/// [`crate::params::TapeMetalParams::record`].
pub const TAPE_LABEL_WGSL: &str = include_str!("../shaders/wgsl/tape_label.wgsl");

/// Include files the shader sources above pull in with `#include`. A host
/// materializing shaders to disk writes these into the same directory as
/// the including shaders, under exactly these file names; librashader
/// resolves includes relative to the including file.
pub const INCLUDES: &[(&str, &str)] = &[(
    "metal_common.slang",
    include_str!("../shaders/metal/metal_common.slang"),
)];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bezel_is_compiled_in_whole_and_is_the_one_it_claims() {
        // A `.slang` pass declares both stages; a truncated include would
        // still be a valid `&str` and would fail only at chain-load time.
        assert!(FRAME_METAL_SLANG.contains("#pragma stage vertex"));
        assert!(FRAME_METAL_SLANG.contains("#pragma stage fragment"));
        // ...and a uniform only the bezel declares, so a mis-pointed
        // `include_str!` is caught rather than silently swapped.
        assert!(FRAME_METAL_SLANG.contains("frameSize"));
        // The metalField it shares with the WGSL half lives in the include it
        // pulls in.
        let (name, common) = INCLUDES[0];
        assert_eq!(name, "metal_common.slang");
        assert!(common.contains("float metalField"));
        assert!(FRAME_METAL_SLANG.contains("#include \"metal_common.slang\""));
        // The one thing the chain still needs from this directory: an include
        // it can resolve beside the body it writes out.
        assert_eq!(INCLUDES.len(), 1);
    }

    /// The WGSL half carries the same surface math under its own spelling,
    /// and carries no bindings: a body that declared a `@group` would fix the
    /// binding numbers of every host that concatenates it.
    #[test]
    fn the_wgsl_bodies_are_functions_a_host_binds_rather_than_passes() {
        assert!(COMMON_WGSL.contains("fn metal_field("));
        assert!(COMMON_WGSL.contains("fn rrect_px("));
        assert!(CHASSIS_METAL_WGSL.contains("fn chassis_metal("));
        assert!(CHASSIS_METAL_WGSL.contains("viewport_size"));
        assert!(PLATE_METAL_WGSL.contains("fn plate_metal("));
        assert!(LED_MATRIX_WGSL.contains("fn led_matrix("));
        assert!(TAPE_LABEL_WGSL.contains("fn tape_label("));
        for name in [
            "fn vector_rect(",
            "fn vector_arc(",
            "fn vector_polygon(",
            "fn vector_text(",
        ] {
            assert!(VECTOR_WGSL.contains(name), "vector.wgsl is missing {name}");
        }
        // The two display bodies read their raster through the host's one
        // supplied function rather than a texture of their own.
        for src in [LED_MATRIX_WGSL, TAPE_LABEL_WGSL] {
            assert!(src.contains("chrome_sample("));
            assert!(!src.contains("texture_2d"));
        }
        for src in [
            COMMON_WGSL,
            CHASSIS_METAL_WGSL,
            PLATE_METAL_WGSL,
            LED_MATRIX_WGSL,
            TAPE_LABEL_WGSL,
            VECTOR_WGSL,
        ] {
            assert!(!src.contains("@group"));
            assert!(!src.contains("@vertex"));
            assert!(!src.contains("@fragment"));
        }
    }
}
