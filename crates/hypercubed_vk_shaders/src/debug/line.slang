#language slang 2026

module line;

import "../_common";
import _common;

struct VertexInput {
    [[vk::location(0)]] float3 p1;
    [[vk::location(1)]] float3 p2;
    [[vk::location(2)]] float size;
    [[vk::location(3)]] float4 colour;
    [[vk::location(4)]] PackedFields packed_fields;
    uint vertex_index : SV_VulkanVertexID;
}

struct VertexOutput {
    float4 pos : SV_Position;
    nointerpolation float4 colour;
    nointerpolation PackedFields packed_fields;
}

[shader("vertex")]
VertexOutput vertex(VertexInput in) {
    VertexOutput out;
    let p1_clip_pos = mul(render_info.view_matrix, float4(in.p1, 1.0));
    let p2_clip_pos = mul(render_info.view_matrix, float4(in.p2, 1.0));
    let clip_diff_norm = normalize(p2_clip_pos.xy - p1_clip_pos.xy);
    // Calculate the perpendicular vector offset.
    let offset_right = clip_diff_norm.yx * float2(-1, 1) * in.size * render_info.recip_screen_size;
    let offset = select((in.vertex_index % 2) == 0, offset_right, -offset_right);
    if ((in.vertex_index / 2) != 0) {
        out.pos = p1_clip_pos;
    } else {
        out.pos = p2_clip_pos;
    }
    out.pos += float4(offset * out.pos.w, 0.0, 0.0);
    out.pos.y = -out.pos.y;
    out.colour = in.colour;
    out.packed_fields = in.packed_fields;
    return out;
}

internal struct FragmentOutput {
    [[vk::location(0)]] float4 colour;
    float depth : SV_Depth;
}

[shader("fragment")]
FragmentOutput fragment(VertexOutput in) {
    FragmentOutput out;
    if (in.packed_fields.ignore_depth) {
        out.depth = 1.0;
    } else {
        out.depth = in.pos.z;
    }
    out.colour = float4(in.colour.xyz * in.colour.w, in.colour.w);
    return out;
}
