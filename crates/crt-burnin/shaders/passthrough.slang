#version 450

// Final pass of the standalone burn-in chain. Copies the accumulator's output
// to the viewport unchanged, alpha included, so a readback sees the freshness
// mask as well as the ghost.
//
// It exists so the feedback pass is not the last pass in the chain, which is
// also how the pass sits in the real graph: the CRT passes run after burn-in.

layout(push_constant) uniform Push {
    vec4 SourceSize;
    vec4 OriginalSize;
    vec4 OutputSize;
    uint FrameCount;
} params;

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

void main() {
    FragColor = texture(Source, vTexCoord);
}
