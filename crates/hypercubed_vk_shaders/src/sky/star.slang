#language slang 2026

module star;

import "../_common";

struct InstanceInput {
    [[vk::location(0)]] float3 p1;
    [[vk::location(1)]] float3 p2;
    [[vk::location(2)]] float3 p3;
    [[vk::location(3)]] float3 p4;
}

struct VertexOutput {
    float4 pos : SV_Position;
    nointerpolation float star_brightness;
}

[shader("vertex")]
VertexOutput vertex(InstanceInput instance, uint vertex_index : SV_VulkanVertexID) {
    VertexOutput out;
    float3 global_pos;
    switch (vertex_index % 4) {
        case 0:
            global_pos = instance.p1;
            break;
        case 1:
            global_pos = instance.p2;
            break;
        case 2:
            global_pos = instance.p3;
            break;
        default:
            global_pos = instance.p4;
    }
    out.pos = mul(render_info.sky_matrix, float4(global_pos, 1.0));
    out.pos.y = -out.pos.y;
    out.star_brightness = render_info.star_brightness;
    return out;
}

[shader("fragment")]
float4 fragment(VertexOutput in) : SV_Target {
    return float4(float3(in.star_brightness), 1.0);
}
