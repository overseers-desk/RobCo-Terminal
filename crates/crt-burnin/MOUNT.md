# Mounting the burn-in pass

What a host filter graph (`crt-render`) has to do to run the accumulator inside
its own preset. Everything here is measured in `tests/burn_in.rs` and
`tests/mount.rs`; the numbers quoted are from those runs on Intel UHD / Vulkan
under Xvfb.

## The preset block

Put the accumulator wherever the ghost belongs in the chain: after the pass that
draws the terminal grid, before the CRT passes that bend and colour it. It reads
`Source`, not `Original`, so it takes whatever pass precedes it, and it must not
be the last pass.

Call `crt_burnin::preset_pass_block(index, "burn_in.slang")` rather than writing
the block by hand, and `crt_burnin::write_shader(dir)` to put the shader beside a
generated preset. The block it produces:

```
shader{N} = burn_in.slang
alias{N} = Burn
scale_type{N} = source
scale{N} = 1.0
filter_linear{N} = false
wrap_mode{N} = clamp_to_edge
float_framebuffer{N} = true
```

`crt-render` is the one host that exists, and it writes the block itself: it
assembles the whole preset by walking its pass list, and pass 0's framebuffer is
sized by `general.burn_in_quality`, a structural setting this block knows
nothing about. So it takes `scale_type`/`scale` from that setting and every
other line from here, and its `pass_zero_is_the_block_the_mount_contract_asks_for`
test holds the generated text to `preset_pass_block` line by line with those two
exempted. A host that wants the accumulator at the size of whatever precedes it,
which is most of them, should just call the function.

Every line earns its place:

- `alias{N} = Burn` is what gives the pass a feedback framebuffer at all, and
  `BurnFeedback` (alias + `Feedback`) is the sampler name the shader declares.
  Omit it and the pass compiles, runs, and has no previous frame.
- `float_framebuffer` is the accuracy. Measured over a 14-frame fade at the
  default `burn_in = 0.25`: 0.3418% worst error against the ramp the set rate
  predicts, versus 1.3235% for the same chain at 8 bits. Over S1's 12-frame
  window the float number is 0.2580%.
- `filter_linear = false` and `wrap_mode = clamp_to_edge`: the accumulator is
  read at exactly the coordinates it was written at, and a linear filter would
  smear the ghost sideways a little more every frame.

  **Unless the accumulator is pass 0.** librashader binds the chain's
  `Original` -- every pass's view of the picture the chain was handed -- with
  the filter and wrap mode of *pass 0*, whichever pass that is
  (`librashader-runtime-wgpu/src/filter_chain.rs`, `let filter =
  passes[0].meta.filter`). Put the accumulator first and this line stops being
  about the ghost: it decides how the whole chain samples its input. `crt-render`
  mounts it first and sets `filter_linear0 = true` for that reason, and its
  `the_first_pass_carries_the_chains_filter_for_the_terminal_grid` test says so;
  the ghost is unharmed because a pass whose framebuffer is the size of its own
  feedback samples that feedback at texel centres, where linear and nearest
  agree. A host that mounts the accumulator anywhere else should take the line
  as written.

### Precision, if the device allows it

`float_framebuffer` is fp16, not fp32: librashader's `get_format_override` maps
it to `R16G16B16A16Sfloat`, and that override beats any `#pragma format` in the
shader. At fp16 the ULP near 1.0 is 1/2048, about 1.5% of a 60 Hz decay step, and
the rounding is one-directional, which is the whole of the 0.26% drift above.

An fp32 accumulator removes it completely (0.0000% over the same run). It needs
two things: `Precision::Fp32`, which injects `#pragma format
R32G32B32A32_SFLOAT` into the shader and leaves `float_framebuffer` out of the
preset, and a device with `wgpu::Features::FLOAT32_FILTERABLE`. Without that
feature it is not a slower path, it is a wgpu validation error inside
librashader's bind group, because librashader asks for a filterable float sample
type for every sampled texture. So:

```rust
let precision = Precision::for_device(&device); // Fp32 when the feature is there
```

and request `FLOAT32_FILTERABLE` at device creation when the adapter offers it.
Defaulting to `Fp16` everywhere is also a perfectly good answer; it is what the
shipped preset does, and 0.26% over a fade nobody watches for 12 frames is not
visible.

## Per frame

The host owns one `BurnInPass` per mounted accumulator and does exactly this,
in this order:

```rust
burn_in.push(chain.parameters(), now_seconds); // before frame(), every frame
chain.frame(&input, &viewport, &mut cmd, frame_index, None)?;
```

`now_seconds` is any monotonic clock, as long as it is the same one every frame;
the pass only ever looks at differences. `push` writes two parameters
(`BURNIN_DECAY_STEP`, `BURNIN_MASK`) and returns the decay it wrote. Do it
unconditionally: a parameter write is 83 ns (S1), so skipping it on unchanged
frames saves nothing and risks a frame running on a stale delta.

Nothing else about the pass is the host's business. It has no textures to bind,
no per-frame uniform block, and no state outside the chain's own feedback
framebuffer.

### The calls the host must not forget

| Host event | Call | Why |
|---|---|---|
| `screen.burn_in` changed in the config | `set_burn_in(v)` | Parameter-level (R0(d)): no rebuild, and the ghost on screen keeps fading from where it is, at the new rate. |
| Chain rebuilt on a structural key, window resized, font changed | `restart()` | The accumulator's contents are discontinuous; the next frame must decay by nothing. |
| Frame skipped, window occluded, process resumed | nothing | The clock clamps any delta above 0.25 s to 0.25 s, so a resumed process fades by one frame's worth rather than erasing the ghost. |

`burn_in = 0` needs no special handling: the clock pushes a decay of 1.0 and
clears the mask, the accumulator collapses to the live image, and the ghost is
gone from the next frame. That is deliberately a uniform rather than a chain
variant, per R0(d).

## Compositing the ghost (whoever owns `terminal_dynamic`)

The accumulator only holds the ghost. The picture is mixed with it in
`terminal_dynamic.slang`, and those three lines are burn-in semantics, not
dynamic-pass semantics, so they are quoted here rather than re-derived:

```glsl
vec4 txt_blur = texture(burnInSource, staticCoords);
float blurDecay = clamp((time - burnInLastUpdate) * burnInTime, 0.0, 1.0);
vec3 burnInColor = 0.65 * (txt_blur.rgb - vec3(blurDecay)) * (1.0 - txt_blur.a);
txt_color = max(txt_color, burnInColor);
```

Two notes for the port:

- `(1.0 - txt_blur.a)` is the freshness mask again, on the consuming side: where
  the accumulator says the pixel is currently live, the ghost contributes
  nothing, because the live text is already drawing it.
- The `blurDecay` term extrapolates decay for the time since the accumulator was
  last updated, which matters only when the accumulator does not run every
  frame. In this graph the accumulator runs on every rendered frame, so that
  interval is zero and the term drops out. If the graph ever gains a
  damage-gated accumulator, the term comes back and `BurnInPass` is where the
  two timestamps would live.

The `0.65` is a brightness trim on the ghost and belongs with the composite.

## What the mount is tested against

`crt::chain::Chain` is the mount that ships, and it holds all three of the calls
above so that nothing above the render crate can drop one:
`crt-render/tests/burn_in_chain.rs` measures the ghost through the whole
five-pass graph, composite included, and reads 0.42% against the set rate over
twelve frames.

`tests/mount.rs` builds a three-pass preset from `preset_pass_block` with the
accumulator at index 1, a copy pass either side, and checks the ghost, the mask
and the rate survive being mounted in the middle of a chain rather than at its
head. If `crt-render`'s graph changes shape, that test is the one to re-run
first.
