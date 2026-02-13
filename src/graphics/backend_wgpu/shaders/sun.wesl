import package::common::{basic_sampler, render_info, sun_texture};

const SUN_BASE_POSITIONS: array<vec3<f32>, 4> = array(
    vec3(0.90453404, 0.30151135, -0.30151135),
    vec3(0.90453404, -0.30151135, -0.30151135),
    vec3(0.90453404, 0.30151135, 0.30151135),
    vec3(0.90453404, -0.30151135, 0.30151135),
);

const SUN_UVS: array<vec2<f32>, 4> = array(
    vec2(0.0, 0.0),
    vec2(1.0, 0.0),
    vec2(0.0, 1.0),
    vec2(1.0, 1.0),
);

struct VertexInput {
    @builtin(vertex_index) index: u32,
}

struct VertexOutput {
    @invariant
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uvs: vec2<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let base_pos = SUN_BASE_POSITIONS[in.index % 4];
    out.clip_pos = render_info.sky_matrix * vec4(base_pos, 1.0);
    out.uvs = SUN_UVS[in.index % 4];
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(sun_texture, basic_sampler, in.uvs);
}
