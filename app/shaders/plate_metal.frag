#version 440

// A piece of aged cast metal cut to a rounded rectangle: the amber shell's
// bank plate, the blue shell's rail body, any plaque that needs the same
// skin. Multi-octave mottling and grain, noise-gated edge wear, a bevel
// highlight/shadow pair keyed to the light direction, a seam line inside the
// raised outer moulding, and a directional vignette. Everything is sized in
// the piece's own pixels, so no mock dimension survives in here.

layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform ubuf {
    mat4 qt_Matrix;
    float qt_Opacity;
    vec2 sizePx;
    vec2 lightDir;
    vec4 baseColor;
    vec4 highlightColor;
    vec4 shadowColor;
    float cornerRadius;
    float bevelPx;
    float grainAmount;
    float mottleAmount;
    float scratchAmount;
    float vignetteStrength;
    float wearAmount;
    float seamGain;
    float seed;
};

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
    float tone = 1.0 + mottleAmount * (0.9 * (m - 0.5) + 0.5 * (m2 - 0.5));
    float stain = smoothstep(0.60, 0.85, fbm(px * 0.004 + 7.7));
    tone -= mottleAmount * 0.35 * stain;
    float g = (vnoise(px * 0.9) - 0.5) + 0.6 * (hash12(px) - 0.5);
    tone += grainAmount * g;
    float streak = vnoise(vec2(px.x * 0.012, px.y * 0.4));
    tone += scratchAmount * 0.3 * (streak - 0.5);
    float fine = vnoise(vec2(px.x * 0.5, px.y * 0.05) + 3.7);
    tone += scratchAmount * 0.22 * (fine - 0.5);
    float pit = smoothstep(0.78, 0.95, vnoise(px * 0.09 + 13.1));
    tone -= mottleAmount * 0.3 * pit;
    float speck = step(0.9975, hash12(floor(px * 0.7) + 9.2));
    tone += scratchAmount * speck * 1.4;
    return max(tone, 0.0);
}

float rrectPx(vec2 p, vec2 topLeft, vec2 bottomRight, float rad) {
    vec2 c = (topLeft + bottomRight) * 0.5;
    vec2 halfSize = (bottomRight - topLeft) * 0.5 - vec2(rad);
    vec2 d = abs(p - c) - halfSize;
    return length(max(d, vec2(0.0))) + min(max(d.x, d.y), 0.0) - rad;
}

void main() {
    vec2 px = qt_TexCoord0 * sizePx;
    float d = rrectPx(px, vec2(0.5), sizePx - vec2(0.5), cornerRadius);
    float coverage = 1.0 - smoothstep(-1.0, 0.5, d);

    vec2 uvn = qt_TexCoord0;
    vec2 nrm = normalize(px - sizePx * 0.5 + vec2(1e-5));
    vec2 L = normalize(lightDir);

    float surf = metalField(px + vec2(seed * 137.0, seed * 61.0));
    vec3 col = baseColor.rgb * surf;

    // Patina blotches: broad patches where the finish has gone toward bare
    // bright metal, the aged-cast look the mocks carry.
    float blotch = smoothstep(0.60, 0.92, fbm(px * 0.008 + vec2(seed * 31.0)));
    col = mix(col, highlightColor.rgb * 0.42, wearAmount * 0.55 * blotch);

    // Edge wear: the paintless bright metal where hands and years rubbed the
    // moulding, gated by its own noise so it comes and goes along the edge.
    float edgeDist = -d;
    float nearEdge = smoothstep(bevelPx + 6.0, 0.0, edgeDist);
    float wearNoise = fbm(px * 0.03 + vec2(seed * 17.0));
    col = mix(col, highlightColor.rgb * (0.45 + 0.8 * wearNoise),
              wearAmount * nearEdge * smoothstep(0.35, 0.8, wearNoise));

    // The bevel pair, taken edge by edge: each face is lit by how squarely
    // it turns toward the key, so the top edge carries the full highlight
    // and the lee edges drop into their cut shadow.
    float bt = 1.0 - smoothstep(0.0, bevelPx, px.y);
    float bl = 1.0 - smoothstep(0.0, bevelPx, px.x);
    float br = 1.0 - smoothstep(0.0, bevelPx, sizePx.x - px.x);
    float bb = 1.0 - smoothstep(0.0, bevelPx, sizePx.y - px.y);
    float lit = bt * clamp(-L.y, 0.0, 1.0) + bl * clamp(-L.x, 0.0, 1.0)
              + br * clamp(L.x, 0.0, 1.0) + bb * clamp(L.y, 0.0, 1.0);
    float shad = bt * clamp(L.y, 0.0, 1.0) + bl * clamp(L.x, 0.0, 1.0)
               + br * clamp(-L.x, 0.0, 1.0) + bb * clamp(-L.y, 0.0, 1.0);
    col += highlightColor.rgb * clamp(lit, 0.0, 1.0) * 0.9;
    col = mix(col, shadowColor.rgb, clamp(shad, 0.0, 1.0) * 0.7);

    // The seam where the raised outer moulding meets the plate's field.
    float seam = 1.0 - smoothstep(0.8, 2.8, abs(edgeDist - (bevelPx + 2.0)));
    col *= 1.0 - 0.35 * seamGain * seam;

    // Low-key light: falls away from the key, pools dark in the far corners.
    vec2 lightPos = clamp(vec2(0.5) + L * 0.5, 0.0, 1.0);
    float away = clamp(dot(uvn - lightPos, -L), 0.0, 2.0);
    float vig = 1.0 - vignetteStrength * smoothstep(0.0, 1.2, away);
    float cornerD = length((uvn - 0.5) * 2.0) / 1.414;
    vig *= 1.0 - vignetteStrength * 0.5 * smoothstep(0.6, 1.0, cornerD);
    col *= vig;

    fragColor = vec4(col, 1.0) * coverage * qt_Opacity;
}
