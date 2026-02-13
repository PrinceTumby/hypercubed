import package::common::render_info;

struct VertexInput {
    @builtin(vertex_index) index: u32,
}

struct InstanceInput {
    @location(0) p1: vec3<f32>,
    @location(1) p2: vec3<f32>,
    @location(2) p3: vec3<f32>,
    @location(3) p4: vec3<f32>,
}

struct VertexOutput {
    @invariant
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) star_brightness: f32,
}

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    var out: VertexOutput;
    let global_pos = array(instance.p1, instance.p2, instance.p3, instance.p4)[vertex.index];
    out.clip_pos = render_info.sky_matrix * vec4(global_pos, 1.0);
    out.star_brightness = render_info.star_brightness;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4(vec3(in.star_brightness), 0.0);
}
