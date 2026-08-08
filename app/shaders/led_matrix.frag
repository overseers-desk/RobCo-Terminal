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
    vec2 spillMargin;
    float spillStrength;
};

layout(binding = 1) uniform sampler2D source;

// How much light the dots inside the window throw at a point outside it: the
// glyph read back over a patch of cells near the closest edge. The patch grows
// and sinks further in with distance, the way a penumbra widens, so the spill
// only appears where LEDs are actually lit but is not starved by one blank
// row of the font.
float edgeBrightness(vec2 edge, vec2 inward, float d) {
    float reach = 1.5 + 5.0 * d;
    vec2 center = edge + inward * reach * 0.6 / gridSize;
    float sum = 0.0;
    float weight = 0.0;
    for (int y = -2; y <= 2; y++) {
        for (int x = -2; x <= 2; x++) {
            vec2 off = vec2(float(x), float(y)) * reach * 0.5;
            float w = exp(-dot(off, off) / (reach * reach));
            vec3 glyph = texture(source, clamp(center + off / gridSize,
                                               vec2(0.0), vec2(1.0))).rgb;
            sum += w * max(glyph.r, max(glyph.g, glyph.b));
            weight += w;
        }
    }
    return sum / max(weight, 1e-4);
}

void main() {
    // The item is grown by a margin of plastic on every side; the window itself
    // is the unit square of this coordinate.
    vec2 window = max(vec2(1.0) - 2.0 * spillMargin, vec2(1e-4));
    vec2 uv = (qt_TexCoord0 - spillMargin) / window;

    if (any(lessThan(uv, vec2(0.0))) || any(greaterThan(uv, vec2(1.0)))) {
        vec2 edge = clamp(uv, vec2(0.0), vec2(1.0));
        vec2 away = (uv - edge) * window / max(spillMargin, vec2(1e-4));
        // How far out of the window we are, as a fraction of the margin: 0 at
        // the lip, 1 where the light has to be gone.
        float d = clamp(length(away), 0.0, 1.0);
        float falloff = 1.0 - d;
        vec2 inward = -normalize(away + vec2(1e-6));
        float a = spillStrength * clamp(2.2 * edgeBrightness(edge, inward, d), 0.0, 1.0)
                  * falloff * falloff * 0.6;
        // Premultiplied: light added to the plastic, never a sheet over it.
        fragColor = vec4(litColor.rgb * a, a) * qt_Opacity;
        return;
    }

    vec2 cell = uv * gridSize;
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
