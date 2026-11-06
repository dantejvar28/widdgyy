// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var uvs = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(2.0, 1.0),
        vec2<f32>(0.0, -1.0),
    );
    return VertexOutput(vec4<f32>(positions[index], 0.0, 1.0), uvs[index]);
}

struct Glow {
    // x: elapsed seconds, y: glow strength, z: glow radius in pixels
    params: vec4<f32>,
};

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@group(0) @binding(2) var<uniform> glow: Glow;

// Soft halo sampled around the current pixel. The overlay texture only
// contains text and panels, so the neighbouring alpha is a good stand-in for
// a blurred glow while staying cheap enough for video frame rate.
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let uv = input.uv;
    let base = textureSample(source, source_sampler, uv);
    if glow.params.y <= 0.0 || glow.params.z <= 0.0 {
        return base;
    }
    let texel = 1.0 / vec2<f32>(textureDimensions(source));
    let radius = glow.params.z;
    let golden_angle = 2.3999631;
    var halo_alpha = 0.0;
    for (var sample = 0u; sample < 12u; sample += 1u) {
        let fraction = (f32(sample) + 0.5) / 12.0;
        let distance = radius * sqrt(fraction);
        let angle = golden_angle * f32(sample);
        let offset = vec2<f32>(cos(angle), sin(angle)) * texel * distance;
        halo_alpha += textureSample(source, source_sampler, uv + offset).a;
    }
    halo_alpha /= 12.0;

    let pulse = 0.62 + 0.38 * sin(glow.params.x * glow.params.w);
    let strength = glow.params.y * pulse;
    // The halo uses a fixed lavender-white tint so it still reads as light
    // around dark text and over bright parts of the wallpaper.
    let halo_rgb = vec3<f32>(0.80, 0.66, 1.0);
    let color = base.rgb + halo_rgb * halo_alpha * strength;
    let alpha = clamp(base.a + halo_alpha * strength * 0.85, 0.0, 1.0);
    return vec4<f32>(color, alpha);
}
