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

void main() {
    float sigma = max(global.radius / 3.0, 1e-4);
    // One output pixel; see bloom_h.slang for why it is not a source texel.
    float texelH = params.OutputSize.w;

    vec4 sum = vec4(0.0);
    float wsum = 0.0;
    for (int i = -7; i <= 7; i++) {
        float y = float(i) * (global.radius / 7.0);
        float w = exp(-0.5 * (y * y) / (sigma * sigma));
        vec2 uv = vTexCoord + vec2(0.0, y * texelH);
        sum += texture(Source, uv) * w;
        wsum += w;
    }
    FragColor = sum / max(wsum, 1e-6);
}
