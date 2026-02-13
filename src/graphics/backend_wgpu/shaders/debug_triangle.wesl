import package::common::render_info;

struct InstanceInput {
    @builtin(vertex_index) index: u32,
    @location(0) p1: vec3<f32>,
    @location(1) p2: vec3<f32>,
    @location(2) p3: vec3<f32>,
    @location(3) color: vec4<f32>,
    @location(4) flags: u32,
}

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @interpolate(flat) @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(in: InstanceInput) -> VertexOutput {
    var out: VertexOutput;
    var vertex_pos: vec3<f32>;
    switch in.index {
        case 0: {
            vertex_pos = in.p1;
        }
        case 1: {
            vertex_pos = in.p2;
        }
        case 2, default: {
            vertex_pos = in.p3;
        }
    }
    var pos = render_info.view_matrix * vec4(vertex_pos, 1.0);
    // `ignore_depth` flag.
    if (in.flags & 0x1u) != 0u {
        pos.z = sign(pos.z);
    }
    out.clip_pos = pos;
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
