// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

// Offscreen CSS filter passes.
//
// Vello writes straight (unpremultiplied) alpha into the layer texture. The
// separable blur converts to premultiplied alpha before filtering so colour
// does not bleed out of transparent pixels. The composite pass writes
// premultiplied colour, which is what the final blit to the premultiplied
// surface expects.

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

struct BlurParams {
    step: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var blur_source: texture_2d<f32>;
@group(0) @binding(1) var blur_sampler: sampler;
@group(0) @binding(2) var<uniform> blur_params: BlurParams;

// Seventeen-tap Gaussian approximation. More taps keep large radii smooth
// instead of showing the sampling grid as a cross-hatch pattern.
const BLUR_WEIGHTS = array<f32, 9>(
    0.1335712203,
    0.1263529607,
    0.1069554721,
    0.0810150404,
    0.0549127703,
    0.0333062788,
    0.0180768989,
    0.0087794398,
    0.0038155288,
);

fn blur_sample(uv: vec2<f32>, step: vec2<f32>, premultiply: bool) -> vec4<f32> {
    var sum = vec4<f32>(0.0);
    for (var i = 0u; i < 9u; i += 1u) {
        let offset = step * f32(i);
        var a = textureSample(blur_source, blur_sampler, uv + offset);
        var b = textureSample(blur_source, blur_sampler, uv - offset);
        if premultiply {
            a = vec4<f32>(a.rgb * a.a, a.a);
            b = vec4<f32>(b.rgb * b.a, b.a);
        }
        if i == 0u {
            sum += a * BLUR_WEIGHTS[i];
        } else {
            sum += (a + b) * BLUR_WEIGHTS[i];
        }
    }
    return sum;
}

/// First blur pass: straight-alpha layer -> premultiplied blur target.
@fragment
fn fs_premultiply(input: VertexOutput) -> @location(0) vec4<f32> {
    return blur_sample(input.uv, blur_params.step, true);
}

/// Second blur pass: premultiplied blur target -> premultiplied blur target.
@fragment
fn fs_blur(input: VertexOutput) -> @location(0) vec4<f32> {
    return blur_sample(input.uv, blur_params.step, false);
}

struct CompositeParams {
    shadow_color: vec4<f32>,
    offset: vec2<f32>,
    mode: f32,
    premultiplied: f32,
};

@group(0) @binding(0) var layer_source: texture_2d<f32>;
@group(0) @binding(1) var blur_result: texture_2d<f32>;
@group(0) @binding(2) var composite_sampler: sampler;
@group(0) @binding(3) var<uniform> composite_params: CompositeParams;

fn premultiplied(source: vec4<f32>, already: bool) -> vec4<f32> {
    if already {
        return source;
    }
    return vec4<f32>(source.rgb * source.a, source.a);
}

@fragment
fn fs_composite(input: VertexOutput) -> @location(0) vec4<f32> {
    let uv = input.uv;
    let source = premultiplied(
        textureSample(layer_source, composite_sampler, uv),
        composite_params.premultiplied > 0.5,
    );

    // 0 = unfiltered layer, 1 = blurred content, 2 = drop shadow.
    if composite_params.mode < 0.5 {
        return source;
    }

    let blurred = textureSample(blur_result, composite_sampler, uv - composite_params.offset);
    if composite_params.mode < 1.5 {
        return blurred;
    }

    // Drop shadow: tint the blurred alpha and composite it behind the
    // original layer. Both operands are premultiplied.
    let shadow_alpha = blurred.a * composite_params.shadow_color.a;
    let shadow = vec4<f32>(composite_params.shadow_color.rgb * shadow_alpha, shadow_alpha);
    return vec4<f32>(
        source.rgb + shadow.rgb * (1.0 - source.a),
        source.a + shadow_alpha * (1.0 - source.a),
    );
}
