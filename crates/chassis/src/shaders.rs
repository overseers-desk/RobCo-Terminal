//! The three metals' source, compiled in.
//!
//! Each also ships a `.slangp` beside it under `shaders/metal/`, which is how
//! the per-pass tests mount them: a preset references its `.slang` by a path
//! relative to its own directory, so a preset needs a directory to be resolved
//! against and the tests give it `CARGO_MANIFEST_DIR`. That works from a source
//! checkout and nowhere else.
//!
//! These constants are the other half, for a host that writes its own preset
//! and has no source tree to read from. It is the same shape `crt::burn_in`
//! gives the burn-in pass, and for the same reason: a shipped binary carries
//! its shaders rather than looking for them.

/// `frame_metal.slang`: the bezel the screen well is set into.
///
/// This one has a chain seat: the shell's frame and the bare screen frame
/// pick between them for the same frame-source slot, so a host drawing a
/// chassis mounts this where `terminal_frame` would otherwise go, with the
/// uniforms [`crate::frame::frame_params`] builds. See [`crate::frame`]'s
/// module doc for why the bezel is the one piece of chassis that belongs
/// inside the curvature rather than composited over it.
pub const FRAME_METAL_SLANG: &str = include_str!("../shaders/metal/frame_metal.slang");

/// `chassis_metal.slang`: the casting under the bank column, drawn in the
/// frame's coordinates so the two read as one poured piece. Uniforms from
/// [`crate::frame::chassis_params`]. Composited over the presented frame, flat
/// and square and left of everything: chrome that sits outside the CRT chain
/// rather than bending with the tube.
pub const CHASSIS_METAL_SLANG: &str = include_str!("../shaders/metal/chassis_metal.slang");

/// `plate_metal.slang`: the raised plate a shell screws over the casting, the
/// bank's furniture punched into it. Uniforms from
/// [`crate::furniture::plate_params`]; which region a shell screws it over is
/// [`crate::shells::plate_region`].
pub const PLATE_METAL_SLANG: &str = include_str!("../shaders/metal/plate_metal.slang");

/// `led_matrix.slang`: the lamp grid one channel strip is made of. Uniforms
/// from [`crate::furniture::led_params`], over the grid raster
/// [`crate::furniture::led_grid`] composes.
pub const LED_MATRIX_SLANG: &str = include_str!("../shaders/led_matrix/led_matrix.slang");

/// `tape_label.slang`: the embossed punch tape the switchboard's strips carry
/// instead of lamps. Uniforms from [`crate::furniture::tape_params`].
pub const TAPE_LABEL_SLANG: &str = include_str!("../shaders/tape_label/tape_label.slang");

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
    fn each_metal_is_compiled_in_whole_and_is_the_one_it_claims() {
        for (name, src, entry) in [
            ("frame", FRAME_METAL_SLANG, "frameSize"),
            ("chassis", CHASSIS_METAL_SLANG, "fieldScale"),
            ("plate", PLATE_METAL_SLANG, "bevelPx"),
        ] {
            // A `.slang` pass declares both stages; a truncated include would
            // still be a valid `&str` and would fail only at chain-load time.
            assert!(
                src.contains("#pragma stage vertex"),
                "{name} metal has no vertex stage"
            );
            assert!(
                src.contains("#pragma stage fragment"),
                "{name} metal has no fragment stage"
            );
            // ...and a uniform only this one of the three declares, so a
            // mis-pointed include_str! is caught rather than silently swapped.
            assert!(src.contains(entry), "{name} metal does not declare {entry}");
        }
        // The metalField the three share lives in the include they pull in.
        let (name, common) = INCLUDES[0];
        assert_eq!(name, "metal_common.slang");
        assert!(common.contains("float metalField"));
        for src in [FRAME_METAL_SLANG, CHASSIS_METAL_SLANG, PLATE_METAL_SLANG] {
            assert!(src.contains("#include \"metal_common.slang\""));
        }
    }
}
