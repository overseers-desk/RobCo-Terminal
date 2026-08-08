#version 440

// The RobCo appliances' bezel and the chassis margin around it: aged gunmetal
// built from noise octaves instead of a texture. One shader serves both
// shells; each sets its own colours, light direction, well depth and wear
// amounts. Authored together with chassis_metal.frag: the same surface law
// and the same field coordinates, so the bank column and the CRT frame read
// as one casting.

layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform ubuf {
    mat4 qt_Matrix;
    float qt_Opacity;
    float screenCurvature;
    float frameSize;
    float screenRadius;
    float ambientLight;
    float frameShininess;
    vec2 viewportSize;
    vec2 lightDir;
    vec4 bezelColor;
    vec4 chassisColor;
    vec4 ridgeColor;
    vec4 bezelMargins;
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
};

float min2(vec2 v) { return min(v.x, v.y); }
float prod2(vec2 v) { return v.x * v.y; }

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

// The aged-metal tone at a pixel: broad mottling, finer patina, grime pooling
// darker in blotches, film grain, horizontal wear streaks, rare bright
// scratch specks that flash to near-white.
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

vec2 distortCoordinates(vec2 coords) {
    vec2 paddedCoords = coords * (vec2(1.0) + frameSize * 2.0) - frameSize;
    vec2 cc = (paddedCoords - vec2(0.5));
    float dist = dot(cc, cc) * screenCurvature;
    return (paddedCoords + cc * (1.0 + dist) * dist);
}

float roundedRectSdfPixels(vec2 p, vec2 topLeft, vec2 bottomRight, float radiusPixels) {
    vec2 sizePixels = (bottomRight - topLeft) * viewportSize;
    vec2 centerPixels = (topLeft + bottomRight) * 0.5 * viewportSize;
    vec2 localPixels = p * viewportSize - centerPixels;
    vec2 halfSize = sizePixels * 0.5 - vec2(radiusPixels);
    vec2 d = abs(localPixels) - halfSize;
    return length(max(d, vec2(0.0))) + min(max(d.x, d.y), 0.0) - radiusPixels;
}

float rrectPx(vec2 p, vec2 topLeft, vec2 bottomRight, float rad) {
    vec2 c = (topLeft + bottomRight) * 0.5;
    vec2 halfSize = (bottomRight - topLeft) * 0.5 - vec2(rad);
    vec2 d = abs(p - c) - halfSize;
    return length(max(d, vec2(0.0))) + min(max(d.x, d.y), 0.0) - rad;
}

void main() {
    vec2 staticCoords = qt_TexCoord0;
    vec2 coords = distortCoordinates(staticCoords);
    vec2 px = staticCoords * viewportSize;

    // The glass opening in distorted space; the bezel's outer edge is a flat
    // plate edge, so it lives in static pixels.
    float distPixels = roundedRectSdfPixels(coords, vec2(0.0), vec2(1.0), screenRadius);
    float dOuter = rrectPx(px, bezelMargins.xy, viewportSize - bezelMargins.zw, outerRadius);

    vec2 cc = coords - vec2(0.5);
    vec2 nrm = normalize(cc + vec2(1e-5));
    vec2 L = normalize(lightDir);

    // Low-key room light: falls off away from the key, pools dark in corners.
    vec2 lightPos = clamp(vec2(0.5) + L * 0.45, 0.0, 1.0);
    float away = clamp(dot(staticCoords - lightPos, -L), 0.0, 2.0);
    float vig = 1.0 - vignetteStrength * smoothstep(0.0, 1.1, away);
    float cornerD = length((staticCoords - 0.5) * 2.0) / 1.414;
    vig *= 1.0 - vignetteStrength * 0.55 * smoothstep(0.62, 1.0, cornerD);

    float surf = metalField(px);

    // The bezel face: a directional band (one side catches the room's fill),
    // sinking toward the glass down the well's sloped wall.
    float band = 0.55 + fillGain * clamp(dot(nrm, L), 0.0, 1.0);
    vec3 bez = bezelColor.rgb * band * surf;
    float well = mix(1.0, wellFloor, smoothstep(wellDepth, 0.0, distPixels));
    bez *= well;

    // The trough floor at the well's outer rim: broad metal the room light
    // reaches, brightest on the side facing away from the key (the light
    // lands on the far floor of the recess, not the near wall).
    float trough = smoothstep(wellDepth * 0.35, wellDepth * 0.9, distPixels)
                 * (1.0 - smoothstep(wellDepth * 0.9, wellDepth * 1.7, distPixels));
    bez += bezelColor.rgb * surf * trough * troughGain
         * (0.4 + 0.9 * clamp(-dot(nrm, L), 0.0, 1.0));

    // The lip where the well meets the glass: a thin catch of light on the
    // wall that faces away from the key.
    float lip = (1.0 - smoothstep(0.0, 2.5, abs(distPixels - 2.0)))
              * clamp(-dot(nrm, L), 0.0, 1.0);
    bez += ridgeColor.rgb * lip * 0.4 * ridgeGain;

    // Chassis metal outside the bezel plate.
    vec3 cha = chassisColor.rgb * surf;

    float inBezel = smoothstep(1.0, -1.0, dOuter);
    vec3 metal = mix(cha, bez, inBezel);

    // The seam between chassis and bezel: a dark groove hugging the plate,
    // then the raised moulding's lit ridge just inside the edge.
    float groove = 1.0 - smoothstep(0.0, 2.2, abs(dOuter - 1.8));
    metal *= 1.0 - 0.6 * groove;
    float ridgeLine = 1.0 - smoothstep(0.5, 2.4, abs(dOuter + 2.0));
    float ridgeLit = 0.45 + 0.55 * clamp(dot(nrm, L), 0.0, 1.0);
    metal += ridgeColor.rgb * ridgeLine * ridgeLit * ridgeGain;

    metal *= vig;

    // Over the glass the frame contributes only its reflection, as upstream.
    float inScreen = smoothstep(0.0, 1.0, -distPixels);
    float frameAlpha = 1.0 - frameShininess * 0.4;
    float alpha = mix(frameAlpha, mix(0.0, 0.3, ambientLight), inScreen);
    float glass = clamp(ambientLight * pow(prod2(coords * (1.0 - coords.yx)) * 25.0, 0.5) * inScreen, 0.0, 1.0);
    vec3 color = mix(metal, vec3(glass), inScreen);

    fragColor = vec4(color, alpha) * qt_Opacity;
}
