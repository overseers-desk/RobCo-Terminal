//! The CRT pass graph: the chain the terminal picture is drawn through.
//!
//! # Shape
//!
//! One librashader preset, six passes, loaded once. Every shader variant that
//! would otherwise be a separately compiled permutation is a runtime uniform
//! here instead: 83 ns to write a parameter, ~10 ms to load a chain, against
//! 56 variants a compile-time approach would generate.
//!
//! ```text
//!   gpu::Target  (the grid, drawn offscreen; carries TEXTURE_BINDING)
//!          |
//!          |  Original
//!          v
//!   0  Burn         <- Source(=Original), BurnFeedback   float fb   [burn-in]
//!   1  BloomH       <- Original                                     [bloom/frame]
//!   2  BloomSource  <- Source(=BloomH)                              [bloom/frame]
//!   3  FrameSource  <- (procedural; two bodies, see below)          [bloom/frame]
//!   4  Static       <- Original, BloomSource                        [core]
//!   5  Screen       <- Source(=Static), Burn, FrameSource, NoiseSource  [core]
//!          |
//!          v
//!   the finished glass image
//! ```
//!
//! The aliases are fixed sampler names, kept stable rather than renamed
//! freely: librashader reserves `<Alias>Size` as a texture semantic, so an
//! alias of `Frame` collides with the `FrameSize` uniform and the reflector
//! rejects the chain outright (`InvalidTypeForSemantic("FrameSize")`). Any
//! later pass alias has to be checked against the uniform names for the same
//! clash.
//!
//! The chain stops at the glass. Chassis chrome composites over this output
//! rather than running through it: bending the cabinet along with
//! the picture is the defect that ruled the Rio fork out, and keeping the two
//! layers separate is the reason this crate exists at all.
//!
//! The bezel is the one exception to that separation. Pass 3 carries the
//! screen's own moulding for a bare tube and the shell's bezel for a window
//! standing inside a chassis, picking between the two for that slot in a
//! single expression; both bodies distort their own coordinates the way the
//! glass does, which is how a bezel hugs a curved tube. See
//! [`preset::CHASSIS_FRAME`]. The composite-outside-the-chain rule is about
//! the flat bank column left of everything, which stays outside.
//!
//! # Modules
//!
//! - [`preset`] the pass list, the structural settings that write it, and the
//!   shader sources compiled into the binary.
//! - [`chain`] the `FilterChain`, and the parameter-versus-rebuild decision.
//! - [`params`] every uniform, and the arithmetic between a setting and it.
//! - [`pacing`] the wall clock the whole chain is timed by.
//! - [`degauss`] the degauss transient, as a hook something else triggers.
//! - [`burn_in`] pass 0: the accumulator's shader, its decay clock and what a
//!   host graph owes it every frame.
//!
//! The wgpu features this chain needs are not this crate's to state. Nothing
//! here creates a device, so they have to be asked for at the call site, and
//! every call site asks `gpu::required_features` for them: the app's surface
//! setup, the offscreen device the grid is drawn on, and the test harness. That
//! crate says why each feature is on the set.
//!
//! # How the pass bodies are sourced
//!
//! - **Burn-in**: pass 0, from [`burn_in`], which owns the accumulator's
//!   shader, its decay clock and the mount contract. The shader written into
//!   the preset is [`burn_in::BURN_IN_SLANG`] and the
//!   host half of the mount is inside [`Chain`], which pushes
//!   `BURNIN_DECAY_STEP` before every frame, sets the rate when the settings
//!   move, and restarts the clock when a rebuild discontinues the ghost.
//!   `alias0 = Burn` gives the pass its feedback framebuffer and the
//!   `BurnFeedback` sampler; `float_framebuffer0 = true` is the decay's
//!   accuracy; `filter_linear0 = false` keeps it from smearing. Alpha is the
//!   freshness mask, not opacity, and the dynamic pass reads it back as
//!   `(1.0 - txt_blur.a)`. There is no fade-rate uniform in [`params`]: the
//!   rate is wall-clock and lives in [`burn_in::DecayClock`].
//! - **The bloom and frame passes**: passes 1-2 (the bloom, `BloomH` then
//!   `BloomSource`) and 3 (`FrameSource`). The bodies live under
//!   `shaders/bloom/` (a separable Gaussian, which is why the bloom is two
//!   passes; a mip-pyramid blur was ruled out as unportable plumbing) and
//!   `shaders/terminal_frame/`, wired into `preset::PASSES` in
//!   place of earlier scaffold stubs. Each still mounts
//!   standalone through its own `.slangp`, which is what its oracle test
//!   measures. The chassis-chrome passes that once lived alongside these have
//!   gone: `crates/chassis` owns `shaders/metal/`, `shaders/led_matrix/` and
//!   `shaders/tape_label/`, the metals' oracles and all five per-pass tests.
//!   Each still mounts standalone there, outside this chain.
//! - **Curvature closure**: the `ScreenCurvature` and `FrameSize` uniforms
//!   in [`params`] are the exact numbers `term::distortion` must invert; both
//!   are already scaled by `normalizedScreenScale` there, so the inverse takes
//!   them as they stand.

//!
//! One further module came with the pass ports: [`color`], the color-helper
//! module. The closed-form shader math the per-pass tests compare against
//! now lives in the `robco-shader-oracle` crate. This crate's own tests reach
//! the one headless GPU harness directly, as `gpu::harness`: the rebuild had
//! grown three copies of it and they are folded there, in the leaf crate that
//! owns the device concerns.

pub mod burn_in;
pub mod chain;
/// The chain's uniforms are built through these same functions.
pub use chassis::color;
pub mod degauss;
/// The measurement rig, behind the `harness` feature. Tests only.
#[cfg(feature = "harness")]
pub mod harness;
pub mod pacing;
pub mod params;
pub mod preset;

pub use chain::{Applied, Chain, Error};
pub use degauss::{Degauss, DegaussState};
pub use pacing::{FrameTime, Pacing};
pub use params::{Geometry, Params};
pub use preset::Structure;
