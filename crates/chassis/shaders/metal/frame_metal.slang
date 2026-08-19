#version 450

// The metal-surface math (`metalField`, noise, `rrectPx`) comes from
// `metal_common.slang`, shared by the three metals in this directory.
// Vector uniforms split into scalar #pragma parameter floats as in
// chassis_metal.slang; `viewportSize` comes from OutputSize, divided by the
// `windowScaling` uniform -- which carries `windowScaling * devicePixelRatio`,
// because OutputSize is a physical-pixel framebuffer and viewportSize is a
// logical-pixel item size (see main(), and the push site in
// `crt::params::Params::build`).
//
// MVP, builtins, and this shader's 36 parameters all live in ONE UBO
// declared in the shared preamble -- see terminal_frame.slang's header for
// why (this shader alone would need ~208 push-constant bytes, well past
// Vulkan's guaranteed-minimum 128-byte range, and a second per-stage block
// hits librashader's one-uniform-buffer-per-stage limit).

#pragma parameter screenCurvature "Screen curvature" 0.30 0.0 1.0 0.01
#pragma parameter frameSize "Frame size" 0.05 0.0 1.0 0.001
#pragma parameter screenRadius "Screen radius (px)" 8.0 0.0 128.0 1.0
#pragma parameter ambientLight "Ambient light" 0.3 0.0 1.0 0.01
#pragma parameter frameShininess "Frame shininess" 0.0 0.0 1.0 0.01
#pragma parameter lightDirX "Light direction X" 0.35 -1.0 1.0 0.01
#pragma parameter lightDirY "Light direction Y" -0.6 -1.0 1.0 0.01
#pragma parameter bezelColorR "Bezel color R" 0.32 0.0 1.0 0.01
#pragma parameter bezelColorG "Bezel color G" 0.32 0.0 1.0 0.01
#pragma parameter bezelColorB "Bezel color B" 0.34 0.0 1.0 0.01
#pragma parameter bezelColorA "Bezel color A" 1.00 0.0 1.0 0.01
#pragma parameter chassisColorR "Chassis color R" 0.30 0.0 1.0 0.01
#pragma parameter chassisColorG "Chassis color G" 0.30 0.0 1.0 0.01
#pragma parameter chassisColorB "Chassis color B" 0.32 0.0 1.0 0.01
#pragma parameter chassisColorA "Chassis color A" 1.00 0.0 1.0 0.01
#pragma parameter ridgeColorR "Ridge color R" 0.55 0.0 1.0 0.01
#pragma parameter ridgeColorG "Ridge color G" 0.55 0.0 1.0 0.01
#pragma parameter ridgeColorB "Ridge color B" 0.58 0.0 1.0 0.01
#pragma parameter ridgeColorA "Ridge color A" 1.00 0.0 1.0 0.01
#pragma parameter bezelMarginsX "Bezel margin left (px)" 8.0 0.0 128.0 1.0
#pragma parameter bezelMarginsY "Bezel margin top (px)" 8.0 0.0 128.0 1.0
#pragma parameter bezelMarginsZ "Bezel margin right (px)" 8.0 0.0 128.0 1.0
#pragma parameter bezelMarginsW "Bezel margin bottom (px)" 8.0 0.0 128.0 1.0
#pragma parameter outerRadius "Outer radius (px)" 6.0 0.0 128.0 1.0
#pragma parameter wellDepth "Well depth (px)" 18.0 0.0 128.0 1.0
#pragma parameter wellFloor "Well floor" 0.55 0.0 1.0 0.01
#pragma parameter ridgeGain "Ridge gain" 0.6 0.0 2.0 0.01
#pragma parameter grainAmount "Grain amount" 0.06 0.0 1.0 0.01
#pragma parameter mottleAmount "Mottle amount" 0.35 0.0 1.0 0.01
#pragma parameter scratchAmount "Scratch amount" 0.08 0.0 1.0 0.01
#pragma parameter vignetteStrength "Vignette strength" 0.35 0.0 1.0 0.01
#pragma parameter fillGain "Fill gain" 0.35 0.0 1.0 0.01
#pragma parameter troughGain "Trough gain" 0.4 0.0 1.0 0.01
#pragma parameter faceBandPx "Face band (px)" 0.0 0.0 64.0 1.0
#pragma parameter rimDistPx "Rim distance (px)" 0.0 0.0 64.0 1.0
#pragma parameter rimGain "Rim gain" 0.0 0.0 2.0 0.01
// `windowScaling * devicePixelRatio`, pushed as one value (see main()); the
// range runs to 16 so 4x scaling on a 4x display is still inside it.
#pragma parameter windowScaling "Framebuffer-to-item ruler (windowScaling * DPR)" 1.0 0.01 16.0 0.01

layout(std140, set = 0, binding = 0) uniform UBO {
    mat4 MVP;
    vec4 SourceSize;
    vec4 OriginalSize;
    vec4 OutputSize;
    uint FrameCount;
    float screenCurvature;
    float frameSize;
    float screenRadius;
    float ambientLight;
    float frameShininess;
    float lightDirX;
    float lightDirY;
    float bezelColorR;
    float bezelColorG;
    float bezelColorB;
    float bezelColorA;
    float chassisColorR;
    float chassisColorG;
    float chassisColorB;
    float chassisColorA;
    float ridgeColorR;
    float ridgeColorG;
    float ridgeColorB;
    float ridgeColorA;
    float bezelMarginsX;
    float bezelMarginsY;
    float bezelMarginsZ;
    float bezelMarginsW;
    float outerRadius;
    float wellDepth;
    float wellFloor;
    float ridgeGain;
    float grainAmount;
    float mottleAmount;
    float scratchAmount;
    float vignetteStrength;
    float fillGain;
    float troughGain;
    float faceBandPx;
    float rimDistPx;
    float rimGain;
    float windowScaling;
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

float min2(vec2 v) { return min(v.x, v.y); }
float prod2(vec2 v) { return v.x * v.y; }

vec2 distortCoordinates(vec2 coords) {
    vec2 paddedCoords = coords * (vec2(1.0) + params.frameSize * 2.0) - params.frameSize;
    vec2 cc = (paddedCoords - vec2(0.5));
    float dist = dot(cc, cc) * params.screenCurvature;
    return (paddedCoords + cc * (1.0 + dist) * dist);
}

float roundedRectSdfPixels(vec2 p, vec2 topLeft, vec2 bottomRight, float radiusPixels, vec2 viewportSize) {
    vec2 sizePixels = (bottomRight - topLeft) * viewportSize;
    vec2 centerPixels = (topLeft + bottomRight) * 0.5 * viewportSize;
    vec2 localPixels = p * viewportSize - centerPixels;
    vec2 halfSize = sizePixels * 0.5 - vec2(radiusPixels);
    vec2 d = abs(localPixels) - halfSize;
    return length(max(d, vec2(0.0))) + min(max(d.x, d.y), 0.0) - radiusPixels;
}

void main() {
    // `viewportSize` is `width / windowScaling, height / windowScaling` --
    // the same formula the screen frame shader carries, because the two are
    // drop-in occupants of one slot. The item is logical-sized; this
    // framebuffer is the render target *times* `windowScaling` (the preset's
    // `scale3`) and the render target is physical pixels, so the divisor
    // that puts the two on one ruler is `windowScaling` times the device
    // pixel ratio. The host pushes that product through this uniform
    // (`crt::params::Params::build`), which is why the division below lands
    // on the same ruler at any DPR. Every pixel count below is stated at
    // that ruler: `screenRadius`, the bezel margins, `outerRadius`, the
    // well.
    vec2 viewportSize = params.OutputSize.xy / max(params.windowScaling, 0.01);
    vec2 lightDir = vec2(params.lightDirX, params.lightDirY);
    vec4 bezelColor = vec4(params.bezelColorR, params.bezelColorG, params.bezelColorB, params.bezelColorA);
    vec4 chassisColor = vec4(params.chassisColorR, params.chassisColorG, params.chassisColorB, params.chassisColorA);
    vec4 ridgeColor = vec4(params.ridgeColorR, params.ridgeColorG, params.ridgeColorB, params.ridgeColorA);
    vec4 bezelMargins = vec4(params.bezelMarginsX, params.bezelMarginsY, params.bezelMarginsZ, params.bezelMarginsW);

    vec2 staticCoords = vTexCoord;
    vec2 coords = distortCoordinates(staticCoords);
    vec2 px = staticCoords * viewportSize;

    float distPixels = roundedRectSdfPixels(coords, vec2(0.0), vec2(1.0), params.screenRadius, viewportSize);
    float dOuter = rrectPx(px, bezelMargins.xy, viewportSize - bezelMargins.zw, params.outerRadius);

    vec2 cc = coords - vec2(0.5);
    vec2 nrm = normalize(cc + vec2(1e-5));
    vec2 L = normalize(lightDir);

    vec2 lightPos = clamp(vec2(0.5) + L * 0.45, 0.0, 1.0);
    float away = clamp(dot(staticCoords - lightPos, -L), 0.0, 2.0);
    float vig = 1.0 - params.vignetteStrength * smoothstep(0.0, 1.1, away);
    float cornerD = length((staticCoords - 0.5) * 2.0) / 1.414;
    vig *= 1.0 - params.vignetteStrength * 0.55 * smoothstep(0.62, 1.0, cornerD);

    float surf = metalField(px, 3.1);

    float band = 0.55 + params.fillGain * clamp(dot(nrm, L), 0.0, 1.0);
    vec3 bez = bezelColor.rgb * band * surf;
    float well = mix(1.0, params.wellFloor, smoothstep(params.wellDepth, 0.0, distPixels));
    if (params.faceBandPx > 0.5) {
        float wall = mix(1.0, params.wellFloor,
                         smoothstep(params.faceBandPx * 0.5, params.faceBandPx * 1.8, -dOuter));
        well = min(well, wall);
    }
    bez *= well;

    float trough = smoothstep(params.wellDepth * 0.35, params.wellDepth * 0.9, distPixels)
                 * (1.0 - smoothstep(params.wellDepth * 0.9, params.wellDepth * 1.7, distPixels));
    bez += bezelColor.rgb * surf * trough * params.troughGain
         * (0.4 + 0.9 * clamp(-dot(nrm, L), 0.0, 1.0));

    bool standingRim = params.rimGain > 0.001;
    float facing = clamp(-dot(nrm, L), 0.0, 1.0);
    float rimD = standingRim ? abs(-dOuter - params.rimDistPx) : abs(distPixels - 2.0);
    float rimW = standingRim ? 3.0 : 2.5;
    float lip = (1.0 - smoothstep(0.0, rimW, rimD))
              * (standingRim ? 0.45 + 0.55 * facing : facing);
    bez += ridgeColor.rgb * lip * (standingRim ? params.rimGain : 0.4 * params.ridgeGain);

    vec3 cha = chassisColor.rgb * surf;

    float inBezel = smoothstep(1.0, -1.0, dOuter);
    vec3 metal = mix(cha, bez, inBezel);

    float groove = 1.0 - smoothstep(0.0, 2.2, abs(dOuter - 1.8));
    metal *= 1.0 - 0.6 * groove;
    float ridgeLine = 1.0 - smoothstep(0.5, 2.4, abs(dOuter + 2.0));
    float ridgeLit = 0.45 + 0.55 * clamp(dot(nrm, L), 0.0, 1.0);
    metal += ridgeColor.rgb * ridgeLine * ridgeLit * params.ridgeGain;

    metal *= vig;

    float inScreen = smoothstep(0.0, 1.0, -distPixels);
    float frameAlpha = 1.0 - params.frameShininess * 0.4;
    float alpha = mix(frameAlpha, mix(0.0, 0.3, params.ambientLight), inScreen);
    float glass = clamp(params.ambientLight * pow(prod2(coords * (1.0 - coords.yx)) * 25.0, 0.5) * inScreen, 0.0, 1.0);
    vec3 color = mix(metal, vec3(glass), inScreen);

    FragColor = vec4(color, alpha);
}
