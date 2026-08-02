#language slang 2026

module egui;

import _common;

[[vk::binding(0, 1)]]
Sampler2D image;

struct VertexInput {
    [[vk::location(0)]] float2 pos;
    [[vk::location(1)]] float2 uvs;
    [[vk::location(2)]] float4 colour;
}

struct VertexOutput {
    float4 pos : SV_Position;
    float2 uvs;
    float4 colour;
}

[shader("vertex")]
VertexOutput vertex(VertexInput in) {
    VertexOutput out;
    let screen_pos = in.pos * render_info.recip_screen_size;
    out.pos = float4(screen_pos.x * 2.0 - 1.0, screen_pos.y * 2.0 - 1.0, 0.0, 1.0);
    out.uvs = in.uvs;
    out.colour = in.colour;
    return out;
}

[shader("fragment")]
float4 fragment(VertexOutput in) : SV_Target {
    return image.Sample(in.uvs) * in.colour;
}
