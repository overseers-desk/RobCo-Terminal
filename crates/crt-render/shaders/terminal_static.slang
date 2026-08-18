#version 450

// Terminal static pass.
//
// "Static" names the half of the screen effect that does not move: screen
// curvature, the RGB shift, the bloom composite, the frame's specular
// reflection, and the overall brightness. Everything that changes frame to
// frame (noise, jitter, flicker, horizontal sync, burn-in) is the *dynamic*
// pass, which runs after this one and samples its output.
//
// Two notes, neither visual:
//
//   1. Selecting one of sixteen precompiled variants by preprocessor
//      (CRT_RGB_SHIFT / CRT_BLOOM / CRT_FRAME_SHININESS / CRT_CURVATURE) is
//      collapsed to runtime uniforms per R0(d): a parameter write costs 83 ns
//      against ~10 ms for a chain load, so every axis is a uniform here and
//      the branches are `if` on the same values that would have picked a
//      file.
//   2. The terminal image is `Original` rather than `Source` in a
//      librashader chain. `Source` at this point in the chain is the pass
//      before this one, which is not what this shader wants.

// Parameters live in the UBO rather than the push block, as in the dynamic
// pass: librashader's reflector caps the push range at 128 bytes whatever the
// device supports, and keeping both terminal passes on the same side of that
// line means adding a uniform to either is the same edit.
layout(push_constant) uniform Push {
    vec4 SourceSize;
    vec4 OriginalSize;
    vec4 OutputSize;
    uint FrameCount;
} params_push;

layout(std140, set = 0, binding = 0) uniform UBO {
    mat4 MVP;

    float ScreenCurvature;
    float RgbShift;
    float FrameShininess;
    float FrameSize;
    float ScreenBrightness;
    float Bloom;
} global;

// The ranges mirror the application settings, except where the setting is
// multiplied before handing the value to the shader (`bloom * 2.5`,
// `lint(0.5, 1.5, brightness)`); those products are computed on the CPU in
// `params.rs`, so the range here is the product's, not the setting's.
#pragma parameter ScreenCurvature "Screen curvature" 0.0 0.0 1.0 0.01
#pragma parameter RgbShift "RGB shift" 0.0 0.0 1.0 0.001
#pragma parameter FrameShininess "Frame shininess" 0.0 0.0 1.0 0.01
#pragma parameter FrameSize "Frame size" 0.0 0.0 1.0 0.01
#pragma parameter ScreenBrightness "Screen brightness" 1.0 0.0 4.0 0.01
#pragma parameter Bloom "Bloom" 0.0 0.0 5.0 0.01

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

// `Original` is the terminal image. Binding 1 is left free for
// the reflected push-constant buffer, which librashader lowers to a UBO.
layout(set = 0, binding = 2) uniform sampler2D Original;
// The bloom pass's output. Its alpha is the bloom mask.
// B3 replaces the pass body; the sampler name and its meaning are fixed here.
layout(set = 0, binding = 3) uniform sampler2D BloomSource;

float min2(vec2 v) { return min(v.x, v.y); }
float max2(vec2 v) { return max(v.x, v.y); }
float rand2(vec2 v) { return fract(sin(dot(v, vec2(12.9898, 78.233))) * 43758.5453); }

vec2 distortCoordinates(vec2 coords) {
    vec2 paddedCoords = coords * (vec2(1.0) + global.FrameSize * 2.0) - global.FrameSize;
    vec2 cc = (paddedCoords - vec2(0.5));
    float dist = dot(cc, cc) * global.ScreenCurvature;
    return (paddedCoords + cc * (1.0 + dist) * dist);
}

void main() {
    float shownDraw = 1.0;
    float isReflection = 0.0;
    float isScreen = 1.0;

    vec2 txt_coords = vTexCoord;

    // The curvature block only matters when `screenCurvature > 0 ||
    // frameSize > 0`; the same test at runtime, and with both at zero
    // `distortCoordinates` is the identity, so the branch is only a saving.
    if (global.ScreenCurvature > 0.0 || global.FrameSize > 0.0) {
        vec2 curvatureCoords = distortCoordinates(vTexCoord);
        shownDraw = max2(step(vec2(0.0), curvatureCoords) - step(vec2(1.0), curvatureCoords));
        isScreen = min2(step(vec2(0.0), curvatureCoords) - step(vec2(1.0), curvatureCoords));
        isReflection = shownDraw - isScreen;
        txt_coords = curvatureCoords * (-1.0 + 2.0 * step(vec2(0.0), curvatureCoords)
                                             - 2.0 * step(vec2(1.0), curvatureCoords));
    }

    vec3 txt_color = texture(Original, txt_coords).rgb;

    if (global.RgbShift > 0.0) {
        vec2 displacement = vec2(global.RgbShift, 0.0);
        vec3 rightColor = texture(Original, txt_coords + displacement).rgb;
        vec3 leftColor = texture(Original, txt_coords - displacement).rgb;
        txt_color.r = leftColor.r * 0.10 + rightColor.r * 0.30 + txt_color.r * 0.60;
        txt_color.g = leftColor.g * 0.20 + rightColor.g * 0.20 + txt_color.g * 0.60;
        txt_color.b = leftColor.b * 0.30 + rightColor.b * 0.10 + txt_color.b * 0.60;
    }

    vec3 finalColor = txt_color * shownDraw;

    vec4 bloomFullColor = texture(BloomSource, txt_coords);
    vec3 bloomColor = bloomFullColor.rgb;
    float bloomAlpha = bloomFullColor.a;

    if (global.Bloom > 0.0) {
        vec3 bloomOnScreen = bloomColor * isScreen;
        finalColor += clamp(bloomOnScreen * global.Bloom * bloomAlpha, 0.0, 0.5);
        float bloomScale = 1.0 + max(global.Bloom, 0.0);
        finalColor /= bloomScale;
    }

    if (global.FrameShininess > 0.0) {
        vec3 reflectionColor = mix(bloomColor * bloomAlpha * 2.0, finalColor,
                                   global.FrameShininess * 0.5);
        finalColor = mix(finalColor, reflectionColor, isReflection);
    }

    finalColor *= global.ScreenBrightness;

    // Fixed-amplitude dither, applied unconditionally, and it is what
    // `xtask mask` exists to blank out when it measures the chrome.
    float noise = rand2(vTexCoord) - 0.5;
    finalColor = clamp(finalColor + vec3(noise * 0.025), 0.0, 1.0);

    FragColor = vec4(finalColor, 1.0);
}
