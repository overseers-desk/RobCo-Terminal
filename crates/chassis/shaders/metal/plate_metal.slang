#version 450

// Procedural cast-metal plate: no input texture. `sizePx` is the plate's own
// pixel size, computed in item-local pixels rather than the render
// target's; it is carried here as an explicit parameter rather than read
// from OutputSize, since a plate can be rendered at a size smaller than its
// pass's own framebuffer (a plaque inset into a larger chassis pass).
//
// MVP, builtins, and parameters all live in ONE UBO declared in the shared
// preamble -- see terminal_frame.slang's header for why.

#pragma parameter sizePxX "Plate width (px)" 256.0 1.0 4096.0 1.0
#pragma parameter sizePxY "Plate height (px)" 64.0 1.0 4096.0 1.0
#pragma parameter lightDirX "Light direction X" 0.35 -1.0 1.0 0.01
#pragma parameter lightDirY "Light direction Y" -0.6 -1.0 1.0 0.01
#pragma parameter baseColorR "Base color R" 0.30 0.0 1.0 0.01
#pragma parameter baseColorG "Base color G" 0.30 0.0 1.0 0.01
#pragma parameter baseColorB "Base color B" 0.32 0.0 1.0 0.01
#pragma parameter baseColorA "Base color A" 1.00 0.0 1.0 0.01
#pragma parameter highlightColorR "Highlight color R" 0.85 0.0 1.0 0.01
#pragma parameter highlightColorG "Highlight color G" 0.85 0.0 1.0 0.01
#pragma parameter highlightColorB "Highlight color B" 0.88 0.0 1.0 0.01
#pragma parameter highlightColorA "Highlight color A" 1.00 0.0 1.0 0.01
#pragma parameter shadowColorR "Shadow color R" 0.05 0.0 1.0 0.01
#pragma parameter shadowColorG "Shadow color G" 0.05 0.0 1.0 0.01
#pragma parameter shadowColorB "Shadow color B" 0.06 0.0 1.0 0.01
#pragma parameter shadowColorA "Shadow color A" 1.00 0.0 1.0 0.01
#pragma parameter cornerRadius "Corner radius (px)" 4.0 0.0 128.0 1.0
#pragma parameter bevelPx "Bevel width (px)" 3.0 0.0 64.0 1.0
#pragma parameter grainAmount "Grain amount" 0.06 0.0 1.0 0.01
#pragma parameter mottleAmount "Mottle amount" 0.35 0.0 1.0 0.01
#pragma parameter scratchAmount "Scratch amount" 0.08 0.0 1.0 0.01
#pragma parameter vignetteStrength "Vignette strength" 0.35 0.0 1.0 0.01
#pragma parameter wearAmount "Wear amount" 0.3 0.0 1.0 0.01
#pragma parameter seamGain "Seam gain" 0.5 0.0 1.0 0.01
#pragma parameter seed "Seed" 0.0 0.0 1000.0 0.01

layout(std140, set = 0, binding = 0) uniform UBO {
    mat4 MVP;
    vec4 SourceSize;
    vec4 OriginalSize;
    vec4 OutputSize;
    uint FrameCount;
    float sizePxX;
    float sizePxY;
    float lightDirX;
    float lightDirY;
    float baseColorR;
    float baseColorG;
    float baseColorB;
    float baseColorA;
    float highlightColorR;
    float highlightColorG;
    float highlightColorB;
    float highlightColorA;
    float shadowColorR;
    float shadowColorG;
    float shadowColorB;
    float shadowColorA;
    float cornerRadius;
    float bevelPx;
    float grainAmount;
    float mottleAmount;
    float scratchAmount;
    float vignetteStrength;
    float wearAmount;
    float seamGain;
    float seed;
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
#include "metal_common.slang"
layout(location = 0) in vec2 vTexCoord;
layout(location = 0) out vec4 FragColor;

void main() {
    vec2 sizePx = vec2(params.sizePxX, params.sizePxY);
    vec2 lightDir = vec2(params.lightDirX, params.lightDirY);
    vec4 baseColor = vec4(params.baseColorR, params.baseColorG, params.baseColorB, params.baseColorA);
    vec4 highlightColor = vec4(params.highlightColorR, params.highlightColorG, params.highlightColorB, params.highlightColorA);
    vec4 shadowColor = vec4(params.shadowColorR, params.shadowColorG, params.shadowColorB, params.shadowColorA);

    vec2 px = vTexCoord * sizePx;
    float d = rrectPx(px, vec2(0.5), sizePx - vec2(0.5), params.cornerRadius);
    float coverage = 1.0 - smoothstep(-1.0, 0.5, d);

    vec2 uvn = vTexCoord;
    vec2 nrm = normalize(px - sizePx * 0.5 + vec2(1e-5));
    vec2 L = normalize(lightDir);

    float surf = metalField(px + vec2(params.seed * 137.0, params.seed * 61.0), 9.2);
    vec3 col = baseColor.rgb * surf;

    float blotch = smoothstep(0.60, 0.92, fbm(px * 0.008 + vec2(params.seed * 31.0)));
    col = mix(col, highlightColor.rgb * 0.42, params.wearAmount * 0.55 * blotch);

    float edgeDist = -d;
    float nearEdge = smoothstep(params.bevelPx + 6.0, 0.0, edgeDist);
    float wearNoise = fbm(px * 0.03 + vec2(params.seed * 17.0));
    col = mix(col, highlightColor.rgb * (0.45 + 0.8 * wearNoise),
              params.wearAmount * nearEdge * smoothstep(0.35, 0.8, wearNoise));

    float bt = 1.0 - smoothstep(0.0, params.bevelPx, px.y);
    float bl = 1.0 - smoothstep(0.0, params.bevelPx, px.x);
    float br = 1.0 - smoothstep(0.0, params.bevelPx, sizePx.x - px.x);
    float bb = 1.0 - smoothstep(0.0, params.bevelPx, sizePx.y - px.y);
    float lit = bt * clamp(-L.y, 0.0, 1.0) + bl * clamp(-L.x, 0.0, 1.0)
              + br * clamp(L.x, 0.0, 1.0) + bb * clamp(L.y, 0.0, 1.0);
    float shad = bt * clamp(L.y, 0.0, 1.0) + bl * clamp(L.x, 0.0, 1.0)
               + br * clamp(-L.x, 0.0, 1.0) + bb * clamp(-L.y, 0.0, 1.0);
    col += highlightColor.rgb * clamp(lit, 0.0, 1.0) * 0.9;
    col = mix(col, shadowColor.rgb, clamp(shad, 0.0, 1.0) * 0.7);

    float seam = 1.0 - smoothstep(0.8, 2.8, abs(edgeDist - (params.bevelPx + 2.0)));
    col *= 1.0 - 0.35 * params.seamGain * seam;

    vec2 lightPos = clamp(vec2(0.5) + L * 0.5, 0.0, 1.0);
    float away = clamp(dot(uvn - lightPos, -L), 0.0, 2.0);
    float vig = 1.0 - params.vignetteStrength * smoothstep(0.0, 1.2, away);
    float cornerD = length((uvn - 0.5) * 2.0) / 1.414;
    vig *= 1.0 - params.vignetteStrength * 0.5 * smoothstep(0.6, 1.0, cornerD);
    col *= vig;

    FragColor = vec4(col, 1.0) * coverage;
}
