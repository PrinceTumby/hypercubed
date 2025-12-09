@group(0) @binding(0)
var<uniform> view_info: ViewInfo;

struct ViewInfo {
    view_matrix: mat4x4<f32>,
    screen_size: vec2<u32>,
}

struct InstanceInput {
    @location(0) p1: vec3<f32>,
    @location(1) p2: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) size: f32,
    @location(4) flags: u32,
}

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @interpolate(flat) @location(0) color: vec4<f32>,
    @interpolate(flat) @location(1) flags: u32,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_i: u32,
    in: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;
    let p1_clip_pos = view_info.view_matrix * vec4(in.p1, 1.0);
    let p2_clip_pos = view_info.view_matrix * vec4(in.p2, 1.0);
    let clip_diff_norm: vec2<f32> = normalize(p2_clip_pos.xy - p1_clip_pos.xy);
    // Calculate the perpendicular vector offset.
    let screen_size_f32 = vec2<f32>(view_info.screen_size);
    let offset_right = clip_diff_norm.yx * vec2(-1.0, 1.0) * in.size / screen_size_f32;
    let offset = select(offset_right, -offset_right, vertex_i % 2 == 0);
    var vertex_clip_pos = select(p1_clip_pos, p2_clip_pos, vertex_i / 2 != 0);
    vertex_clip_pos += vec4(offset * vertex_clip_pos.w, 0.0, 0.0);
    out.clip_pos = vertex_clip_pos;
    out.color = in.color;
    out.flags = in.flags;
    return out;
}

struct FragmentOutput {
    @builtin(frag_depth) depth: f32,
    @location(0) color: vec4<f32>,
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;
    out.depth = select(in.clip_pos.z, 1.0, (in.flags & 0x1u) != 0u);
    out.color = vec4(in.color.xyz * in.color.w, in.color.w);
    return out;
}
