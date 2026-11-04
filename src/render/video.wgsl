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

@group(0) @binding(0) var y_plane: texture_2d<f32>;
@group(0) @binding(1) var uv_plane: texture_2d<f32>;
@group(0) @binding(2) var source_sampler: sampler;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let y = textureSample(y_plane, source_sampler, input.uv).r;
    let uv = textureSample(uv_plane, source_sampler, input.uv).rg;

    // BT.709 limited-range NV12 -> RGB.
    let luma = (y - 16.0 / 255.0) * 1.164;
    let u = uv.x - 0.5;
    let v = uv.y - 0.5;
    let r = luma + 1.793 * v;
    let g = luma - 0.213 * u - 0.533 * v;
    let b = luma + 2.112 * u;
    return vec4<f32>(r, g, b, 1.0);
}
