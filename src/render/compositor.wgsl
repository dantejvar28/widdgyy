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

// One CSS text shadow: placement = (uv offset, slot), color = straight RGBA.
struct ShadowItem {
    placement: vec4<f32>,
    color: vec4<f32>,
};

struct ShadowParams {
    // x = number of active shadows.
    header: vec4<f32>,
    items: array<ShadowItem, 16>,
};

@group(0) @binding(0) var overlay: texture_2d<f32>;
@group(0) @binding(1) var overlay_sampler: sampler;
@group(0) @binding(2) var<uniform> shadows: ShadowParams;
@group(0) @binding(3) var slot0: texture_2d<f32>;
@group(0) @binding(4) var slot1: texture_2d<f32>;
@group(0) @binding(5) var slot2: texture_2d<f32>;
@group(0) @binding(6) var slot3: texture_2d<f32>;

// Explicit LOD keeps these samples legal inside the non-uniform shadow loop.
fn slot_coverage(slot: i32, uv: vec2<f32>) -> f32 {
    if slot == 0 {
        return textureSampleLevel(slot0, overlay_sampler, uv, 0.0).a;
    }
    if slot == 1 {
        return textureSampleLevel(slot1, overlay_sampler, uv, 0.0).a;
    }
    if slot == 2 {
        return textureSampleLevel(slot2, overlay_sampler, uv, 0.0).a;
    }
    return textureSampleLevel(slot3, overlay_sampler, uv, 0.0).a;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let uv = input.uv;

    // Shadows are composed premultiplied in paint order: each later item is
    // placed over the accumulated result, as CSS does for text-shadow lists.
    var shadow_rgb = vec3<f32>(0.0);
    var shadow_a = 0.0;
    let count = u32(shadows.header.x);
    for (var i = 0u; i < count; i += 1u) {
        let item = shadows.items[i];
        let coverage = slot_coverage(i32(item.placement.z), uv - item.placement.xy);
        let src_a = coverage * item.color.a;
        shadow_rgb = item.color.rgb * src_a + shadow_rgb * (1.0 - src_a);
        shadow_a = src_a + shadow_a * (1.0 - src_a);
    }

    // The crisp overlay sits above every shadow.
    let base = textureSample(overlay, overlay_sampler, uv);
    let out_a = base.a + shadow_a * (1.0 - base.a);
    let out_premultiplied = base.rgb * base.a + shadow_rgb * (1.0 - base.a);
    return vec4<f32>(out_premultiplied / max(out_a, 1e-4), out_a);
}

@group(0) @binding(0) var backdrop_blur: texture_2d<f32>;
@group(0) @binding(1) var backdrop_mask: texture_2d<f32>;
@group(0) @binding(2) var backdrop_sampler: sampler;

/// Blurred video masked by the panel coverage. Drawn over the sharp video,
/// before the overlay.
@fragment
fn fs_backdrop(input: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(backdrop_blur, backdrop_sampler, input.uv);
    let mask = textureSample(backdrop_mask, backdrop_sampler, input.uv).a;
    return vec4<f32>(color.rgb, mask);
}
