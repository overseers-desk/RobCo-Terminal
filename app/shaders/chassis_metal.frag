#version 440

// The RobCo chassis beside the tube: frame_metal.frag's chassis law read
// outside the frame's own rectangle. Same surface field, same lighting,
// continued through fieldScale/fieldOffset so the seam where the bank column
// meets the frame's left edge has nothing to see. Authored together with
// frame_metal.frag; a change to the surface law lands in both.

layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform ubuf {
    mat4 qt_Matrix;
    float qt_Opacity;
    vec2 viewportSize;
    vec2 fieldScale;
    vec2 fieldOffset;
    vec2 lightDir;
    vec4 chassisColor;
    float grainAmount;
    float mottleAmount;
    float scratchAmount;
    float vignetteStrength;
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

// frame_metal.frag's metalField, verbatim.
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
    float speck = step(0.9975, hash12(floor(px * 0.7) + 3.1));
    tone += scratchAmount * speck * 1.4;
    return max(tone, 0.0);
}

void main() {
    // This pixel's place in the frame's field: inside the frame's rectangle
    // these land in 0..1, the bank column sits at negative x.
    vec2 staticCoords = qt_TexCoord0 * fieldScale + fieldOffset;
    vec2 px = staticCoords * viewportSize;

    vec2 L = normalize(lightDir);
    vec2 lightPos = clamp(vec2(0.5) + L * 0.45, 0.0, 1.0);
    float away = clamp(dot(staticCoords - lightPos, -L), 0.0, 2.0);
    float vig = 1.0 - vignetteStrength * smoothstep(0.0, 1.1, away);
    float cornerD = length((staticCoords - 0.5) * 2.0) / 1.414;
    vig *= 1.0 - vignetteStrength * 0.55 * smoothstep(0.62, 1.0, cornerD);

    vec3 metal = chassisColor.rgb * metalField(px) * vig;

    fragColor = vec4(metal, 1.0) * qt_Opacity;
}
