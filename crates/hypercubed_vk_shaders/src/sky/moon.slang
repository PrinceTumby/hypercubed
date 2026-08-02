#language slang 2026

module moon;

import "../_common";

static const float3[4] MOON_BASE_POSITIONS = {
    float3(-0.90453404, 0.30151135, -0.30151135),
    float3(-0.90453404, -0.30151135, -0.30151135),
    float3(-0.90453404, 0.30151135, 0.30151135),
    float3(-0.90453404, -0.30151135, 0.30151135),
};

float2 calculate_moon_uvs(int32_t time_of_day, uint vertex_index) {
    let moon_phase_i = ((time_of_day - 6000) / 24000) % 8;
    let uv_start_x = float(moon_phase_i % 4) / 4.0;
    let uv_start_y = float(moon_phase_i / 4) / 2.0;
    let uv_width = 1.0 / 4.0;
    let uv_height = 1.0 / 2.0;
    switch (vertex_index) {
        case 0: return float2(uv_start_x, uv_start_y);
        case 1: return float2(uv_start_x + uv_width, uv_start_y);
        case 2: return float2(uv_start_x, uv_start_y + uv_height);
        default: return float2(uv_start_x + uv_width, uv_start_y + uv_height);
    }
}

struct VertexOutput {
    float4 pos : SV_Position;
    float2 uvs;
}

[shader("vertex")]
VertexOutput vertex(uint vertex_index : SV_VulkanVertexID) {
    VertexOutput out;
    let base_pos = MOON_BASE_POSITIONS[vertex_index % 4];
    out.pos = mul(render_info.sky_matrix, float4(base_pos, 1.0));
    out.pos.y = -out.pos.y;
    out.uvs = calculate_moon_uvs(int32_t(render_info.time_of_day), vertex_index % 4);
    return out;
}

[shader("fragment")]
float4 fragment(VertexOutput in) : SV_Target {
    return moon_phases_image.Sample(in.uvs);
}
