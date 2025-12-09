@group(0) @binding(0)
var<uniform> view_matrix: mat4x4<f32>;

struct InstanceInput {
    @location(0) pos: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) size: f32,
    @location(3) flags: u32,
}

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @interpolate(flat) @location(0) color: vec4<f32>,
}

const VERTEX_SCREEN_OFFSETS: array<vec2<f32>, 4> = array(
    vec2(-0.5, -0.5),
    vec2(0.5, -0.5),
    vec2(-0.5, 0.5),
    vec2(0.5, 0.5),
);

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_i: u32,
    in: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;
    var centre_pos = view_matrix * vec4(in.pos, 1.0);
    // `ignore_depth` flag.
    if (in.flags & 0x1u) != 0u {
        centre_pos.z = sign(centre_pos.z);
    }
    let vertex_screen_offset = vec4(VERTEX_SCREEN_OFFSETS[vertex_i % 4], 0.0, 0.0);
    out.clip_pos = centre_pos + (vertex_screen_offset * in.size);
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
