#language slang 2026

module point;

import "../_common";
import _common;

struct VertexInput {
    [[vk::location(0)]] float3 pos;
    [[vk::location(1)]] float4 colour;
    [[vk::location(2)]] float size;
    [[vk::location(3)]] PackedFields packed_fields;
}

struct VertexOutput {
    float4 pos : SV_Position;
    float size;
    float4 colour;
}

[shader("vertex")]
VertexOutput vertex(VertexInput in) {
    VertexOutput out;
    out.pos = mul(render_info.view_matrix, float4(in.pos, 1.0));
    out.pos.y = -out.pos.y;
    if (in.packed_fields.ignore_depth) {
        out.pos.z = sign(in.pos.z);
    }
    out.size = in.size;
    out.colour = in.colour;
    return out;
}

[shader("fragment")]
float4 fragment(VertexOutput in) {
    return in.colour;
}
