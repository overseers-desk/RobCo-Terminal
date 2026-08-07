#version 440

layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform ubuf {
    mat4 qt_Matrix;
    float qt_Opacity;
    vec2 gridSize;
    vec4 litColor;
    vec4 dimColor;
    vec4 panelColor;
    float dotRadius;
    float threshold;
    float glow;
    float pixelsPerCell;
};

layout(binding = 1) uniform sampler2D source;

void main() {
    vec2 cell = qt_TexCoord0 * gridSize;
    vec2 idx = floor(cell);
    float d = length(cell - idx - vec2(0.5));

    // One glyph texel per LED: snap-sample the cell center and binarize.
    vec3 glyph = texture(source, (idx + vec2(0.5)) / gridSize).rgb;
    float lit = step(threshold, max(glyph.r, max(glyph.g, glyph.b)));

    float aa = 1.0 / max(pixelsPerCell, 1.0);
    float disk = 1.0 - smoothstep(dotRadius - aa, dotRadius + aa, d);
    float halo = glow * lit * (1.0 - smoothstep(dotRadius, dotRadius + 2.0 * aa, d));

    vec3 color = mix(panelColor.rgb, litColor.rgb, halo);
    color = mix(color, mix(dimColor.rgb, litColor.rgb, lit), disk);

    fragColor = vec4(color, 1.0) * qt_Opacity;
}
