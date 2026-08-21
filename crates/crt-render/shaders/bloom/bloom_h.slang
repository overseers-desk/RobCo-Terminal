#version 450

// Bloom, horizontal pass. `bloomSource` was generated with FastBlur, a
// growing/variable-width box-blur pyramid (`radius: lint(16, 64,
// bloomQuality)`, applied to a copy of the terminal downsampled to
// `bloomQuality` of full size). FastBlur's exact per-tap weights are
// undocumented and not vendored in this repository (see rebuild-stack.md's
// inheritance list: that chaining is explicitly left-behind plumbing).
//
// Deliberate divergence, recorded per the card: this ports the *portable
// substance* -- a separable blur of a given pixel radius feeding the same
// `bloom` composite in terminal_static.frag (`finalColor += clamp(bloomOnScreen
// * bloom * bloomAlpha, 0, 0.5)`) -- as a standard two-pass separable Gaussian
// (this file + bloom_v.slang) rather than reverse-engineering FastBlur's
// undocumented box-pyramid. A Gaussian with sigma = radius/3 is visually
// equivalent glow for the terminal's purposes and is exactly analytic, which
// a box-pyramid approximation is not. The `radius` parameter carries the
// `lint(16, 64, bloomQuality)` mapping (computed on the Rust side, see
// crt::color); `bloomQuality`'s effect on the source's own downsample
// resolution is a `.slangp` `scale0` choice, made by whatever preset embeds
// this pass, not by the shader.

// `radius` sits in the UBO rather than the push block, which is this repo's
// standing convention for every ported parameter (see terminal_static.slang and
// terminal_dynamic.slang): librashader's reflector caps the push range at 128
// bytes per stage whatever the device supports, and it rejects a second
// per-stage uniform block, so a pass that gains parameters over time only has
// room to grow on the UBO side. The builtins stay in the push block, which is
// where librashader's own semantics for them live.
layout(push_constant) uniform Push {
    vec4 SourceSize;
    vec4 OriginalSize;
    vec4 OutputSize;
    uint FrameCount;
} params;

#pragma parameter radius "Bloom blur radius (px)" 16.0 0.0 64.0 1.0

layout(std140, set = 0, binding = 0) uniform UBO {
    mat4 MVP;
    float radius;
} global;

#pragma stage vertex
layout(location = 0) in vec4 Position;
layout(location = 1) in vec2 TexCoord;
layout(location = 0) out vec2 vTexCoord;

void main() {
    gl_Position = global.MVP * Position;
    vTexCoord = TexCoord;
}

#pragma stage fragment
layout(location = 0) in vec2 vTexCoord;
layout(location = 0) out vec4 FragColor;
// The terminal image, `Original` and not `Source` in a librashader chain:
// this pass sits after the burn-in accumulator, so `Source` here would be
// the ghost. In the standalone `bloom_h.slangp` / `bloom.slangp` presets
// this pass is index 0, where `Original` and `Source` are the same texture,
// so the ported tests measure the same shader the chain runs.
layout(set = 0, binding = 2) uniform sampler2D Original;

// sigma = radius / 3: the tap window reaches +/-radius and keeps ~99% of the
// Gaussian mass inside it for any radius in the parameter's declared range.
//
// The taps sit at most one texel of the sampled texture apart. A bilinear
// fetch is a tent two texels wide, and tents spaced one texel (or any
// divisor of one) sum to a constant: a kernel built of them is a blur,
// whatever its alignment to the grid. Spaced wider, the tents leave gaps the
// kernel never reads, and a lit pixel comes out as a row of copies at the
// tap pitch rather than a halo. So the stride is one output pixel, tightened
// to one source texel where an output pixel spans more than one of them --
// the chain's case, where this framebuffer is `bloomQuality` of the terminal
// -- and the tap count follows the radius instead of being fixed. The cost
// is the radius in fetches per pixel per pass, at its highest (129 a pass)
// with `bloomQuality` at 1; at the default 0.5 the framebuffer has a quarter
// of the pixels and the bill is a fifth of that.
void main() {
    float sigma = max(global.radius / 3.0, 1e-4);
    // One *output* pixel, not one source texel. The radius is in the
    // pixels of the item it blurs, which is the terminal downscaled to
    // `bloomQuality`; in the chain that item is this pass's own framebuffer,
    // which is `bloomQuality` of the original and not the same size as either
    // `Source` or `Original`. In the standalone presets all three sizes are
    // equal, so the ported tests measure the same numbers either way.
    float texelW = params.OutputSize.z;
    // Source texels per output pixel, and the stride that keeps a tap
    // within one of them.
    float sourcePerOutput = params.OriginalSize.x * params.OutputSize.z;
    float stride = min(1.0, 1.0 / max(sourcePerOutput, 1e-4));
    // 512 taps a side is the full +/-radius window down to a `bloomQuality`
    // of 0.04; below that the framebuffer is a few dozen pixels and the
    // truncation is moot.
    int n = min(int(ceil(global.radius / stride)), 512);

    vec4 sum = vec4(0.0);
    float wsum = 0.0;
    for (int i = -n; i <= n; i++) {
        float x = float(i) * stride;
        float w = exp(-0.5 * (x * x) / (sigma * sigma));
        vec2 uv = vTexCoord + vec2(x * texelW, 0.0);
        sum += texture(Original, uv) * w;
        wsum += w;
    }
    FragColor = sum / max(wsum, 1e-6);
}
