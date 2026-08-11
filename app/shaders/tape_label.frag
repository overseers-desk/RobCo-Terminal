#version 440

// A strip of embossed punch-tape label stuck on the chassis metal: the
// near-black glossy plastic of a Dymo tape, rounded ends, and uppercase
// letters raised out of it by the punch. The glyph mask arrives as a
// texture; the emboss is derived from it here, keyed to the same light the
// casting is lit by, so the letters' bright rim and lee shadow agree with
// the chassis. The punch fattens a stroke as it stretches the plastic, so
// the mask is dilated before it is lit. Everything is sized in the strip's
// own pixels; no mock dimension survives in here.

layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform ubuf {
    mat4 qt_Matrix;
    float qt_Opacity;
    vec2 sizePx;
    vec2 lightDir;
    vec4 tapeColor;
    vec4 letterColor;
    vec4 glyphRectPx; // xy: origin of the glyph raster on the strip; zw: its size
    float bevelPx;
    float dilatePx;
    float sheenAmount;
    float grainAmount;
    float seed;
};

layout(binding = 1) uniform sampler2D glyphMask;

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

// The glyph mask at a point on the strip, zero off the raster's rect.
float maskAt(vec2 px) {
    vec2 guv = (px - glyphRectPx.xy) / glyphRectPx.zw;
    if (guv.x < 0.0 || guv.x > 1.0 || guv.y < 0.0 || guv.y > 1.0) {
        return 0.0;
    }
    return texture(glyphMask, guv).a;
}

// The mask swollen by the punch: the widest ink within dilatePx.
float punched(vec2 px) {
    float m = maskAt(px);
    float r = dilatePx;
    m = max(m, maskAt(px + vec2(r, 0.0)));
    m = max(m, maskAt(px + vec2(-r, 0.0)));
    m = max(m, maskAt(px + vec2(0.0, r)));
    m = max(m, maskAt(px + vec2(0.0, -r)));
    float rd = r * 0.7071;
    m = max(m, maskAt(px + vec2(rd, rd)));
    m = max(m, maskAt(px + vec2(rd, -rd)));
    m = max(m, maskAt(px + vec2(-rd, rd)));
    m = max(m, maskAt(px + vec2(-rd, -rd)));
    return m;
}

void main() {
    vec2 px = qt_TexCoord0 * sizePx;
    vec2 L = normalize(lightDir);
    // In item coordinates y grows downward, and lightDir is given with y
    // negative meaning the light stands high; toward the light on the strip
    // is +L.x, -(-L.y) -- L works unchanged, matching the plate's bevel law.

    // The strip's cast shadow on the metal: the same rounded shape, pushed
    // away from the light, soft, present only past the tape's own edge.
    float inset = 1.5;
    float rad = sizePx.y * 0.24;
    float dTape = rrectPx(px, vec2(inset), sizePx - vec2(inset), rad);
    float body = 1.0 - smoothstep(-0.8, 0.8, dTape);
    float dShadow = rrectPx(px + L * 2.2, vec2(inset), sizePx - vec2(inset), rad);
    float shadow = (1.0 - smoothstep(-1.0, 3.0, dShadow)) * (1.0 - body);

    // The plastic ground: near-black with the faintest blue holding on, a
    // whisper of grain, and the gloss band a curved glossy strip throws.
    vec3 col = tapeColor.rgb;
    float grain = hash12(px + vec2(seed * 91.0)) - 0.5;
    col *= 1.0 + grainAmount * grain;
    float v = px.y / sizePx.y;
    float band = exp(-pow((v - 0.28) / 0.16, 2.0));
    // The gloss stays on the strip's face: run it to the cut and it draws a
    // bright ring inside the rounded ends that reads as a badge's bevel.
    float faceMask = smoothstep(1.0, 4.0, -dTape);
    col += vec3(1.0) * sheenAmount * 0.05 * band * faceMask;

    // The cut edges: a hair of darkening all round, and the edge facing the
    // light catching a thin bright line where the gloss turns over.
    float edge = 1.0 - smoothstep(0.0, 2.0, -dTape);
    col *= 1.0 - 0.35 * edge;
    float bt = 1.0 - smoothstep(0.0, 1.8, px.y);
    float bb = 1.0 - smoothstep(0.0, 1.8, sizePx.y - px.y);
    float rimLit = bt * clamp(-L.y, 0.0, 1.0) + bb * clamp(L.y, 0.0, 1.0);
    col += vec3(0.30, 0.30, 0.34) * rimLit * body;

    // The letters. The raised face is the whitened stretched plastic; its
    // light-facing rim takes the hard highlight, the lee side falls off
    // soft into shadow, and just past the lee edge the letter shades the
    // tape it stands on.
    float mC = punched(px);
    float mToward = punched(px + L * bevelPx);
    float mAway = punched(px - L * bevelPx);
    float mAwayFar = punched(px - L * bevelPx * 1.8);

    float rimHi = clamp(mC - mToward, 0.0, 1.0);
    float rimSh = clamp(mC - mAway, 0.0, 1.0) * 0.7
                + clamp(mC - mAwayFar, 0.0, 1.0) * 0.3;
    float drop = clamp(mToward - mC, 0.0, 1.0);

    float faceGrain = hash12(px * 1.7 + vec2(seed * 47.0)) - 0.5;
    vec3 face = letterColor.rgb * (0.74 + 0.10 * faceGrain);
    col = mix(col, face, mC);
    col = mix(col, tapeColor.rgb * 0.55, clamp(rimSh, 0.0, 1.0) * 0.8);
    col += (vec3(1.0) - 0.05 * tapeColor.rgb) * rimHi * 0.85;
    col = mix(col, tapeColor.rgb * 0.25, drop * 0.55);

    // Gloss crosses the letters too: one lacquer over the whole strip.
    col += vec3(1.0) * sheenAmount * 0.03 * band * mC;

    float shadowAlpha = shadow * 0.40;
    float alpha = clamp(body + shadowAlpha, 0.0, 1.0);
    fragColor = vec4(col * body, alpha) * qt_Opacity;
}
