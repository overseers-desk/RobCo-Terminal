//! The one preset, and the structural settings that write it.
//!
//! This crate fixes the shape: one librashader preset, every variant axis a
//! runtime uniform. The exception is chain *structure*, which a uniform
//! cannot express: how many passes there are, what resolution each renders at,
//! and whether a framebuffer is float. Those come from
//! `app::settings::STRUCTURAL_KEYS`, they are what [`Structure`] carries, and a
//! change to any of them writes a new preset and rebuilds the `FilterChain`,
//! an accepted cost of ~10 ms and a reset burn-in ghost.
//!
//! The shaders are compiled into the binary and written out next to the preset
//! at load time. librashader parses presets from a path and resolves every
//! `shaderN =` and every texture relative to it, so there has to be a directory;
//! materialising one means the rebuild has no install-layout question to answer
//! and tests are hermetic.

use std::io;
use std::path::{Path, PathBuf};

use config::Config;

/// The six passes, in chain order. The index is the preset's `shaderN`.
///
/// This is the seam the rest of the crate builds against: the order, the
/// aliases, and which pass samples which are settled here, so a shader body
/// can be replaced without rewiring the graph.
///
/// Bloom is the one place a single shader body does not fit that shape: its
/// substance is a separable Gaussian, which is two passes, spending the one
/// structural graph edit this preset budgets for rather than folding it back
/// into one pass with a quadratic kernel. Only the second half carries the
/// `BloomSource` alias,
/// because that alias names what the static pass samples, and what it samples
/// is the finished two-axis blur.
///
/// A pass is described, not spelled out, so the preset is assembled by walking
/// this list. Inserting a pass is then an edit to the list rather than a rewrite
/// of the preset text, which is what the burn-in accumulator's mount contract
/// asks for: its accumulator block goes in mid-chain, and every `shaderN`
/// after it renumbers itself.
pub const PASSES: &[Pass] = &[
    Pass {
        alias: crt_burnin::ALIAS,
        file: "burn_in.slang",
        source: crt_burnin::BURN_IN_SLANG,
        owner: "burn-in",
        scale: Scale::BurnInQuality,
        float_framebuffer: true,
        // Not the accumulator's own preference, and the one line of this table
        // that is not about the pass it sits on. In librashader, pass 0's
        // filter is the *chain's* filter for `Original`
        // (`librashader-runtime-wgpu-0.12.0/src/filter_chain.rs:461-479`: the
        // input image is wrapped in an `InputImage` carrying
        // `passes[0].meta.filter`), and `Original` is the terminal grid, which
        // the static pass reads through the screen curvature and the bloom
        // reads to blur. Nearest there point-samples a pixel font through a
        // warp: 14% of the grid's lit pixels and 7% of its strokes do not
        // arrive (`tests/glyph_survival.rs`), where linear filtering resamples
        // the glyph edges instead of losing them outright.
        //
        // One texture, one answer for all of it: the terminal grid is
        // sampled once and both the burn-in effect and the static shader
        // read it under whatever filtering it carries. So does this chain.
        //
        // What `crt-burnin/MOUNT.md` warns about -- a linear filter reading the
        // accumulator at coordinates it was not written at -- does not arise at
        // this mount: the accumulator renders a full-screen quad into a
        // framebuffer the same size as the feedback it samples, so every fetch
        // lands on a texel centre. `tests/burn_in_chain.rs` measures the ramp
        // that would drift if it did not.
        filter_linear: true,
        note: "the burn-in accumulator, shader and all, from `crt-burnin`. \
               `alias0` is what creates the feedback framebuffer and names it \
               `BurnFeedback` inside the shader; `float_framebuffer0` is the \
               decay's accuracy (0.26% over a 12-frame fade against 1.32% at 8 \
               bits); `filter_linear0 = false` keeps the ghost from smearing \
               sideways a little more every frame.",
    },
    Pass {
        alias: "BloomH",
        file: "bloom_h.slang",
        source: include_str!("../shaders/bloom/bloom_h.slang"),
        owner: "bloom",
        scale: Scale::BloomQuality,
        float_framebuffer: false,
        filter_linear: true,
        note: "bloom, horizontal half, at `general.bloom_quality`. It samples \
               `Original` (the terminal image) rather than `Source`, which \
               at this point in the chain is the burn-in accumulator. The \
               alias is only here to be checked against the `<Alias>Size` \
               rule: nothing samples `BloomH` but the vertical half, which \
               reads it as its own `Source`.",
    },
    Pass {
        alias: "BloomSource",
        file: "bloom_v.slang",
        source: include_str!("../shaders/bloom/bloom_v.slang"),
        owner: "bloom",
        scale: Scale::BloomQuality,
        float_framebuffer: false,
        filter_linear: true,
        note: "bloom, vertical half, and the pass the alias belongs to: this is \
               what the static pass samples as `bloomSource`, rgb the blurred \
               image and alpha the mask it multiplies by. The alias is the \
               established sampler name, not the shorter `Bloom`: librashader \
               reserves `<Alias>Size` as a texture semantic, so a `Frame` \
               alias would collide with the `FrameSize` uniform and the \
               whole chain would fail to reflect. The pair is kept consistent.",
    },
    Pass {
        alias: "FrameSource",
        file: "terminal_frame.slang",
        source: include_str!("../shaders/terminal_frame/terminal_frame.slang"),
        owner: "bloom",
        scale: Scale::WindowScaling,
        float_framebuffer: false,
        filter_linear: true,
        note: "the screen's own moulding, which is drawn on the glass and so \
               belongs inside the chain, unlike the chassis around it. Fully \
               procedural: it samples nothing, and its parameters keep their \
               lower-case spelling rather than the chain's own naming \
               convention, so the shader body and the oracle test that \
               measures it stay byte-identical. This is the slot with two \
               occupants: see `crt::preset::CHASSIS_FRAME`.",
    },
    Pass {
        alias: "Static",
        file: "terminal_static.slang",
        source: include_str!("../shaders/terminal_static.slang"),
        owner: "glass",
        scale: Scale::WindowScaling,
        float_framebuffer: false,
        filter_linear: true,
        note: "the static terminal pass: curvature, RGB shift, bloom \
               composite, frame specular, brightness. Samples `Original`, not \
               `Source`.",
    },
    Pass {
        alias: "Screen",
        file: "terminal_dynamic.slang",
        source: include_str!("../shaders/terminal_dynamic.slang"),
        owner: "glass",
        scale: Scale::Viewport,
        float_framebuffer: false,
        filter_linear: true,
        note: "the dynamic terminal pass, and the chain's output: the finished \
               glass image. Chassis chrome composites outside the chain, \
               so nothing after this bends with the screen.",
    },
];

/// Pass 0's position in `PASSES`, for the one thing outside this module that
/// needs to name it: the burn-in host mount and this crate's own tests. It is
/// the only pass position anything outside `PASSES` still needs a name for,
/// so it stays a written-down constant rather than a lookup; a pass inserted
/// ahead of it would need this updated by hand along with `PASSES` itself.
pub const BURN_PASS: usize = 0;

/// The frame-source slot's position in [`PASSES`]: the one pass whose body is
/// not fixed.
pub const FRAME_PASS: usize = 3;

/// The frame-source slot's second occupant: the shell's bezel, for a window
/// that is standing inside a chassis.
///
/// The two occupants are drop-in alternatives for the same slot, picked
/// between in one expression rather than by any arrangement outside it.
/// Both bodies distort their own coordinates with the same
/// `distort_coordinates` the glass uses, which is why the bezel hugs a
/// curved tube instead of standing square around
/// it, and why the bezel, alone among the chassis, is inside the chain rather
/// than composited over it: the composite-outside-the-chain rule is about
/// the flat bank column left of everything; `chassis::frame`'s module doc
/// draws the same line.
///
/// The body is `chassis::shaders::FRAME_METAL_SLANG`. The dependency runs this
/// way and only this way: the chain has to be able to name both occupants of
/// its own slot, while the chassis crate depends on no part of the chain.
///
/// What is wired here is the *switch*. The uniforms are
/// [`crate::params`]', and it pushes both halves of them: the five the two
/// bodies share by name (`screenCurvature`, `frameSize`, `screenRadius`,
/// `ambientLight`, `frameShininess`), which land on either body, and, when
/// this body is the one standing, the rest of the shell's casting from
/// `chassis::frame::frame_params`, which is the other 31 and is what keeps the
/// bezel the shell's own metal rather than the shader's `#pragma` grey.
pub const CHASSIS_FRAME: FrameBody = FrameBody {
    file: "frame_metal.slang",
    source: chassis::shaders::FRAME_METAL_SLANG,
    note: "the shell's bezel, standing in for `terminal_frame` because a \
           chassis is shown (`general.chassis_shown`). It is in the chain, \
           not composited over it, because it distorts with the glass it \
           is set into.",
};

/// One of the frame-source slot's two bodies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameBody {
    pub file: &'static str,
    pub source: &'static str,
    pub note: &'static str,
}

/// Where a pass's framebuffer size comes from. Every variant but `Viewport` is
/// a structural setting, which is the whole reason a change to one rebuilds the
/// chain instead of writing a uniform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scale {
    WindowScaling,
    BloomQuality,
    BurnInQuality,
    /// The last pass renders straight to the viewport and takes no scale.
    Viewport,
}

/// The user texture the dynamic pass samples for noise, jitter, flicker and
/// the horizontal-sync tear. Tiled; the four channels are four independent
/// noise fields, which is why one fetch feeds four unrelated effects.
pub const NOISE_TEXTURE: &str = "allNoise512.png";
const NOISE_PNG: &[u8] = include_bytes!("../assets/allNoise512.png");

pub struct Pass {
    pub alias: &'static str,
    pub file: &'static str,
    pub source: &'static str,
    /// A short tag for the shader body, stamped into the generated preset's
    /// per-pass comment.
    pub owner: &'static str,
    pub scale: Scale,
    pub float_framebuffer: bool,
    /// How the *next* passes sample this one's output. True everywhere the
    /// picture is being resampled anyway, false on the accumulator, which reads
    /// its own previous frame and must read it where it wrote it.
    pub filter_linear: bool,
    /// What the pass is for, written into the preset above its block so the
    /// generated file explains itself to whoever opens it next.
    pub note: &'static str,
}

impl Pass {
    /// This pass's block of the preset, at chain position `index`.
    ///
    /// The body comes from the structure, not from `self`: the frame-source
    /// slot has two of them.
    fn block(&self, index: usize, structure: &Structure) -> String {
        let (file, _, note) = structure.body_at(index);
        let mut s = comment(&format!("Pass {index} ({}), {}", self.owner, note));
        s.push_str(&format!("shader{index} = \"{}\"\n", file));
        s.push_str(&format!("alias{index} = \"{}\"\n", self.alias));
        if let Some(scale) = structure.scale_of(self.scale) {
            s.push_str(&format!("scale_type{index} = \"original\"\n"));
            s.push_str(&format!("scale{index} = \"{scale:.6}\"\n"));
        }
        s.push_str(&format!(
            "filter_linear{index} = \"{}\"\n",
            self.filter_linear
        ));
        s.push_str(&format!("wrap_mode{index} = \"clamp_to_edge\"\n"));
        if self.float_framebuffer {
            s.push_str(&format!("float_framebuffer{index} = \"true\"\n"));
        }
        s
    }
}

/// Everything about the chain a uniform cannot carry.
///
/// Deriving this from a `Config` and comparing it is how the parameter/
/// structural split is enforced at the render layer: two configs that differ
/// only in parameter keys produce equal `Structure`s, so the chain is not
/// rebuilt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Structure {
    /// `general.window_scaling`. The resolution the picture is composed at.
    pub window_scaling: f32,
    /// `general.bloom_quality`. The bloom pass renders at this fraction.
    pub bloom_quality: f32,
    /// `general.burn_in_quality`. The accumulator renders at this fraction, and
    /// its resolution is why the burn-in *quality* is structural while the
    /// burn-in *strength* is a uniform: one is a framebuffer size, the other is
    /// a number the shader multiplies by.
    pub burn_in_quality: f32,
    /// `general.chassis_shown`. Which body sits in the frame-source slot, and
    /// therefore structural for a reason none of the others are: it changes
    /// which shader the preset names, not how big a framebuffer is. See
    /// [`CHASSIS_FRAME`].
    pub chassis_frame: bool,
}

impl Structure {
    pub fn from_config(cfg: &Config) -> Self {
        Self {
            window_scaling: cfg.general.window_scaling as f32,
            bloom_quality: cfg.general.bloom_quality as f32,
            burn_in_quality: cfg.general.burn_in_quality as f32,
            chassis_frame: cfg.general.chassis_shown,
        }
    }

    /// The body the frame-source slot carries under this structure, and the
    /// file it is written out as. Every other pass answers with its own.
    fn body_at(&self, index: usize) -> (&'static str, &'static str, &'static str) {
        let pass = &PASSES[index];
        if index == FRAME_PASS && self.chassis_frame {
            let b = &CHASSIS_FRAME;
            return (b.file, b.source, b.note);
        }
        (pass.file, pass.source, pass.note)
    }

    /// The framebuffer scale a pass's [`Scale`] resolves to, or `None` for the
    /// last pass, which renders to the viewport.
    ///
    /// Scales are floored at a small positive value: a zero scale is a
    /// zero-sized framebuffer, which is a device error rather than an invisible
    /// pass, and the config schema does not forbid one.
    fn scale_of(&self, scale: Scale) -> Option<f32> {
        match scale {
            Scale::WindowScaling => Some(self.window_scaling.max(0.01)),
            Scale::BloomQuality => Some(self.bloom_quality.max(0.01)),
            Scale::BurnInQuality => Some(self.burn_in_quality.max(0.01)),
            Scale::Viewport => None,
        }
    }

    /// The preset text, assembled by walking [`PASSES`].
    pub fn preset_text(&self) -> String {
        let mut s = format!(
            "\
# Generated by crt::preset. Do not edit: this file is rewritten whenever a
# structural setting changes, and hand edits are lost with it.
#
# One preset, {count} passes. Every variant axis that is not a framebuffer size
# is a runtime uniform; a parameter write costs 83 ns, a chain load
# ~10 ms. The pass list is `crt::preset::PASSES`.

textures = \"NoiseSource\"
NoiseSource = \"{NOISE_TEXTURE}\"
NoiseSource_linear = true
NoiseSource_wrap_mode = repeat
NoiseSource_mipmap = false

shaders = {count}
",
            count = PASSES.len(),
        );
        for (index, pass) in PASSES.iter().enumerate() {
            s.push('\n');
            s.push_str(&pass.block(index, self));
        }
        s
    }
}

/// Write the preset, its shaders and its noise texture into `dir`, and return
/// the path of the `.slangp`.
///
/// Shader bodies and the texture are written once; the preset itself is
/// rewritten on every call, because it is the part a structural change edits.
pub fn materialize(dir: &Path, structure: &Structure) -> io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    for index in 0..PASSES.len() {
        let (file, source, _) = structure.body_at(index);
        write_if_changed(&dir.join(file), source.as_bytes())?;
    }
    write_if_changed(&dir.join(NOISE_TEXTURE), NOISE_PNG)?;

    let preset = dir.join("robco.slangp");
    std::fs::write(&preset, structure.preset_text())?;
    Ok(preset)
}

/// Wrap prose into `#` comment lines. The generated preset is meant to be
/// opened and read, and a note running to three hundred columns is not.
fn comment(text: &str) -> String {
    const WIDTH: usize = 76;
    let mut out = String::new();
    let mut line = String::from("#");
    for word in text.split_whitespace() {
        if line.len() + 1 + word.len() > WIDTH && line.len() > 1 {
            out.push_str(&line);
            out.push('\n');
            line = String::from("#");
        }
        line.push(' ');
        line.push_str(word);
    }
    out.push_str(&line);
    out.push('\n');
    out
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Ok(existing) = std::fs::read(path) {
        if existing == bytes {
            return Ok(());
        }
    }
    std::fs::write(path, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pass_zero_is_the_block_the_mount_contract_asks_for() {
        // `MOUNT.md` says to call `crt_burnin::preset_pass_block` rather than
        // write the accumulator's block by hand. This preset cannot: every
        // block here is assembled by walking `PASSES`, and pass 0's framebuffer
        // is sized by `general.burn_in_quality`, which that function's own
        // block knows nothing about. So the block is written here and held to
        // that function's line for line instead, which is the same contract
        // with the scale left to the host.
        let text = Structure::from_config(&Config::default()).preset_text();
        for line in crt_burnin::preset_pass_block(BURN_PASS, PASSES[BURN_PASS].file).lines() {
            let (key, value) = line.split_once(" = ").expect("a `key = value` line");
            // The three the host owns. The standalone block renders the
            // accumulator at the size of whatever precedes it, and this chain
            // renders it at the structural quality setting; and `filter_linear`
            // on pass 0 is not the accumulator's setting at all in librashader,
            // it is the chain's filter for `Original`. See the note on the
            // pass, and `MOUNT.md`'s own paragraph on the position.
            if key == "scale_type0" || key == "scale0" || key == "filter_linear0" {
                continue;
            }
            assert!(
                text.contains(&format!("{key} = \"{value}\"")),
                "the generated preset is missing `{key} = {value}`, which \
                 crt_burnin::preset_pass_block puts on the accumulator:\n{text}"
            );
        }
    }

    /// The first pass's filter is the whole chain's filter for `Original`, and
    /// `Original` is the terminal grid. Nothing in the preset text says so --
    /// it is librashader's rule, not the format's -- so a pass inserted ahead
    /// of the accumulator, or its `filter_linear` flipped back on the
    /// assumption that it only concerns the ghost, would point-sample the pixel
    /// font through the curvature again and eat its stems. This is the line
    /// that would have to be edited on purpose to do that.
    #[test]
    fn the_first_pass_carries_the_chains_filter_for_the_terminal_grid() {
        assert!(
            PASSES[0].filter_linear,
            "pass 0 is where librashader reads the filter it binds `Original` \
             with (`filter_chain.rs`: `let filter = passes[0].meta.filter`), and \
             `Original` is the grid the static pass warps and the bloom blurs"
        );
        assert!(
            Structure::from_config(&Config::default())
                .preset_text()
                .contains("filter_linear0 = \"true\""),
            "the generated preset does not carry it"
        );
    }

    #[test]
    fn the_accumulator_is_not_the_last_pass() {
        // The feedback contract: later passes sample `Burn`, and a feedback
        // pass at the end of a chain has nothing to hand its ghost to.
        assert!(BURN_PASS < PASSES.len() - 1);
        assert_eq!(PASSES[BURN_PASS].alias, crt_burnin::ALIAS);
    }
}
