#language slang 2026

module block_face;

import common;

typedef SubchunkFaceGroupInfo<TintedBlockFaceInstance> SubchunkTintedFaceGroupInfo;

[[vk::push_constant]]
ConstantBuffer<SubchunkTintedFaceGroupInfo*> face_groups;

struct VertexOutput {
    float4 pos : SV_Position;
    float2 uvs;
    float3 light_rgb;
}

[shader("vertex")]
VertexOutput tinted_block_face_vertex_bda(
    uint draw_index : SV_DrawIndex,
    uint vertex_index : SV_VulkanVertexID,
    uint instance_index : SV_VulkanInstanceID,
) {
    VertexOutput out;
    // Fetch face group data.
    let face_group = face_groups[draw_index];
    // Fetch and unpack instance data.
    let instance = face_group.instances[instance_index];
    let uvs = instance.uvs;
    let tint_colour = instance.tint_colour;
    let packed_fields = instance.packed_fields;
    let xyz_offset = packed_fields.xyz_offset;
    let sky_light_level = packed_fields.sky_light_level;
    let block_light_level = packed_fields.block_light_level;
    // Position.
    let base_pos = get_base_position(vertex_index);
    let face_matrix = render_info.face_matrices[face_group.face_matrix_index];
    let local_pos = mul(face_matrix, base_pos);
    let block_centre_pos = face_group.subchunk_start_coords_float3 + xyz_offset;
    let global_pos = local_pos + block_centre_pos + 0.5;
    out.pos = float4(1.0, -1.0, 1.0, 1.0) * mul(render_info.view_matrix, float4(global_pos, 1.0));
    // UVs.
    let start_uvs = (float2)uvs.xy * render_info.recip_block_item_atlas_size;
    let end_uvs = (float2)uvs.zw * render_info.recip_block_item_atlas_size;
    let face_rotation_i = packed_fields.uv_rotation;
    let base_uvs = get_uvs(vertex_index, face_rotation_i);
    out.uvs = lerp(start_uvs, end_uvs, base_uvs);
    // Light RGB.
    // Includes block lighting, as well as some basic per-face directional shading.
    let normal = mul(face_matrix, float3(0.0, 1.0, 0.0));
    let light_source_dir = normalize(float3(2.0, 5.0, 1.0));
    let dir_lighting = dot(normal, light_source_dir);
    let dir_light_coef = (dir_lighting * 0.3) + 0.7;
    let lightmap_rgb = lightmap.Load((sky_light_level * 16) + block_light_level).rgb;
    out.light_rgb = lightmap_rgb * tint_colour.rgb * dir_light_coef;
    return out;
}

[shader("fragment")]
float4 tinted_block_face_fragment_bda(VertexOutput in) {
    let tex_sample = block_item_atlas.Sample(in.uvs);
    if (tex_sample.w < 1.0) {
        discard;
    }
    return float4(tex_sample.rgb * in.light_rgb, 1.0);
}
