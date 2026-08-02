#language slang 2026

module triangle;

import "../_common";
import _common;

struct VertexInput {
    [[vk::location(0)]] float3 p1;
    [[vk::location(1)]] float3 p2;
    [[vk::location(2)]] float3 p3;
    [[vk::location(3)]] float4 colour;
    [[vk::location(4)]] PackedFields packed_fields;
    uint vertex_index : SV_VulkanVertexID;
}

struct VertexOutput {
    float4 pos : SV_Position;
    nointerpolation float4 colour;
}

[shader("vertex")]
VertexOutput vertex(VertexInput in) {
    VertexOutput out;
    float3 vertex_pos;
    switch (in.vertex_index) {
        case 0:
            vertex_pos = in.p1;
            break;
        case 1:
            vertex_pos = in.p2;
            break;
        default:
            vertex_pos = in.p3;
    }
    out.pos = mul(render_info.view_matrix, float4(vertex_pos, 1.0));
    if (in.packed_fields.ignore_depth) {
        out.pos.z = sign(out.pos.z);
    }
    out.pos.y = -out.pos.y;
    out.colour = in.colour;
    return out;
}

[shader("fragment")]
float4 fragment(VertexOutput in) {
    return in.colour;
}
