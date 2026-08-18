#version 450

// Procedural: no input texture, the whole surface is fbm-noise-and-lighting.
//
// `viewportSize` comes from OutputSize, and OutputSize here is *not* the size
// of what this pass draws: the casting is measured in the frame's field,
// so the host declares the screen well's size on the
// output view while the quad covers the bank column
// (`app::column`'s "Which size is which", which has the reasoning and the
// librashader mechanics -- the declared size and the viewport rect are two
// different things). That well goes in on the logical ruler `frame_metal`
// reaches by dividing its own OutputSize by `windowScaling * DPR`, so the two
// castings share one field across the seam at any pixel ratio.
//
// Vector uniforms (fieldScale/fieldOffset/lightDir, chassisColor) are split
// into scalar #pragma parameter floats, RetroArch's parameter grammar having
// no vector type, and reassembled in main().
//
// MVP, builtins, and parameters all live in ONE UBO declared in the shared
// preamble (not push_constant, and not a second per-stage block) -- see
// terminal_frame.slang's header for the two reasons why (the push-constant
// byte budget, and librashader's one-uniform-buffer-per-stage limit).

#pragma parameter fieldScaleX "Field scale X" 1.0 0.0 8.0 0.01
#pragma parameter fieldScaleY "Field scale Y" 1.0 0.0 8.0 0.01
#pragma parameter fieldOffsetX "Field offset X" 0.0 -8.0 8.0 0.01
#pragma parameter fieldOffsetY "Field offset Y" 0.0 -8.0 8.0 0.01
#pragma parameter lightDirX "Light direction X" 0.35 -1.0 1.0 0.01
#pragma parameter lightDirY "Light direction Y" -0.6 -1.0 1.0 0.01
#pragma parameter chassisColorR "Chassis color R" 0.30 0.0 1.0 0.01
#pragma parameter chassisColorG "Chassis color G" 0.30 0.0 1.0 0.01
#pragma parameter chassisColorB "Chassis color B" 0.32 0.0 1.0 0.01
#pragma parameter chassisColorA "Chassis color A" 1.00 0.0 1.0 0.01
#pragma parameter grainAmount "Grain amount" 0.06 0.0 1.0 0.01
#pragma parameter mottleAmount "Mottle amount" 0.35 0.0 1.0 0.01
#pragma parameter scratchAmount "Scratch amount" 0.08 0.0 1.0 0.01
#pragma parameter vignetteStrength "Vignette strength" 0.35 0.0 1.0 0.01

layout(std140, set = 0, binding = 0) uniform UBO {
    mat4 MVP;
    vec4 SourceSize;
    vec4 OriginalSize;
    vec4 OutputSize;
    uint FrameCount;
    float fieldScaleX;
    float fieldScaleY;
    float fieldOffsetX;
    float fieldOffsetY;
    float lightDirX;
    float lightDirY;
    float chassisColorR;
    float chassisColorG;
    float chassisColorB;
    float chassisColorA;
    float grainAmount;
    float mottleAmount;
    float scratchAmount;
    float vignetteStrength;
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

float hash12(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

float vnoise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash12(i), hash12(i + vec2(1.0, 0.0)), u.x),
               mix(hash12(i + vec2(0.0, 1.0)), hash12(i + vec2(1.0, 1.0)), u.x), u.y);
}

float fbm(vec2 p) {
    float a = 0.5;
    float s = 0.0;
    for (int i = 0; i < 4; i++) {
        s += a * vnoise(p);
        p = p * 2.13 + vec2(11.7, 5.3);
        a *= 0.5;
    }
    return s;
}

float metalField(vec2 px) {
    float m = fbm(px * 0.006);
    float m2 = fbm(px * 0.02 + 31.4);
    float tone = 1.0 + params.mottleAmount * (0.9 * (m - 0.5) + 0.5 * (m2 - 0.5));
    float stain = smoothstep(0.60, 0.85, fbm(px * 0.004 + 7.7));
    tone -= params.mottleAmount * 0.35 * stain;
    float g = (vnoise(px * 0.9) - 0.5) + 0.6 * (hash12(px) - 0.5);
    tone += params.grainAmount * g;
    float streak = vnoise(vec2(px.x * 0.012, px.y * 0.4));
    tone += params.scratchAmount * 0.3 * (streak - 0.5);
    float fine = vnoise(vec2(px.x * 0.5, px.y * 0.05) + 3.7);
    tone += params.scratchAmount * 0.22 * (fine - 0.5);
    float pit = smoothstep(0.78, 0.95, vnoise(px * 0.09 + 13.1));
    tone -= params.mottleAmount * 0.3 * pit;
    float speck = step(0.9975, hash12(floor(px * 0.7) + 3.1));
    tone += params.scratchAmount * speck * 1.4;
    return max(tone, 0.0);
}

void main() {
    vec2 viewportSize = params.OutputSize.xy;
    vec2 fieldScale = vec2(params.fieldScaleX, params.fieldScaleY);
    vec2 fieldOffset = vec2(params.fieldOffsetX, params.fieldOffsetY);
    vec2 lightDir = vec2(params.lightDirX, params.lightDirY);
    vec4 chassisColor = vec4(params.chassisColorR, params.chassisColorG, params.chassisColorB, params.chassisColorA);

    vec2 staticCoords = vTexCoord * fieldScale + fieldOffset;
    vec2 px = staticCoords * viewportSize;

    vec2 L = normalize(lightDir);
    vec2 lightPos = clamp(vec2(0.5) + L * 0.45, 0.0, 1.0);
    float away = clamp(dot(staticCoords - lightPos, -L), 0.0, 2.0);
    float vig = 1.0 - params.vignetteStrength * smoothstep(0.0, 1.1, away);
    float cornerD = length((staticCoords - 0.5) * 2.0) / 1.414;
    vig *= 1.0 - params.vignetteStrength * 0.55 * smoothstep(0.62, 1.0, cornerD);

    vec3 metal = chassisColor.rgb * metalField(px) * vig;

    FragColor = vec4(metal, 1.0);
}
