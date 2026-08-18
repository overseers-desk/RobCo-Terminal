#version 450

// Terminal dynamic pass, and the last pass in the chain.
//
// Everything that changes frame to frame lives here: the noise field, the
// jitter it drives, the glowing scan line, the horizontal-sync tear, the
// flicker, the burn-in composite, the rasterization mask, and the conversion of
// the terminal's greyscale into the profile's two colours. It samples the
// static pass's output as `Source`, so the geometry (curvature, RGB shift,
// bloom) is already baked in by the time this runs.
//
// Four notes on how this pass is built:
//
//   1. The noise texture could be sampled once per vertex to derive the
//      flicker brightness and the sync tear, passing the three results down
//      as varyings. Those values are constant across the fullscreen quad,
//      so they are computed here in the fragment stage instead: same numbers,
//      one shader stage fewer, and no varying contract to keep in step.
//   2. The eight raster/burn-in/frame/chroma variants that a preprocessor
//      could branch on (CRT_RASTER_MODE 0..4 x CRT_BURN_IN x
//      CRT_DISPLAY_FRAME x CRT_CHROMA) are runtime uniforms per R0(d).
//      `RasterMode` is an integer-valued float branched on, exactly as the
//      wave ledger records it.
//   3. The burn-in accumulator and the terminal frame are chain passes here
//      rather than separate offscreen sources: `Burn` (pass 0, B2) and
//      `FrameSource` (pass 2, B3). The tiled noise field is a librashader
//      user texture, `NoiseSource`. The burn-in fade rate is not a uniform of
//      this pass at all: the accumulator is driven by `BURNIN_DECAY_STEP`,
//      which `crt_burnin::DecayClock` computes from the real frame delta, and
//      `BurnIn` survives here only as the switch that decides whether the
//      ghost is composited, a runtime branch.
//   4. The degauss transient is applied here. Elsewhere it could be a
//      transform on the container that holds the glass, outside any shader;
//      but chassis chrome is outside this chain per R0(a), so the finished
//      glass image is the last thing that can carry it. Squeezing the
//      picture in the final pass is the same operation applied one layer
//      down.

// Parameters live in the UBO, not the push block. librashader's reflector caps
// the push range at 128 bytes (`MAX_PUSH_BUFFER_SIZE`) whatever the device
// supports, and it applies the cap per stage, so this pass's thirty-odd
// uniforms overflow it at 176 bytes. The UBO has no such cap, and on the wgpu
// runtime both blocks are lowered to uniform buffers anyway, so nothing is paid
// for the move.
layout(push_constant) uniform Push {
    vec4 SourceSize;
    vec4 OriginalSize;
    vec4 OutputSize;
    uint FrameCount;
} params_push;

layout(std140, set = 0, binding = 0) uniform UBO {
    mat4 MVP;

    float Time;
    float VirtualResolutionX;
    float VirtualResolutionY;
    float RasterizationIntensity;
    float RasterMode;
    float BurnIn;
    float StaticNoise;
    float ScreenCurvature;
    float GlowingLine;
    float ChromaColor;
    float JitterX;
    float JitterY;
    float Jitter;
    float HorizontalSync;
    float HorizontalSyncStrength;
    float Flickering;
    float ScaleNoiseX;
    float ScaleNoiseY;
    float FrameSize;
    float FrameEnabled;
    float Bloom;
    float FontColorR;
    float FontColorG;
    float FontColorB;
    float BackgroundColorR;
    float BackgroundColorG;
    float BackgroundColorB;
    float DegaussBrightness;
    float DegaussScaleY;
    float WindowOpacity;
} global;

#pragma parameter Time "Elapsed wall-clock seconds" 0.0 0.0 100000.0 0.001
#pragma parameter VirtualResolutionX "Virtual resolution, columns in pixels" 640.0 1.0 16384.0 1.0
#pragma parameter VirtualResolutionY "Virtual resolution, rows in pixels" 480.0 1.0 16384.0 1.0
#pragma parameter RasterizationIntensity "Rasterization intensity" 0.0 0.0 1.0 0.01
#pragma parameter RasterMode "Rasterization mode, 0..4" 0.0 0.0 4.0 1.0
#pragma parameter BurnIn "Burn-in strength" 0.0 0.0 1.0 0.01
#pragma parameter StaticNoise "Static noise" 0.0 0.0 1.0 0.01
#pragma parameter ScreenCurvature "Screen curvature" 0.0 0.0 1.0 0.01
#pragma parameter GlowingLine "Glowing line" 0.0 0.0 1.0 0.01
#pragma parameter ChromaColor "Chroma colour" 0.0 0.0 1.0 0.01
#pragma parameter JitterX "Jitter displacement, x" 0.0 0.0 1.0 0.0001
#pragma parameter JitterY "Jitter displacement, y" 0.0 0.0 1.0 0.0001
#pragma parameter Jitter "Jitter" 0.0 0.0 1.0 0.01
#pragma parameter HorizontalSync "Horizontal sync" 0.0 0.0 1.0 0.01
#pragma parameter HorizontalSyncStrength "Horizontal sync strength" 0.05 0.0 1.0 0.01
#pragma parameter Flickering "Flickering" 0.0 0.0 1.0 0.01
#pragma parameter ScaleNoiseX "Noise tile scale, x" 1.0 0.0 1000.0 0.01
#pragma parameter ScaleNoiseY "Noise tile scale, y" 1.0 0.0 1000.0 0.01
#pragma parameter FrameSize "Frame size" 0.0 0.0 1.0 0.01
#pragma parameter FrameEnabled "Whether the frame pass contributes" 0.0 0.0 1.0 1.0
#pragma parameter Bloom "Bloom" 0.0 0.0 5.0 0.01
#pragma parameter FontColorR "Font colour, red" 1.0 0.0 1.0 0.001
#pragma parameter FontColorG "Font colour, green" 1.0 0.0 1.0 0.001
#pragma parameter FontColorB "Font colour, blue" 1.0 0.0 1.0 0.001
#pragma parameter BackgroundColorR "Background colour, red" 0.0 0.0 1.0 0.001
#pragma parameter BackgroundColorG "Background colour, green" 0.0 0.0 1.0 0.001
#pragma parameter BackgroundColorB "Background colour, blue" 0.0 0.0 1.0 0.001
#pragma parameter DegaussBrightness "Degauss brightness multiplier" 1.0 1.0 4.0 0.01
#pragma parameter DegaussScaleY "Degauss vertical squeeze" 1.0 0.9 1.0 0.001
#pragma parameter WindowOpacity "Item opacity of the whole terminal" 1.0 0.0 1.0 0.001

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

// The static pass's output.
layout(set = 0, binding = 2) uniform sampler2D Source;
// The burn-in accumulator; pass 0, aliased `Burn`.
layout(set = 0, binding = 3) uniform sampler2D Burn;
// The terminal frame; B3's pass, aliased `FrameSource` because `Frame` would
// reserve `FrameSize`.
layout(set = 0, binding = 4) uniform sampler2D FrameSource;
// The tiled noise field, a librashader user texture.
layout(set = 0, binding = 5) uniform sampler2D NoiseSource;

float min2(vec2 v) { return min(v.x, v.y); }
float rgb2grey(vec3 v) { return dot(v, vec3(0.21, 0.72, 0.04)); }

vec2 virtualResolution() {
    return vec2(global.VirtualResolutionX, global.VirtualResolutionY);
}

vec2 distortCoordinates(vec2 coords) {
    vec2 paddedCoords = coords * (vec2(1.0) + global.FrameSize * 2.0) - global.FrameSize;
    vec2 cc = (paddedCoords - vec2(0.5));
    float dist = dot(cc, cc) * global.ScreenCurvature;
    return (paddedCoords + cc * (1.0 + dist) * dist);
}

vec3 applyRasterization(vec2 screenCoords, vec3 texel, vec2 virtualRes, float intensity) {
    // Modes 0 (none) and 4 (modern) leave the texel alone.
    int mode = int(global.RasterMode + 0.5);
    if (mode == 0 || mode == 4 || intensity <= 0.0) {
        return texel;
    }

    const float INTENSITY = 0.30;
    const float BRIGHTBOOST = 0.30;

    vec3 base = texel;
    if (mode == 3) {
        const float SUBPIXELS = 3.0;
        vec3 offsets = vec3(3.141592654) * vec3(0.5, 0.5 - 2.0 / 3.0, 0.5 - 4.0 / 3.0);
        vec2 omega = vec2(3.141592654) * vec2(2.0) * virtualRes;
        vec2 angle = screenCoords * omega;
        vec3 xfactors = (SUBPIXELS + sin(angle.x + offsets)) / (SUBPIXELS + 1.0);
        base = texel * xfactors;
    }

    vec3 pixelHigh = ((1.0 + BRIGHTBOOST) - (0.2 * base)) * base;
    vec3 pixelLow  = ((1.0 - INTENSITY) + (0.1 * base)) * base;

    vec2 coords = fract(screenCoords * virtualRes) * 2.0 - vec2(1.0);

    // Mode 2 (pixel) masks in both axes; modes 1 and 3 mask in y only.
    float mask;
    if (mode == 2) {
        vec2 sq = coords * coords;
        mask = 1.0 - sq.x - sq.y;
    } else {
        mask = 1.0 - abs(coords.y);
    }

    vec3 rasterizationColor = mix(pixelLow, pixelHigh, mask);
    return mix(texel, rasterizationColor, intensity);
}

float randomPass(vec2 coords) {
    return fract(smoothstep(-120.0, 0.0,
        coords.y - (global.VirtualResolutionY + 120.0) * fract(global.Time * 0.15)));
}

vec3 convertWithChroma(vec3 inColor) {
    vec3 fontColor = vec3(global.FontColorR, global.FontColorG, global.FontColorB);
    vec3 backgroundColor = vec3(global.BackgroundColorR, global.BackgroundColorG,
                                global.BackgroundColorB);
    float grey = rgb2grey(inColor);
    if (global.ChromaColor > 0.0) {
        float denom = max(grey, 0.0001);
        vec3 foregroundColor = mix(fontColor, inColor * fontColor / denom, global.ChromaColor);
        return mix(backgroundColor, foregroundColor, grey);
    }
    return mix(backgroundColor, fontColor, grey);
}

void main() {
    // The degauss squeeze, before anything samples. Dividing by the scale
    // widens the range sampled, which is what shrinking the picture looks
    // like from the output side; outside the unit square there is no picture,
    // so the bands the squeeze opens read black rather than a clamped smear.
    vec2 uv = vec2(0.5) + (vTexCoord - vec2(0.5)) / vec2(1.0, global.DegaussScaleY);
    float inside = min2(step(vec2(0.0), uv) - step(vec2(1.0), uv));

    // The per-vertex noise fetch, moved here. One noise fetch at a
    // time-derived coordinate feeds the flicker and the horizontal-sync tear.
    vec4 initialNoiseTexel = texture(NoiseSource,
        vec2(fract(global.Time / 2.048), fract(global.Time / 1048.576)));
    float vBrightness = 1.0 + (initialNoiseTexel.g - 0.5) * global.Flickering;
    float randval = global.HorizontalSyncStrength - initialNoiseTexel.r;
    float vDistortionScale = step(0.0, randval) * randval
                           * global.HorizontalSyncStrength * global.HorizontalSync;
    float vDistortionFreq = mix(4.0, 40.0, initialNoiseTexel.g) * step(0.0, global.HorizontalSync);

    vec2 cc = vec2(0.5) - uv;
    float distance = length(cc);

    vec2 staticCoords = distortCoordinates(uv);
    vec2 coords = uv;

    float dst = sin((coords.y + global.Time) * vDistortionFreq);
    coords.x += dst * vDistortionScale;

    vec4 noiseTexel = texture(NoiseSource,
        vec2(global.ScaleNoiseX, global.ScaleNoiseY) * coords
        + vec2(fract(global.Time / 0.051), fract(global.Time / 0.237)));

    vec2 txt_coords = coords
        + (noiseTexel.ba - vec2(0.5)) * vec2(global.JitterX, global.JitterY) * global.Jitter;

    float color = 0.0001;
    color += noiseTexel.a * global.StaticNoise * (1.0 - distance * 1.3);
    color += randomPass(coords * virtualResolution()) * global.GlowingLine;

    vec4 frameColor = texture(FrameSource, uv) * global.FrameEnabled;
    color *= (1.0 - frameColor.a);

    vec3 txt_color = texture(Source, txt_coords).rgb;
    float bloomScale = 1.0 + max(global.Bloom, 0.0);
    txt_color *= bloomScale;

    // The burn-in composite; `crt-burnin/MOUNT.md` explains each term.
    //
    //   * `(1.0 - txt_blur.a)` is the freshness mask on the consuming side.
    //     Alpha out of the accumulator is not opacity: it is 1 where the pixel
    //     is currently lit by the live image, and there the ghost contributes
    //     nothing, because the live text is already drawing it.
    //   * `0.65` is a brightness trim on the ghost, and belongs here with the
    //     composite rather than in the accumulator.
    //   * A third term, `- vec3(blurDecay)`, extrapolating the decay for the
    //     time since the accumulator was last updated, is gone: that only
    //     matters when the accumulator updates on a different cadence than
    //     the dynamic pass runs. Pass 0 here runs on every rendered frame, so
    //     that interval is always zero and the term is always subtracting
    //     nothing. If this graph ever gains a damage-gated accumulator the
    //     term comes back, and the two timestamps it needs belong in
    //     `crt_burnin::BurnInPass`, not in this pass's uniforms.
    if (global.BurnIn > 0.0) {
        vec4 txt_blur = texture(Burn, staticCoords);
        vec3 burnInColor = 0.65 * txt_blur.rgb * (1.0 - txt_blur.a);
        txt_color = max(txt_color, burnInColor);
    }

    txt_color += vec3(color);
    txt_color = applyRasterization(staticCoords, txt_color, virtualResolution(),
                                   global.RasterizationIntensity);

    vec3 finalColor = convertWithChroma(txt_color);
    float brightness = mix(1.0, vBrightness, step(0.0, global.Flickering));
    finalColor *= brightness;

    finalColor = mix(finalColor, frameColor.rgb, frameColor.a);

    // The degauss flood: a 0.25 white overlay whose peak is the same as a
    // 2.6x brightness on the phosphor; multiplying is the cheaper of the two
    // and is what the card fixes the hook at.
    finalColor *= global.DegaussBrightness;

    // Window opacity: `windowOpacity * 0.3 + 0.7`, applied on the finished
    // picture rather than on any one effect inside it. This pass is the
    // chain's output, so this is the same point in the pipeline. The uniform
    // arrives already through that `* 0.3 + 0.7` (`crt::params`); a
    // see-through profile is never fully transparent.
    //
    // Written premultiplied, colour and alpha carrying the same factor,
    // because the opacity is a *composite* and not a dim: what a translucent
    // picture blends with is the desktop, colour "#00000000". Multiplying the
    // colour alone would blend it with black instead, which is only the same
    // picture where the desktop happens to be black.
    //
    // On the surface this actually gets today the two are identical: `app`
    // configures `caps.alpha_modes[0]`, which is `Opaque` on every backend in
    // use, and no window is created transparent -- so the alpha is discarded
    // and the premultiplied colour is exactly what was drawn before. Writing
    // it this way is what makes the platform half a change to the window and
    // the surface alone, rather than to the picture as well.
    FragColor = vec4(clamp(finalColor, 0.0, 1.0) * inside * global.WindowOpacity,
                     inside * global.WindowOpacity);
}
