import package::common::render_info;

@group(1) @binding(0)
var egui_texture: texture_2d<f32>;
@group(1) @binding(1)
var egui_sampler: sampler;

struct VertexInput {
    @location(0) pos: vec2<f32>,
    @location(1) uvs: vec2<f32>,
    @location(2) colour: vec4<f32>,
}

struct VertexOutput {
    @invariant
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uvs: vec2<f32>,
    @location(1) colour: vec4<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let screen_pos = in.pos * render_info.recip_screen_size;
    out.clip_pos = vec4(
        screen_pos.x * 2.0 - 1.0,
        1.0 - (screen_pos.y * 2.0),
        0.0,
        1.0,
    );
    out.uvs = in.uvs;
    out.colour = in.colour;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_sample = textureSample(egui_texture, egui_sampler, in.uvs);
    return vec4(tex_sample * in.colour);
}
