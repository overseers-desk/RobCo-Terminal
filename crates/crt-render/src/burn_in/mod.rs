//! Burn-in: the feedback pass that ages a lit pixel's ghost after it goes dark.
//!
//! The pass is a feedback loop: an alias gives it a framebuffer holding its own
//! previous frame, which it samples against the live source every frame. The
//! port is deliberately literal, down to the alpha channel carrying the
//! freshness mask rather than an opacity.
//!
//! The module is three things:
//!
//!   * `shaders/burn_in/burn_in.slang`, the pass itself, reached as
//!     [`BURN_IN_SLANG`], plus a standalone preset that mounts it for
//!     measurement.
//!   * [`decay`], the wall-clock arithmetic that produces the one uniform the
//!     pass takes from the CPU. No GPU state, unit-testable on its own.
//!   * [`BurnInPass`], the mount: what a host graph has to put in its preset
//!     and what it has to call every frame.
//!
//! What it does not do is composite the ghost into the final image.
//! `terminal_dynamic.slang` does that, and it belongs with that pass; the lines
//! it needs are quoted below so nobody re-derives them.
//!
//! # Mounting the pass
//!
//! Put the accumulator wherever the ghost belongs in the chain: after the pass
//! that draws the terminal grid, before the CRT passes that bend and colour it.
//! It reads `Source`, not `Original`, so it takes whatever pass precedes it,
//! and it must not be the last pass. [`preset_pass_block`] writes the block
//! rather than a host writing it by hand:
//!
//! ```text
//! shader{N} = burn_in.slang
//! alias{N} = Burn
//! scale_type{N} = source
//! scale{N} = 1.0
//! filter_linear{N} = false
//! wrap_mode{N} = clamp_to_edge
//! float_framebuffer{N} = true
//! ```
//!
//! Every line earns its place:
//!
//! - `alias{N} = Burn` is what gives the pass a feedback framebuffer at all,
//!   and `BurnFeedback` (alias plus `Feedback`) is the sampler name the shader
//!   declares. Omit it and the pass compiles, runs, and has no previous frame.
//! - `float_framebuffer` is the accuracy. Over a 14-frame fade at the default
//!   `burn_in = 0.25`: 0.3418% worst error against the ramp the set rate
//!   predicts, against 1.3235% for the same chain at 8 bits. It is fp16, not
//!   fp32: librashader's `get_format_override` maps it to `R16G16B16A16Sfloat`
//!   and that override beats any `#pragma format` in the shader. The ULP near
//!   1.0 is 1/2048, about 1.5% of a 60 Hz decay step, one-directional, which is
//!   the whole of that drift.
//! - `filter_linear = false` and `wrap_mode = clamp_to_edge`: the accumulator
//!   is read at exactly the coordinates it was written at, and a linear filter
//!   would smear the ghost sideways a little more every frame.
//!
//!   **Unless the accumulator is pass 0.** librashader binds the chain's
//!   `Original`, every pass's view of the picture the chain was handed, with
//!   the filter and wrap mode of *pass 0*, whichever pass that is
//!   (`librashader-runtime-wgpu/src/filter_chain.rs`, `let filter =
//!   passes[0].meta.filter`). Put the accumulator first and this line stops
//!   being about the ghost: it decides how the whole chain samples its input.
//!   [`crate::preset`] mounts it first and sets `filter_linear0 = true` for
//!   that reason, and `the_first_pass_carries_the_chains_filter_for_the_terminal_grid`
//!   says so; the ghost is unharmed because a pass whose framebuffer is the
//!   size of its own feedback samples that feedback at texel centres, where
//!   linear and nearest agree.
//!
//! [`crate::preset`] is the one host, and it writes the block itself: it
//! assembles the whole preset by walking its pass list, and pass 0's
//! framebuffer is sized by `general.burn_in_quality`, a structural setting this
//! block knows nothing about. So it takes `scale_type`/`scale` from that
//! setting and every other line from here, and its
//! `pass_zero_is_the_block_the_mount_contract_asks_for` test holds the
//! generated text to [`preset_pass_block`] line by line with those two
//! exempted.
//!
//! # Per frame
//!
//! The host owns one [`BurnInPass`] per mounted accumulator and does exactly
//! this, in this order:
//!
//! ```text
//! burn_in.push(chain.parameters(), now_seconds); // before frame(), every frame
//! chain.frame(&input, &viewport, &mut cmd, frame_index, None)?;
//! ```
//!
//! `now_seconds` is any monotonic clock, as long as it is the same one every
//! frame; the pass only ever looks at differences. [`BurnInPass::push`] writes
//! two parameters and returns the decay it wrote. Do it unconditionally: a
//! parameter write is 83 ns, so skipping it on unchanged frames saves nothing
//! and risks a frame running on a stale delta. Nothing else about the pass is
//! the host's business: no textures to bind, no per-frame uniform block, no
//! state outside the chain's own feedback framebuffer.
//!
//! | Host event | Call | Why |
//! |---|---|---|
//! | `screen.burn_in` changed in the config | [`BurnInPass::set_burn_in`] | Parameter-level: no rebuild, and the ghost on screen keeps fading from where it is, at the new rate. |
//! | Chain rebuilt on a structural key, window resized, font changed | [`BurnInPass::restart`] | The accumulator's contents are discontinuous; the next frame must decay by nothing. |
//! | Frame skipped, window occluded, process resumed | nothing | The clock clamps any delta above 0.25 s to 0.25 s, so a resumed process fades by one frame's worth rather than erasing the ghost. |
//!
//! `burn_in = 0` needs no special handling: the clock pushes a decay of 1.0 and
//! clears the mask, the accumulator collapses to the live image, and the ghost
//! is gone from the next frame.
//!
//! # Compositing the ghost
//!
//! The accumulator only holds the ghost. The picture is mixed with it in
//! `terminal_dynamic.slang`, and those lines are burn-in semantics rather than
//! dynamic-pass semantics, so they are quoted here:
//!
//! ```glsl
//! vec4 txt_blur = texture(burnInSource, staticCoords);
//! float blurDecay = clamp((time - burnInLastUpdate) * burnInTime, 0.0, 1.0);
//! vec3 burnInColor = 0.65 * (txt_blur.rgb - vec3(blurDecay)) * (1.0 - txt_blur.a);
//! txt_color = max(txt_color, burnInColor);
//! ```
//!
//! `(1.0 - txt_blur.a)` is the freshness mask on the consuming side: where the
//! accumulator says the pixel is currently live, the ghost contributes nothing,
//! because the live text is already drawing it. The `blurDecay` term
//! extrapolates decay for the time since the accumulator was last updated,
//! which matters only when the accumulator does not run every frame; in this
//! graph it runs on every rendered frame, so that interval is zero and the term
//! drops out. A damage-gated accumulator would bring it back, and
//! [`BurnInPass`] is where the two timestamps would live. The `0.65` is a
//! brightness trim on the ghost and belongs with the composite.
//!
//! # What the mount is tested against
//!
//! [`crate::chain::Chain`] is the mount that ships, and it holds all three of
//! the calls above so that nothing above this crate can drop one.
//! `tests/suite/burn_in_chain.rs` measures the ghost through the whole
//! five-pass graph, composite included. `tests/suite/mount.rs` builds a
//! three-pass preset from [`preset_pass_block`] with the accumulator at index
//! 1, a copy pass either side, and checks the ghost, the mask and the rate
//! survive being mounted in the middle of a chain rather than at its head. If
//! the graph changes shape, that test is the one to re-run first.

pub mod decay;

pub use decay::{DecayClock, DECAY_PARAM, MASK_PARAM};

/// The accumulator's shader source, so a host graph can write it next to a
/// preset it generates at runtime instead of shipping a second copy.
pub const BURN_IN_SLANG: &str = include_str!("../../shaders/burn_in/burn_in.slang");

/// The pass alias. It is not cosmetic: the alias is what gives the pass a
/// feedback framebuffer, and `<alias>Feedback` is the sampler name the shader
/// declares. Changing it here means changing it in the shader.
pub const ALIAS: &str = "Burn";

/// The preset lines for the accumulator at pass index `index`.
///
/// Every line matters and the reasons are in this module's docs. A host that
/// writes its own block instead of calling this owns the consequences; the most
/// expensive omission is the framebuffer format, which does not fail, it just
/// makes the decay wrong by an order of magnitude (1.83% error at 8 bits
/// against 0.24% with a float target).
pub fn preset_pass_block(index: usize, shader_path: &str) -> String {
    format!(
        "shader{index} = {shader_path}\n\
         alias{index} = {ALIAS}\n\
         scale_type{index} = source\n\
         scale{index} = 1.0\n\
         filter_linear{index} = false\n\
         wrap_mode{index} = clamp_to_edge\n\
         float_framebuffer{index} = true\n"
    )
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
    fn the_shipped_preset_matches_the_generated_block() {
        let preset = include_str!("../../shaders/burn_in/burn_in.slangp");
        for line in preset_pass_block(0, "burn_in.slang").lines() {
            assert!(preset.contains(line), "shipped preset lacks `{line}`");
        }
    }
}
