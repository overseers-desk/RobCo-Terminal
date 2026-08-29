//! The unproven corner worth retiring early: can librashader's user
//! texture path carry `noiseSource` and `frameSource`?
//!
//! The two sources are not the same kind of thing, and only one of them is a
//! LUT question at all.
//!
//! `frameSource` is a *rendered* texture, not an image on disk. It is a
//! chain pass here, aliased `FrameSource`, so no user-texture path is involved.
//!
//! `noiseSource` is a genuine LUT: a 512x512 noise texture, tiled by
//! repeating, its four channels four independent noise fields feeding the
//! jitter, the static, the flicker and the horizontal-sync tear. That is
//! what this file tests, and it tests the two properties this crate
//! actually depends on: that the image reaches the sampler with its content
//! intact, and that it wraps by repeating rather than clamping. A clamped
//! noise texture would still be a picture; it would just smear one edge
//! pixel across the whole screen once the time-derived offset walked past
//! 1.0, which is within the first two seconds of running.

use crate::support;

use std::path::Path;

use crt::preset::{self, Structure};
use librashader::presets::ShaderFeatures;
use librashader::runtime::wgpu::{FilterChain, FilterChainOptions, WgpuOutputView};
use librashader::runtime::{Size, Viewport};
use gpu::Image;

/// A one-pass preset that does nothing but show the LUT.
const PROBE_PRESET: &str = "\
textures = \"NoiseSource\"
NoiseSource = \"allNoise512.png\"
NoiseSource_linear = false
NoiseSource_wrap_mode = repeat
NoiseSource_mipmap = false

shaders = 1
shader0 = \"lut_probe.slang\"
filter_linear0 = \"false\"
";

fn probe_shader(offset: &str) -> String {
    format!(
        "\
#version 450
layout(push_constant) uniform Push {{
    vec4 SourceSize;
    vec4 OriginalSize;
    vec4 OutputSize;
    uint FrameCount;
}} params;
layout(std140, set = 0, binding = 0) uniform UBO {{ mat4 MVP; }} global;

#pragma stage vertex
layout(location = 0) in vec4 Position;
layout(location = 1) in vec2 TexCoord;
layout(location = 0) out vec2 vTexCoord;
void main() {{ gl_Position = global.MVP * Position; vTexCoord = TexCoord; }}

#pragma stage fragment
layout(location = 0) in vec2 vTexCoord;
layout(location = 0) out vec4 FragColor;
layout(set = 0, binding = 2) uniform sampler2D NoiseSource;
void main() {{ FragColor = texture(NoiseSource, vTexCoord + {offset}); }}
"
    )
}

fn render_probe(h: &support::Harness, dir: &Path, offset: &str) -> Image {
    std::fs::create_dir_all(dir).expect("probe dir");
    // Reuse the real materialiser so the probe reads the same PNG bytes the
    // chain does, from the same place.
    preset::materialize(dir, &Structure::from_config(&config::Config::default())).expect("assets");
    std::fs::write(dir.join("lut_probe.slang"), probe_shader(offset)).expect("probe shader");
    let preset_path = dir.join("lut_probe.slangp");
    std::fs::write(&preset_path, PROBE_PRESET).expect("probe preset");

    let mut chain = FilterChain::load_from_path(
        &preset_path,
        ShaderFeatures::NONE,
        &h.gpu.device,
        &h.gpu.queue,
        Some(&FilterChainOptions {
            force_no_mipmaps: false,
            enable_cache: false,
            adapter_info: None,
        }),
    )
    .expect("the probe chain loads");

    let size = Size::new(h.output.width, h.output.height);
    let mut encoder = h
        .gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    chain
        .frame(
            &h.input.texture,
            &Viewport {
                x: 0.0,
                y: 0.0,
                mvp: None,
                output: WgpuOutputView::new_from_raw(
                    &h.output.view,
                    size,
                    gpu::TARGET_FORMAT,
                ),
                size,
            },
            &mut encoder,
            0,
            None,
        )
        .expect("probe frame");
    let index = h.gpu.queue.submit([encoder.finish()]);
    h.gpu
        .device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(index),
            timeout: None,
        })
        .expect("poll");
    h.output.read_rgba(&h.gpu.device, &h.gpu.queue)
}

#[test]
fn the_noise_lut_reaches_the_shader_and_repeats() {
    let h = support::Harness::new("lut", 64, 64).expect("gpu");
    h.draw_picture();

    let plain = render_probe(&h, &h.dir.join("plain"), "vec2(0.0)");

    // A LUT that failed to load binds as a solid colour, so the test that
    // separates "loaded" from "silently absent" is that the image has
    // structure. `allNoise512.png` is four channels of noise, so a 64x64 window
    // onto it has a wide spread of values in every channel.
    let reds = plain.distinct_luma_values();
    assert!(
        reds.len() > 32,
        "the noise LUT read back only {} distinct red values, which is not a \
         noise field: the texture did not reach the sampler",
        reds.len()
    );
    let mean = support::mean_luma(&plain);
    assert!(
        (0.05..0.95).contains(&mean),
        "mean luma {mean:.4} is at a rail, so the sampler is reading a constant"
    );

    // Repeat wrap: three whole tiles across and seven down is the same tile.
    // The dynamic pass samples at `fract(Time / 0.051)`, which leaves the unit
    // square within a twentieth of a second of the clock starting, so this is
    // the property the port stands on, not a nicety.
    let wrapped = render_probe(&h, &h.dir.join("wrapped"), "vec2(3.0, 7.0)");
    let diff = plain.diff(&wrapped);
    assert_eq!(
        diff.differing,
        0,
        "sampling three tiles across and seven down differs from sampling the \
         first tile, so the LUT is not wrapping by repeat: {}",
        diff.describe()
    );

    println!(
        "user-LUT verdict: allNoise512.png loaded through librashader's \
         `textures =` path, {} distinct values in red, mean luma {:.4}, and \
         wrap_mode=repeat holds exactly at +3/+7 tiles.",
        reds.len(),
        mean
    );
}
