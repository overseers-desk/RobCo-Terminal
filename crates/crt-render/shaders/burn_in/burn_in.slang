#version 450

// Burn-in accumulator.
//
// The pass reads its own previous-frame output. librashader binds that under
// `<alias>Feedback`, and the alias comes from `alias = Burn` in the preset, so
// the sampler below is `BurnFeedback`. Drop the alias and the pass silently
// loses its previous frame: no error, just no ghost.
//
// Two things matter here and neither is decoration:
//
//   * The decay is a *subtraction*, not a multiplication. It is computed as
//     `clamp((lastUpdate - prevLastUpdate) * burnInTime, 0, 1)` from the real
//     frame delta and subtracts it, so the ghost falls on a linear ramp whose
//     slope is wall-clock time, not frame count. That subtraction term is the
//     only thing this shader gets from the CPU: `BURNIN_DECAY_STEP`, pushed
//     every frame (see `DecayClock` on the Rust side).
//
//   * The alpha channel is not alpha. It is `currMask`, the freshness mask:
//     1.0 where the accumulator is currently being driven by the live source
//     rather than by its own ghost, i.e. where the pixel was just lit. The next
//     frame reads it back as `prevMask` and subtracts it from the decay step,
//     which zeroes decay for one frame on a newly-lit pixel. Without it a
//     freshly lit pixel starts fading in the same frame it appears.
//
// `Source` (not `Original`) is the live input, so the pass composes wherever it
// sits in a longer chain: in a standalone chain it is pass 0 and the two are the
// same texture, but mounted after the terminal grid pass it must read that
// pass's output.

layout(push_constant) uniform Push {
    vec4 SourceSize;
    vec4 OriginalSize;
    vec4 OutputSize;
    uint FrameCount;
    // Wall-clock decay for this frame: `clamp(dt_seconds / fade_time, 0, 1)`,
    // computed on the CPU from the real frame delta and pushed every frame.
    // 1.0 means "decay fully this frame", which is how burn-in switches off
    // without a chain rebuild.
    float BURNIN_DECAY_STEP;
    // 1 keeps the freshness mask, 0 disables it. Exists so a test can hold one
    // chain and measure what the mask does; the application leaves it at 1.
    float BURNIN_MASK;
} params;

#pragma parameter BURNIN_DECAY_STEP "Burn-in decay this frame" 0.0 0.0 1.0 0.0001
#pragma parameter BURNIN_MASK "Freshness mask on newly-lit pixels" 1.0 0.0 1.0 1.0

layout(std140, set = 0, binding = 0) uniform UBO {
    mat4 MVP;
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
layout(set = 0, binding = 2) uniform sampler2D Source;
layout(set = 0, binding = 3) uniform sampler2D BurnFeedback;

float rgb2grey(vec3 v) {
    return dot(v, vec3(0.21, 0.72, 0.04));
}

void main() {
    vec3 txtColor = texture(Source, vTexCoord).rgb;
    vec4 accColor = texture(BurnFeedback, vTexCoord);

    float prevMask = accColor.a * params.BURNIN_MASK;

    float blurDecay = clamp(params.BURNIN_DECAY_STEP, 0.0, 1.0);
    blurDecay = max(0.0, blurDecay - prevMask);
    vec3 color = max(accColor.rgb - vec3(blurDecay), txtColor);

    float currMask = step(rgb2grey(color), rgb2grey(txtColor));

    FragColor = vec4(color, currMask);
}
