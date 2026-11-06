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

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@group(0) @binding(2) var glow_source: texture_2d<f32>;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let uv = input.uv;
    let base = textureSample(source, source_sampler, uv);

    // The glow texture contains the CSS text shadows, already blurred and
    // stored premultiplied. Composite them behind the crisp overlay text.
    let glow_sample = textureSample(glow_source, source_sampler, uv);
    let glow_rgb = glow_sample.rgb / max(glow_sample.a, 1e-4);
    let alpha = clamp(base.a + glow_sample.a, 0.0, 1.0);
    let color = (base.rgb * base.a + glow_rgb * glow_sample.a) / max(alpha, 1e-4);
    return vec4<f32>(color, alpha);
}
