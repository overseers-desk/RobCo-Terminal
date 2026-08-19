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
#include "metal_common.slang"
layout(location = 0) in vec2 vTexCoord;
layout(location = 0) out vec4 FragColor;

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

    vec3 metal = chassisColor.rgb * metalField(px, 3.1) * vig;

    FragColor = vec4(metal, 1.0);
}
