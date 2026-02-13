import package::common::{
    BASE_NORMAL, basic_sampler, block_item_atlas_texture, get_base_position, get_uvs, lightmap,
    render_info,
};

struct VertexInput {
    @builtin(vertex_index) index: u32,
    @location(0) subchunk_start_coords: vec3<f32>,
    @location(1) face_matrix_index: u32,
}

struct InstanceInput {
    @location(10) uvs: vec4<u32>,
    @location(11) tint_colour: vec4<f32>,
    @location(12) packed_xyz: vec2<u32>,
    @location(13) uv_rotation_and_lighting: vec2<u32>,
}

struct VertexOutput {
    @invariant
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uvs: vec2<f32>,
    @interpolate(flat)
    @location(1) light_rgb: vec3<f32>,
}

@vertex
fn vs_main(
    block_vertex: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;
    // Unpack instance data.
    let xyz_offset = vec3<f32>(
        f32(instance.packed_xyz[0] & 0xF),
        f32((instance.packed_xyz[0] >> 4) & 0xF),
        f32(instance.packed_xyz[1] & 0xF),
    );
    let sky_light_level = instance.uv_rotation_and_lighting[1] & 0xFu;
    let block_light_level = instance.uv_rotation_and_lighting[1] >> 4u;
    // Position
    let base_pos = get_base_position(block_vertex.index);
    let base_uvs = get_uvs(block_vertex.index, instance.uv_rotation_and_lighting[0]);
    let face_matrix = render_info.face_matrices[block_vertex.face_matrix_index];
    let local_pos = face_matrix * base_pos;
    let block_centre_pos = block_vertex.subchunk_start_coords + xyz_offset;
    let global_pos = local_pos + block_centre_pos + vec3(0.5);
    out.clip_pos = render_info.view_matrix * vec4(global_pos, 1.0);
    // UVs
    let start_uvs = vec2<f32>(instance.uvs.xy) * render_info.recip_block_item_atlas_size;
    let end_uvs = vec2<f32>(instance.uvs.zw) * render_info.recip_block_item_atlas_size;
    out.uvs = mix(start_uvs, end_uvs, base_uvs);
    // Light RGB
    // Includes block lighting, as well as some basic per-face directional shading.
    let normal = face_matrix * BASE_NORMAL;
    let light_source_dir = normalize(vec3(2.0, 5.0, 1.0));
    let dir_lighting = dot(normal, light_source_dir);
    let dir_light_coef = fma(dir_lighting, 0.3, 0.7);
    let lightmap_rgb = unpack4x8unorm(lightmap[sky_light_level][block_light_level]).rgb;
    out.light_rgb = lightmap_rgb * instance.tint_colour.rgb * dir_light_coef;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_sample = textureSample(block_item_atlas_texture, basic_sampler, in.uvs);
    if tex_sample.a < 1.0 {
        discard;
    }
    return vec4(tex_sample.rgb * in.light_rgb, 1.0);
}
