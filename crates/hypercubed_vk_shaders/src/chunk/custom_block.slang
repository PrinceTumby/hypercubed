#language slang 2026

module custom_block;

import "../_common";

[[vk::push_constant]]
internal ConstantBuffer<CustomBlockInstance**> instance_groups;

internal struct VertexOutput {
    float4 pos : SV_Position;
    float2 uvs;
    float3 light_rgb;
}

[shader("vertex")]
VertexOutput vertex(
    uint draw_index : SV_DrawIndex,
    uint vertex_index : SV_VulkanVertexID,
    uint instance_index : SV_VulkanInstanceID,
) {
    VertexOutput out;
    // Fetch instance group data.
    let instance_group = instance_groups[draw_index];
    // Fetch and unpack instance data.
    let instance = instance_group[instance_index];
    let instance_pos = instance.pos;
    let tint_colour = instance.tint_colour;
    let light_level_pairs_1 = instance.light_level_pairs_1;
    let light_level_pairs_2_and_packed_fields = instance.light_level_pairs_2_and_packed_fields;
    // Pull vertex from faces buffer and unpack data.
    let face_i = vertex_index / 6;
    uint[6] block_face_indices = {1, 0, 2, 3, 1, 2};
    let face_vertex_i = block_face_indices[vertex_index % 6];
    let vertex = custom_block_faces[face_i][face_vertex_i];
    let vertex_pos = vertex.pos;
    let uvs = vertex.uvs;
    let normal = vertex.normal;
    let packed_fields = vertex.packed_fields;
    uint[7] light_pairs = {
        light_level_pairs_1.x,
        light_level_pairs_1.y,
        light_level_pairs_1.z,
        light_level_pairs_1.w,
        light_level_pairs_2_and_packed_fields.x,
        light_level_pairs_2_and_packed_fields.y,
        light_level_pairs_2_and_packed_fields.z,
    };
    // Position.
    let global_pos = vertex_pos + instance_pos + 0.5;
    out.pos = float4(1.0, -1.0, 1.0, 1.0) * mul(render_info.view_matrix, float4(global_pos, 1.0));
    // UVs.
    out.uvs = uvs * render_info.recip_block_item_atlas_size;
    // Light RGB.
    // Includes block lighting, as well as some basic per-face directional shading.
    {
        let adjusted_pos = (normal * 0.02) + vertex_pos;
        float3[7] light_pair_positions = {
            float3(0.0, 0.0, 0.0),
            float3(0.0, 1.01, 0.0),
            float3(0.0, -1.01, 0.0),
            float3(0.0, 0.0, -1.01),
            float3(0.0, 0.0, 1.01),
            float3(0.0, 1.01, 0.0),
            float3(0.0, -1.01, 0.0),
        };
        float closest_squared_dist = 1.#INF;
        uint closest_light_pair_index = 0;
        for (uint i = 0; i < 7; i++) {
            let diff = adjusted_pos - light_pair_positions[i];
            let squared_dist = dot(diff, diff);
            let is_closer = squared_dist < closest_squared_dist;
            closest_squared_dist = is_closer ? squared_dist : closest_squared_dist;
            closest_light_pair_index = is_closer ? i : closest_light_pair_index;
        }
        let closest_light_pair = light_pairs[closest_light_pair_index];
        // Per-face directional shading.
        let light_source_dir = normalize(float3(2.0, 5.0, 1.0));
        let dir_lighting = dot(normal, light_source_dir);
        let dir_light_coef = (dir_lighting * 0.3) + 0.7;
        // Lightmap fetch.
        let sky_light_level = closest_light_pair & 0xF;
        let block_light_level = (closest_light_pair >> 4) & 0xF;
        let lightmap_rgb = lightmap.Load((sky_light_level * 16) + block_light_level).rgb;
        // Tint.
        let tint_percentage = packed_fields.tint_percentage;
        let applied_tint_rgb = lerp(float3(1.0), tint_colour.xyz, tint_percentage);
        out.light_rgb = lightmap_rgb * applied_tint_rgb * dir_light_coef;
    }
    return out;
}

[shader("fragment")]
float4 fragment(VertexOutput in) {
    let tex_sample = block_item_atlas.Sample(in.uvs);
    if (tex_sample.w < 1.0) {
        discard;
    }
    return float4(tex_sample.rgb * in.light_rgb, 1.0);
}
