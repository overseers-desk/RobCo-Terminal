#version 450

// Samples `Source` as the glyph raster driving the LED grid. Vector
// uniforms split into scalar #pragma parameter floats.
//
// MVP, builtins, and parameters all live in ONE UBO declared in the shared
// preamble -- see terminal_frame.slang's header for why.

#pragma parameter gridSizeX "Grid columns" 40.0 1.0 256.0 1.0
#pragma parameter gridSizeY "Grid rows" 16.0 1.0 256.0 1.0
#pragma parameter litColorR "Lit color R" 1.00 0.0 1.0 0.01
#pragma parameter litColorG "Lit color G" 0.55 0.0 1.0 0.01
#pragma parameter litColorB "Lit color B" 0.10 0.0 1.0 0.01
#pragma parameter litColorA "Lit color A" 1.00 0.0 1.0 0.01
#pragma parameter dimColorR "Dim color R" 0.20 0.0 1.0 0.01
#pragma parameter dimColorG "Dim color G" 0.10 0.0 1.0 0.01
#pragma parameter dimColorB "Dim color B" 0.05 0.0 1.0 0.01
#pragma parameter dimColorA "Dim color A" 1.00 0.0 1.0 0.01
#pragma parameter panelColorR "Panel color R" 0.05 0.0 1.0 0.01
#pragma parameter panelColorG "Panel color G" 0.05 0.0 1.0 0.01
#pragma parameter panelColorB "Panel color B" 0.05 0.0 1.0 0.01
#pragma parameter panelColorA "Panel color A" 1.00 0.0 1.0 0.01
#pragma parameter dotRadius "Dot radius (cell units)" 0.35 0.0 1.0 0.01
#pragma parameter threshold "Lit threshold" 0.5 0.0 1.0 0.01
#pragma parameter glow "Glow" 0.5 0.0 2.0 0.01
#pragma parameter spillMarginX "Spill margin X" 0.05 0.0 0.49 0.001
#pragma parameter spillMarginY "Spill margin Y" 0.05 0.0 0.49 0.001
#pragma parameter spillStrength "Spill strength" 0.6 0.0 2.0 0.01
#pragma parameter spillDeadX "Spill dead X" 0.2 0.0 1.0 0.01
#pragma parameter spillDeadY "Spill dead Y" 0.2 0.0 1.0 0.01

layout(std140, set = 0, binding = 0) uniform UBO {
    mat4 MVP;
    vec4 SourceSize;
    vec4 OriginalSize;
    vec4 OutputSize;
    uint FrameCount;
    float gridSizeX;
    float gridSizeY;
    float litColorR;
    float litColorG;
    float litColorB;
    float litColorA;
    float dimColorR;
    float dimColorG;
    float dimColorB;
    float dimColorA;
    float panelColorR;
    float panelColorG;
    float panelColorB;
    float panelColorA;
    float dotRadius;
    float threshold;
    float glow;
    float spillMarginX;
    float spillMarginY;
    float spillStrength;
    float spillDeadX;
    float spillDeadY;
} params;

#pragma stage vertex
layout(location = 0) in vec4 Position;
layout(location = 1) in vec2 TexCoord;
layout(location = 0) out vec2 vTexCoord;

void main() {
    gl_Position = params.MVP * Position;
    vTexCoord = TexCoord;
}

#pragma stage fragment
layout(location = 0) in vec2 vTexCoord;
layout(location = 0) out vec4 FragColor;
layout(set = 0, binding = 2) uniform sampler2D Source;

float edgeBrightness(vec2 edge, vec2 inward, float d, vec2 gridSize) {
    float reach = 1.5 + 5.0 * d;
    vec2 center = edge + inward * reach * 0.6 / gridSize;
    float sum = 0.0;
    float weight = 0.0;
    for (int y = -2; y <= 2; y++) {
        for (int x = -2; x <= 2; x++) {
            vec2 off = vec2(float(x), float(y)) * reach * 0.5;
            float w = exp(-dot(off, off) / (reach * reach));
            vec3 glyph = texture(Source, clamp(center + off / gridSize,
                                               vec2(0.0), vec2(1.0))).rgb;
            sum += w * max(glyph.r, max(glyph.g, glyph.b));
            weight += w;
        }
    }
    return sum / max(weight, 1e-4);
}

void main() {
    vec2 gridSize = vec2(params.gridSizeX, params.gridSizeY);
    vec4 litColor = vec4(params.litColorR, params.litColorG, params.litColorB, params.litColorA);
    vec4 dimColor = vec4(params.dimColorR, params.dimColorG, params.dimColorB, params.dimColorA);
    vec4 panelColor = vec4(params.panelColorR, params.panelColorG, params.panelColorB, params.panelColorA);
    vec2 spillMargin = vec2(params.spillMarginX, params.spillMarginY);
    vec2 spillDead = vec2(params.spillDeadX, params.spillDeadY);

    vec2 window = max(vec2(1.0) - 2.0 * spillMargin, vec2(1e-4));
    vec2 uv = (vTexCoord - spillMargin) / window;

    vec2 tol = 0.05 * fwidth(uv);

    if (any(lessThan(uv, -tol)) || any(greaterThan(uv, vec2(1.0) - tol))) {
        vec2 edge = clamp(uv, vec2(0.0), vec2(1.0));
        vec2 away = (uv - edge) * window / max(spillMargin, vec2(1e-4));
        float dPhys = clamp(length(away), 0.0, 1.0);
        vec2 live = max(abs(away) - spillDead, vec2(0.0))
                    / max(vec2(1.0) - spillDead, vec2(1e-4));
        float d = clamp(length(live), 0.0, 1.0);
        float falloff = 1.0 - d;
        float rise = smoothstep(0.0, 0.25, d);
        vec2 inward = -normalize(away + vec2(1e-6));
        float a = params.spillStrength * clamp(2.2 * edgeBrightness(edge, inward, dPhys, gridSize), 0.0, 1.0)
                  * rise * falloff * falloff * 0.6;
        FragColor = vec4(litColor.rgb * a, a);
        return;
    }

    vec2 cell = uv * gridSize;
    vec2 idx = floor(cell);
    float d = length(cell - idx - vec2(0.5));

    vec3 glyph = texture(Source, (idx + vec2(0.5)) / gridSize).rgb;
    float lit = step(params.threshold, max(glyph.r, max(glyph.g, glyph.b)));

    float aa = max(0.5 * fwidth(d), 0.002);
    float disk = 1.0 - smoothstep(params.dotRadius - aa, params.dotRadius + aa, d);
    float halo = params.glow * lit * (1.0 - smoothstep(params.dotRadius, params.dotRadius + 0.45, d));

    vec3 color = mix(panelColor.rgb, litColor.rgb, halo);
    color = mix(color, mix(dimColor.rgb, litColor.rgb, lit), disk);

    FragColor = vec4(color, 1.0);
}
