#version 450

// Draws the frame that surrounds the curved screen: a rounded-rect SDF, a
// directional shadow along two of its edges, and the glass sheen showing
// through where the frame overlaps the screen. No input texture: everything
// here is procedural given the frame/screen geometry and the ambient light.
//
// The frame colour is a four-component colour value; the RetroArch slang
// parameter grammar has no vector parameters, so it is split into four
// scalar #pragma parameter floats and reassembled below.
//
// Every builtin (SourceSize/OriginalSize/OutputSize/FrameCount), MVP, and
// every #pragma parameter lives in ONE `std140` UBO declared here in the
// shared preamble (not `push_constant`, and not a second per-stage block):
// a shader with this many parameters exceeds Vulkan's guaranteed-minimum
// 128-byte push constant range (this one alone needs ~100 bytes; the
// 36-parameter frame_metal.slang needs ~208), and librashader's reflection
// rejects more than one uniform buffer per stage, which a block declared
// after `#pragma stage fragment` becomes once combined with this preamble's
// own block. One shared UBO, referenced by both stages, avoids both traps.

#pragma parameter screenCurvature "Screen curvature" 0.30 0.0 1.0 0.01
#pragma parameter frameColorR "Frame color R" 0.15 0.0 1.0 0.01
#pragma parameter frameColorG "Frame color G" 0.15 0.0 1.0 0.01
#pragma parameter frameColorB "Frame color B" 0.15 0.0 1.0 0.01
#pragma parameter frameColorA "Frame color A" 1.00 0.0 1.0 0.01
#pragma parameter frameSize "Frame size" 0.05 0.0 1.0 0.001
#pragma parameter screenRadius "Screen radius (px)" 8.0 0.0 128.0 1.0
#pragma parameter ambientLight "Ambient light" 0.3 0.0 1.0 0.01
#pragma parameter frameShininess "Frame shininess" 0.0 0.0 1.0 0.01
// The host pushes `windowScaling * devicePixelRatio` here (see main()), so the
// range runs past any window scaling a settings file holds: 4x scaling on a 4x
// display is 16. The parameter carries windowScaling times the DPR.
#pragma parameter windowScaling "Framebuffer-to-item ruler (windowScaling * DPR)" 1.0 0.01 16.0 0.01

layout(std140, set = 0, binding = 0) uniform UBO {
    mat4 MVP;
    vec4 SourceSize;
    vec4 OriginalSize;
    vec4 OutputSize;
    uint FrameCount;
    float screenCurvature;
    float frameColorR;
    float frameColorG;
    float frameColorB;
    float frameColorA;
    float frameSize;
    float screenRadius;
    float ambientLight;
    float frameShininess;
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
layout(location = 0) in vec2 vTexCoord;
layout(location = 0) out vec4 FragColor;

float min2(vec2 v) { return min(v.x, v.y); }
float prod2(vec2 v) { return v.x * v.y; }
float rand2(vec2 v) { return fract(sin(dot(v, vec2(12.9898, 78.233))) * 43758.5453); }

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
    // `viewportSize` is `width / windowScaling, height / windowScaling`. The
    // item is sized in logical pixels; this shader's framebuffer is the
    // render target *times* `windowScaling` (the preset's `scale3`), and a
    // render target is physical pixels. So the divisor that puts the two on
    // one ruler is `windowScaling` times the device pixel ratio, and that
    // product is what the host pushes through this uniform
    // (`crt::params::Params::build`, at the `windowScaling` entry).
    //
    // It matters because everything measured against `viewportSize` is a
    // pixel count stated at that ruler: `screenRadius` is the corner radius
    // in those pixels, and `seamWidth` is derived from it. Divide by the
    // window scaling alone and a 2x display gets half the corner it should.
    vec2 viewportSize = params.OutputSize.xy / max(params.windowScaling, 0.01);
    vec4 frameColor = vec4(params.frameColorR, params.frameColorG, params.frameColorB, params.frameColorA);

    vec2 staticCoords = vTexCoord;
    vec2 coords = distortCoordinates(staticCoords);

    float screenRadiusPixels = params.screenRadius;
    float edgeSoftPixels = 1.0;

    float seamWidth = max(screenRadiusPixels, 0.5) / min2(viewportSize);

    float e = min(
        smoothstep(-seamWidth, seamWidth, coords.x - coords.y),
        smoothstep(-seamWidth, seamWidth, coords.x - (1.0 - coords.y))
    );
    float s = min(
        smoothstep(-seamWidth, seamWidth, coords.y - coords.x),
        smoothstep(-seamWidth, seamWidth, coords.x - (1.0 - coords.y))
    );
    float w = min(
        smoothstep(-seamWidth, seamWidth, coords.y - coords.x),
        smoothstep(-seamWidth, seamWidth, (1.0 - coords.x) - coords.y)
    );
    float n = min(
        smoothstep(-seamWidth, seamWidth, coords.x - coords.y),
        smoothstep(-seamWidth, seamWidth, (1.0 - coords.x) - coords.y)
    );

    float distPixels = roundedRectSdfPixels(coords, vec2(0.0), vec2(1.0), screenRadiusPixels, viewportSize);
    float frameShadow = (e * 0.66 + w * 0.66 + n * 0.33 + s);
    frameShadow *= smoothstep(0.0, edgeSoftPixels * 5.0, distPixels);

    float frameAlpha = 1.0 - params.frameShininess * 0.4;

    float inScreen = smoothstep(0.0, edgeSoftPixels, -distPixels);
    float alpha = mix(frameAlpha, mix(0.0, 0.3, params.ambientLight), inScreen);
    float glass = clamp(params.ambientLight * pow(prod2(coords * (1.0 - coords.yx)) * 25.0, 0.5) * inScreen, 0.0, 1.0);
    vec3 frameTint = frameColor.rgb * frameShadow;
    float noise = rand2(staticCoords * viewportSize) - 0.5;
    frameTint = clamp(frameTint + vec3(noise * 0.04), 0.0, 1.0);
    vec3 color = mix(frameTint, vec3(glass), inScreen);

    FragColor = vec4(color, alpha);
}
