#version 450

// Bloom, vertical pass. See bloom_h.slang for the full account of the
// divergence from FastBlur. Same kernel, transposed.

// `radius` in the UBO, builtins in the push block: see bloom_h.slang.
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
layout(set = 0, binding = 2) uniform sampler2D Source;

// Same stride rule as bloom_h.slang: at most one texel of the sampled
// texture between taps, the count following the radius. `Source` here is
// bloom_h's framebuffer, the same size as this one, so the stride is one
// output pixel; the tightening is kept for the standalone presets, where the
// scales are whatever the preset says.
void main() {
    float sigma = max(global.radius / 3.0, 1e-4);
    float texelH = params.OutputSize.w;
    float sourcePerOutput = params.SourceSize.y * params.OutputSize.w;
    float stride = min(1.0, 1.0 / max(sourcePerOutput, 1e-4));
    int n = min(int(ceil(global.radius / stride)), 512);

    vec4 sum = vec4(0.0);
    float wsum = 0.0;
    for (int i = -n; i <= n; i++) {
        float y = float(i) * stride;
        float w = exp(-0.5 * (y * y) / (sigma * sigma));
        vec2 uv = vTexCoord + vec2(0.0, y * texelH);
        sum += texture(Source, uv) * w;
        wsum += w;
    }
    FragColor = sum / max(wsum, 1e-6);
}
