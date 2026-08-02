#language slang 2026

module sun;

import "../_common";

static const float3[4] SUN_BASE_POSITIONS = {
    float3(0.90453404, 0.30151135, -0.30151135),
    float3(0.90453404, -0.30151135, -0.30151135),
    float3(0.90453404, 0.30151135, 0.30151135),
    float3(0.90453404, -0.30151135, 0.30151135),
};

static const float2[4] SUN_UVS = {
    float2(0, 0),
    float2(1, 0),
    float2(0, 1),
    float2(1, 1),
};

struct VertexOutput {
    float4 pos : SV_Position;
    float2 uvs;
}

[shader("vertex")]
VertexOutput vertex(uint vertex_index : SV_VulkanVertexID) {
    VertexOutput out;
    let base_pos = SUN_BASE_POSITIONS[vertex_index % 4];
    out.pos = mul(render_info.sky_matrix, float4(base_pos, 1.0));
    out.pos.y = -out.pos.y;
    out.uvs = SUN_UVS[vertex_index % 4];
    return out;
}

[shader("fragment")]
float4 fragment(VertexOutput in) : SV_Target {
    return sun_image.Sample(in.uvs);
}
