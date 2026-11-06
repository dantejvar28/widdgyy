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

// Only bright, nearly neutral pixels contribute to the halo. This keeps the
// glow on light text while ignoring dark panel fills and saturated borders.
fn glow_weight(color: vec3<f32>) -> f32 {
    let max_component = max(color.r, max(color.g, color.b));
    let min_component = min(color.r, min(color.g, color.b));
    let luminance = dot(color, vec3<f32>(0.299, 0.587, 0.114));
    let saturation = (max_component - min_component) / max(max_component, 1e-4);
    let bright = smoothstep(0.55, 0.85, luminance);
    let neutral = 1.0 - smoothstep(0.10, 0.35, saturation);
    return bright * neutral;
}

// Soft halo sampled around the current pixel. Text glyphs are the only
// bright neutral shapes in the overlay, so weighting samples this way yields
// a text-only glow instead of outlining the whole panel.
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
        let neighbour = textureSample(source, source_sampler, uv + offset);
        halo_alpha += neighbour.a * glow_weight(neighbour.rgb);
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
