//! Burn-in as a librashader feedback pass.
//!
//! The pass is a feedback loop: an alias gives it a framebuffer holding its
//! own previous frame, which it samples against the live source every
//! frame. The port is deliberately literal, down to the alpha channel
//! carrying the freshness mask rather than an opacity.
//!
//! The crate is three things:
//!
//!   * `shaders/burn_in.slang`, the pass itself, plus a standalone preset that
//!     mounts it for measurement.
//!   * [`decay`], the wall-clock arithmetic that produces the one uniform the
//!     pass takes from the CPU. No GPU state, unit-testable on its own.
//!   * [`BurnInPass`], the mount: what a host graph has to put in its preset and
//!     what it has to call every frame. `MOUNT.md` states that contract in
//!     prose; this type is the executable half of it.
//!
//! What the crate does not do is composite the ghost into the final image.
//! `terminal_dynamic.slang` does that, and it belongs with that pass; the
//! two lines it needs are quoted in `MOUNT.md` so nobody re-derives them.

pub mod decay;

#[cfg(feature = "wgpu-runtime")]
pub mod chain;
#[cfg(feature = "wgpu-runtime")]
pub mod headless;

use std::io;
use std::path::Path;

pub use decay::{DecayClock, DECAY_PARAM, MASK_PARAM};

/// The accumulator's shader source, so a host graph can write it next to a
/// preset it generates at runtime instead of shipping a second copy.
pub const BURN_IN_SLANG: &str = include_str!("../shaders/burn_in.slang");

/// The pass alias. It is not cosmetic: the alias is what gives the pass a
/// feedback framebuffer, and `<alias>Feedback` is the sampler name the shader
/// declares. Changing it here means changing it in the shader.
pub const ALIAS: &str = "Burn";

/// What the accumulator stores its ghost in.
///
/// The decay is a subtraction of about 0.032 per frame at 60 Hz, so the
/// framebuffer's resolution is the decay's accuracy, and the choice is measured
/// rather than assumed (`tests/burn_in.rs`, `tests/mount.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Precision {
    /// `float_framebuffer = true`, which librashader maps to `Rgba16Float`.
    /// Available everywhere and accurate to 0.26% over a 12-frame fade. The
    /// default, and the measured floor.
    #[default]
    Fp16,
    /// `Rgba32Float`, requested by a `#pragma format` in the shader with
    /// `float_framebuffer` left off, since the preset key would override the
    /// pragma back to fp16.
    ///
    /// Tracks the set rate exactly (0.0000% over the same run), but it needs the
    /// device to have `wgpu::Features::FLOAT32_FILTERABLE`: librashader's bind
    /// group layout asks for a filterable float sample type for every sampled
    /// texture, and without the feature an `Rgba32Float` framebuffer is a wgpu
    /// validation error, not a slower path. Ask [`Precision::for_device`].
    Fp32,
}

impl Precision {
    /// The best precision this device can actually run.
    #[cfg(feature = "wgpu-runtime")]
    pub fn for_device(device: &wgpu::Device) -> Self {
        if device
            .features()
            .contains(wgpu::Features::FLOAT32_FILTERABLE)
        {
            Self::Fp32
        } else {
            Self::Fp16
        }
    }

    /// The accumulator's shader source at this precision.
    pub fn shader_source(self) -> std::borrow::Cow<'static, str> {
        match self {
            Self::Fp16 => std::borrow::Cow::Borrowed(BURN_IN_SLANG),
            Self::Fp32 => std::borrow::Cow::Owned(BURN_IN_SLANG.replacen(
                "#version 450",
                "#version 450\n#pragma format R32G32B32A32_SFLOAT",
                1,
            )),
        }
    }
}

/// Write the accumulator shader into `dir` at the default precision.
pub fn write_shader(dir: &Path) -> io::Result<std::path::PathBuf> {
    write_shader_with(dir, Precision::default())
}

/// Write the accumulator shader into `dir`, returning the file's path.
///
/// For hosts that assemble their preset in a temporary directory. A host that
/// ships a static preset can copy `shaders/burn_in.slang` at build time instead
/// and never call this, but then it is pinned to fp16, because the fp32 variant
/// differs by a `#pragma` this function injects.
pub fn write_shader_with(dir: &Path, precision: Precision) -> io::Result<std::path::PathBuf> {
    let path = dir.join("burn_in.slang");
    std::fs::write(&path, precision.shader_source().as_ref())?;
    Ok(path)
}

/// The preset lines for the accumulator at pass index `index`, at the default
/// precision.
pub fn preset_pass_block(index: usize, shader_path: &str) -> String {
    preset_pass_block_with(index, shader_path, Precision::default())
}

/// The preset lines for the accumulator at pass index `index`.
///
/// Every line matters and the reasons are on each one. A host that writes its
/// own block instead of calling this owns the consequences; the most expensive
/// omission is the framebuffer format, which does not fail, it just makes the
/// decay wrong by an order of magnitude (1.83% error at 8 bits against
/// 0.24% with a float target).
pub fn preset_pass_block_with(index: usize, shader_path: &str, precision: Precision) -> String {
    let mut block = format!(
        "shader{index} = {shader_path}\n\
         alias{index} = {ALIAS}\n\
         scale_type{index} = source\n\
         scale{index} = 1.0\n\
         filter_linear{index} = false\n\
         wrap_mode{index} = clamp_to_edge\n"
    );
    if precision == Precision::Fp16 {
        // Deliberately absent at fp32: the key is an override that would beat
        // the shader's own `#pragma format` and put the pass back at fp16.
        block.push_str(&format!("float_framebuffer{index} = true\n"));
    }
    block
}

/// The mounted pass: a decay clock plus the parameter writes that carry its
/// output to the shader.
///
/// A host graph owns one of these per mounted accumulator, ticks it once per
/// rendered frame *before* `FilterChain::frame`, and otherwise leaves the pass
/// alone.
#[derive(Debug, Clone)]
pub struct BurnInPass {
    clock: DecayClock,
    mask: bool,
    last_pushed: Option<f32>,
}

impl BurnInPass {
    /// Mount for the given `screen.burn_in` setting.
    pub fn new(burn_in: f64) -> Self {
        Self {
            clock: DecayClock::new(burn_in),
            mask: true,
            last_pushed: None,
        }
    }

    pub fn clock(&mut self) -> &mut DecayClock {
        &mut self.clock
    }

    /// Live settings reload. Parameter-level: no chain rebuild, no ghost reset.
    pub fn set_burn_in(&mut self, burn_in: f64) {
        self.clock.set_burn_in(burn_in);
    }

    /// Freshness mask on or off. The application never turns it off; the switch
    /// exists so a test can measure what the mask is doing on one chain.
    pub fn set_mask(&mut self, on: bool) {
        self.mask = on;
    }

    /// Discontinue the accumulator: the next frame decays by nothing.
    ///
    /// Call after a chain rebuild, a resize, or a font change: anything
    /// that discontinues the accumulator.
    pub fn restart(&mut self) {
        self.clock.restart();
        self.last_pushed = None;
    }

    /// The decay pushed on the last frame, for logging and tests.
    pub fn last_decay(&self) -> Option<f32> {
        self.last_pushed
    }

    /// Compute this frame's decay from `now` (monotonic seconds) and write both
    /// parameters into the chain. Returns the decay written.
    ///
    /// Unconditional every frame: the write measures at 83 ns, so gating it
    /// on a change costs more than it saves and risks the chain running a frame
    /// on a stale delta.
    #[cfg(feature = "wgpu-runtime")]
    pub fn push(&mut self, params: &librashader::runtime::RuntimeParameters, now: f64) -> f32 {
        let step = self.clock.tick(now);
        params.set_parameter_value(DECAY_PARAM, step);
        // The mask goes off with burn-in itself. The shader computes
        // `max(0, step - prevMask)`, so a pixel lit on the previous frame
        // would survive one frame of even a full decay, and switching
        // burn-in off would leave a frame of ghost behind if this switch
        // did not say so explicitly.
        let mask = self.mask && self.clock.burn_in() > 0.0;
        params.set_parameter_value(MASK_PARAM, if mask { 1.0 } else { 0.0 });
        self.last_pushed = Some(step);
        step
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_block_carries_the_whole_contract() {
        let b = preset_pass_block(2, "burn_in.slang");
        for line in [
            "shader2 = burn_in.slang",
            "alias2 = Burn",
            "float_framebuffer2 = true",
            "filter_linear2 = false",
            "wrap_mode2 = clamp_to_edge",
        ] {
            assert!(b.contains(line), "missing {line} in\n{b}");
        }
    }

    #[test]
    fn the_shipped_shader_declares_the_alias_sampler() {
        // If the alias and the sampler name ever drift apart the pass compiles
        // and silently reads nothing, so pin them to each other here.
        assert!(BURN_IN_SLANG.contains(&format!("sampler2D {ALIAS}Feedback")));
        assert!(BURN_IN_SLANG.contains(DECAY_PARAM));
        assert!(BURN_IN_SLANG.contains(MASK_PARAM));
    }

    #[test]
    fn fp32_asks_by_pragma_and_fp16_by_preset_key() {
        // The two ways of naming a format are mutually exclusive: the preset key
        // wins, so the fp32 block must not carry it and the fp32 shader must.
        let fp32 = preset_pass_block_with(0, "burn_in.slang", Precision::Fp32);
        assert!(!fp32.contains("float_framebuffer"));
        assert!(Precision::Fp32
            .shader_source()
            .contains("#pragma format R32G32B32A32_SFLOAT"));
        assert!(preset_pass_block_with(0, "burn_in.slang", Precision::Fp16)
            .contains("float_framebuffer0 = true"));
        assert!(!Precision::Fp16.shader_source().contains("#pragma format"));
    }

    #[test]
    fn the_shipped_preset_matches_the_generated_block() {
        let preset = include_str!("../shaders/burn_in.slangp");
        for line in preset_pass_block(0, "burn_in.slang").lines() {
            assert!(preset.contains(line), "shipped preset lacks `{line}`");
        }
    }
}
