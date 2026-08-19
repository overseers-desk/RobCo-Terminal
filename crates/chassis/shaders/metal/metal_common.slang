// Shared metal-surface math for the three metal shaders in this directory,
// pulled in with `#include "metal_common.slang"` after `#pragma stage
// fragment` (librashader resolves includes relative to the including file;
// an include carries no #version line). Each includer's ONE-UBO preamble
// declares `params` with `grainAmount`/`mottleAmount`/`scratchAmount`,
// which these read. The CPU reference implementations live in the
// shader-oracle test crate.

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

// `speckSeed` is the one point the three metals differ on: the plate hashes
// its specks from a different lattice offset so its dust does not repeat
// the casting's.
float metalField(vec2 px, float speckSeed) {
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
    float speck = step(0.9975, hash12(floor(px * 0.7) + speckSeed));
    tone += params.scratchAmount * speck * 1.4;
    return max(tone, 0.0);
}

float rrectPx(vec2 p, vec2 topLeft, vec2 bottomRight, float rad) {
    vec2 c = (topLeft + bottomRight) * 0.5;
    vec2 halfSize = (bottomRight - topLeft) * 0.5 - vec2(rad);
    vec2 d = abs(p - c) - halfSize;
    return length(max(d, vec2(0.0))) + min(max(d.x, d.y), 0.0) - rad;
}
