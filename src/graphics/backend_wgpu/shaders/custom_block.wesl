import package::common::{
    BASE_NORMAL, basic_sampler, block_item_atlas_texture, custom_block_faces, get_base_position,
    get_uvs, lightmap, render_info,
};

const BLOCK_FACE_INDICES: array<u32, 6> = array(1, 0, 2, 3, 1, 2);

struct InstanceInput {
    @location(10) pos: vec3<f32>,
    @location(11) tint_colour: vec4<f32>,
    @location(12) light_level_pairs_1: vec4<u32>,
    @location(13) light_level_pairs_2: vec4<u32>,
}

struct VertexOutput {
    @builtin(position) @invariant clip_pos: vec4<f32>,
    @location(0) uvs: vec2<f32>,
    @location(1) light_rgb: vec3<f32>,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_i: u32,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;
    // Pull vertex from faces buffer.
    let face_i = vertex_i / 6;
    let face_vertex_i = BLOCK_FACE_INDICES[vertex_i % 6];
    let vertex = custom_block_faces[face_i][face_vertex_i];
    let vertex_local_pos = vec3(vertex.local_pos[0], vertex.local_pos[1], vertex.local_pos[2]);
    let vertex_normal = vec3(vertex.normal[0], vertex.normal[1], vertex.normal[2]);
    let vertex_uvs = vec2(
        vertex.packed_uvs & 0xFFFFu,
        (vertex.packed_uvs >> 16u) & 0xFFFFu,
    );
    // Position
    let global_pos = vertex_local_pos + instance.pos + vec3(0.5);
    out.clip_pos = render_info.view_matrix * vec4(global_pos, 1.0);
    // UVs
    out.uvs = vec2<f32>(vertex_uvs) * render_info.recip_block_item_atlas_size;
    // Light RGB
    {
        let adjusted_pos = fma(vertex_normal, vec3(0.02), vertex_local_pos);
        let light_pair_positions = array(
            vec3(0.0, 0.0, 0.0),
            vec3(0.0, 1.01, 0.0),
            vec3(0.0, -1.01, 0.0),
            vec3(0.0, 0.0, -1.01),
            vec3(0.0, 0.0, 1.01),
            vec3(0.0, 1.01, 0.0),
            vec3(0.0, -1.01, 0.0),
        );
        let light_pairs = array(
            vec2(instance.light_level_pairs_1.x & 0xFu, instance.light_level_pairs_1.x >> 4u),
            vec2(instance.light_level_pairs_1.y & 0xFu, instance.light_level_pairs_1.y >> 4u),
            vec2(instance.light_level_pairs_1.z & 0xFu, instance.light_level_pairs_1.z >> 4u),
            vec2(instance.light_level_pairs_1.w & 0xFu, instance.light_level_pairs_1.w >> 4u),
            vec2(instance.light_level_pairs_2.x & 0xFu, instance.light_level_pairs_2.x >> 4u),
            vec2(instance.light_level_pairs_2.y & 0xFu, instance.light_level_pairs_2.y >> 4u),
            vec2(instance.light_level_pairs_2.z & 0xFu, instance.light_level_pairs_2.z >> 4u),
        );
        var closest_dist: f32 = 1000000.0;
        var closest_light_pair_idx: u32;
        for (var i: u32 = 0; i < 7; i++) {
            let dist = distance(adjusted_pos, light_pair_positions[i]);
            let is_closer = dist < closest_dist;
            closest_dist = select(closest_dist, dist, is_closer);
            closest_light_pair_idx = select(closest_light_pair_idx, i, is_closer);
        }
        let closest_light_pair = light_pairs[closest_light_pair_idx];
        // Per-face directional shading.
        let light_source_dir = normalize(vec3(2.0, 5.0, 1.0));
        let dir_lighting = dot(vertex_normal, light_source_dir);
        let dir_light_coef = fma(dir_lighting, 0.3, 0.7);
        // Lightmap fetch.
        let sky_light_level = closest_light_pair[0];
        let block_light_level = closest_light_pair[1];
        let lightmap_rgb = unpack4x8unorm(lightmap[sky_light_level][block_light_level]).rgb;
        // Tint.
        let applied_tint_rgb =
            mix(vec3(1.0, 1.0, 1.0), instance.tint_colour.rgb, vertex.tint_percentage);
        out.light_rgb = lightmap_rgb * applied_tint_rgb * dir_light_coef;
    }
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
