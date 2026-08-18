#version 450

// Samples `Source` as the glyph mask raster. Vector uniforms split into
// scalar #pragma parameter floats; `sizePx` is the strip's own pixel size,
// same reasoning as plate_metal.slang.
//
// MVP, builtins, and parameters all live in ONE UBO declared in the shared
// preamble -- see terminal_frame.slang's header for why.

#pragma parameter sizePxX "Strip width (px)" 128.0 1.0 4096.0 1.0
#pragma parameter sizePxY "Strip height (px)" 24.0 1.0 4096.0 1.0
#pragma parameter lightDirX "Light direction X" 0.35 -1.0 1.0 0.01
#pragma parameter lightDirY "Light direction Y" -0.6 -1.0 1.0 0.01
#pragma parameter tapeColorR "Tape color R" 0.03 0.0 1.0 0.01
#pragma parameter tapeColorG "Tape color G" 0.03 0.0 1.0 0.01
#pragma parameter tapeColorB "Tape color B" 0.05 0.0 1.0 0.01
#pragma parameter tapeColorA "Tape color A" 1.00 0.0 1.0 0.01
#pragma parameter letterColorR "Letter color R" 0.92 0.0 1.0 0.01
#pragma parameter letterColorG "Letter color G" 0.92 0.0 1.0 0.01
#pragma parameter letterColorB "Letter color B" 0.90 0.0 1.0 0.01
#pragma parameter letterColorA "Letter color A" 1.00 0.0 1.0 0.01
#pragma parameter glyphRectPxX "Glyph rect origin X (px)" 4.0 0.0 4096.0 1.0
#pragma parameter glyphRectPxY "Glyph rect origin Y (px)" 4.0 0.0 4096.0 1.0
#pragma parameter glyphRectPxZ "Glyph rect width (px)" 120.0 1.0 4096.0 1.0
#pragma parameter glyphRectPxW "Glyph rect height (px)" 16.0 1.0 4096.0 1.0
#pragma parameter bevelPx "Bevel width (px)" 1.5 0.0 32.0 0.1
#pragma parameter dilatePx "Dilate (px)" 0.8 0.0 8.0 0.1
#pragma parameter sheenAmount "Sheen amount" 0.6 0.0 1.0 0.01
#pragma parameter grainAmount "Grain amount" 0.05 0.0 1.0 0.01
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
    float tapeColorR;
    float tapeColorG;
    float tapeColorB;
    float tapeColorA;
    float letterColorR;
    float letterColorG;
    float letterColorB;
    float letterColorA;
    float glyphRectPxX;
    float glyphRectPxY;
    float glyphRectPxZ;
    float glyphRectPxW;
    float bevelPx;
    float dilatePx;
    float sheenAmount;
    float grainAmount;
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
layout(location = 0) in vec2 vTexCoord;
layout(location = 0) out vec4 FragColor;
layout(set = 0, binding = 2) uniform sampler2D Source;

float hash12(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

float rrectPx(vec2 p, vec2 topLeft, vec2 bottomRight, float rad) {
    vec2 c = (topLeft + bottomRight) * 0.5;
    vec2 halfSize = (bottomRight - topLeft) * 0.5 - vec2(rad);
    vec2 d = abs(p - c) - halfSize;
    return length(max(d, vec2(0.0))) + min(max(d.x, d.y), 0.0) - rad;
}

float maskAt(vec2 px, vec4 glyphRectPx) {
    vec2 guv = (px - glyphRectPx.xy) / glyphRectPx.zw;
    if (guv.x < 0.0 || guv.x > 1.0 || guv.y < 0.0 || guv.y > 1.0) {
        return 0.0;
    }
    return texture(Source, guv).a;
}

float punched(vec2 px, vec4 glyphRectPx) {
    float m = maskAt(px, glyphRectPx);
    float r = params.dilatePx;
    m = max(m, maskAt(px + vec2(r, 0.0), glyphRectPx));
    m = max(m, maskAt(px + vec2(-r, 0.0), glyphRectPx));
    m = max(m, maskAt(px + vec2(0.0, r), glyphRectPx));
    m = max(m, maskAt(px + vec2(0.0, -r), glyphRectPx));
    float rd = r * 0.7071;
    m = max(m, maskAt(px + vec2(rd, rd), glyphRectPx));
    m = max(m, maskAt(px + vec2(rd, -rd), glyphRectPx));
    m = max(m, maskAt(px + vec2(-rd, rd), glyphRectPx));
    m = max(m, maskAt(px + vec2(-rd, -rd), glyphRectPx));
    return m;
}

void main() {
    vec2 sizePx = vec2(params.sizePxX, params.sizePxY);
    vec2 lightDir = vec2(params.lightDirX, params.lightDirY);
    vec4 tapeColor = vec4(params.tapeColorR, params.tapeColorG, params.tapeColorB, params.tapeColorA);
    vec4 letterColor = vec4(params.letterColorR, params.letterColorG, params.letterColorB, params.letterColorA);
    vec4 glyphRectPx = vec4(params.glyphRectPxX, params.glyphRectPxY, params.glyphRectPxZ, params.glyphRectPxW);

    vec2 px = vTexCoord * sizePx;
    vec2 L = normalize(lightDir);

    float inset = 1.5;
    float rad = sizePx.y * 0.24;
    float dTape = rrectPx(px, vec2(inset), sizePx - vec2(inset), rad);
    float body = 1.0 - smoothstep(-0.8, 0.8, dTape);
    float dShadow = rrectPx(px + L * 2.2, vec2(inset), sizePx - vec2(inset), rad);
    float shadow = (1.0 - smoothstep(-1.0, 3.0, dShadow)) * (1.0 - body);

    vec3 col = tapeColor.rgb;
    float grain = hash12(px + vec2(params.seed * 91.0)) - 0.5;
    col *= 1.0 + params.grainAmount * grain;
    float v = px.y / sizePx.y;
    float band = exp(-pow((v - 0.28) / 0.16, 2.0));
    float faceMask = smoothstep(1.0, 4.0, -dTape);
    col += vec3(1.0) * params.sheenAmount * 0.05 * band * faceMask;

    float edge = 1.0 - smoothstep(0.0, 2.0, -dTape);
    col *= 1.0 - 0.35 * edge;
    float bt = 1.0 - smoothstep(0.0, 1.8, px.y);
    float bb = 1.0 - smoothstep(0.0, 1.8, sizePx.y - px.y);
    float rimLit = bt * clamp(-L.y, 0.0, 1.0) + bb * clamp(L.y, 0.0, 1.0);
    col += vec3(0.30, 0.30, 0.34) * rimLit * body;

    float mC = punched(px, glyphRectPx);
    float mToward = punched(px + L * params.bevelPx, glyphRectPx);
    float mAway = punched(px - L * params.bevelPx, glyphRectPx);
    float mAwayFar = punched(px - L * params.bevelPx * 1.8, glyphRectPx);

    float rimHi = clamp(mC - mToward, 0.0, 1.0);
    float rimSh = clamp(mC - mAway, 0.0, 1.0) * 0.7
                + clamp(mC - mAwayFar, 0.0, 1.0) * 0.3;
    float drop = clamp(mToward - mC, 0.0, 1.0);

    float faceGrain = hash12(px * 1.7 + vec2(params.seed * 47.0)) - 0.5;
    vec3 face = letterColor.rgb * (0.74 + 0.10 * faceGrain);
    col = mix(col, face, mC);
    col = mix(col, tapeColor.rgb * 0.55, clamp(rimSh, 0.0, 1.0) * 0.8);
    col += (vec3(1.0) - 0.05 * tapeColor.rgb) * rimHi * 0.85;
    col = mix(col, tapeColor.rgb * 0.25, drop * 0.55);

    col += vec3(1.0) * params.sheenAmount * 0.03 * band * mC;

    float shadowAlpha = shadow * 0.40;
    float alpha = clamp(body + shadowAlpha, 0.0, 1.0);
    FragColor = vec4(col * body, alpha);
}
