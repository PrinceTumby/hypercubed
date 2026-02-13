import package::common::{basic_sampler, moon_phases_texture, render_info};

const MOON_BASE_POSITIONS: array<vec3<f32>, 4> = array(
    vec3(-0.90453404, 0.30151135, -0.30151135),
    vec3(-0.90453404, -0.30151135, -0.30151135),
    vec3(-0.90453404, 0.30151135, 0.30151135),
    vec3(-0.90453404, -0.30151135, 0.30151135),
);

fn calculate_moon_uvs(time_of_day: i32, vertex_i: u32) -> vec2<f32> {
    let moon_phase_i = ((time_of_day - 6000) / 24000) % 8;
    let uv_start_x = f32(moon_phase_i % 4) / 4.0;
    let uv_start_y = f32(moon_phase_i / 4) / 2.0;
    let uv_width = 1.0 / 4.0;
    let uv_height = 1.0 / 2.0;
    var out: vec2<f32>;
    switch vertex_i % 4 {
        case 0u: {
            out = vec2(uv_start_x, uv_start_y);
        }
        case 1u: {
            out = vec2(uv_start_x + uv_width, uv_start_y);
        }
        case 2u: {
            out = vec2(uv_start_x, uv_start_y + uv_height);
        }
        case 3u, default: {
            out = vec2(uv_start_x + uv_width, uv_start_y + uv_height);
        }
    }
    return out;
}

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
    let base_pos = MOON_BASE_POSITIONS[in.index % 4];
    out.clip_pos = render_info.sky_matrix * vec4(base_pos, 1.0);
    out.uvs = calculate_moon_uvs(i32(render_info.time_of_day), in.index);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(moon_phases_texture, basic_sampler, in.uvs);
}
