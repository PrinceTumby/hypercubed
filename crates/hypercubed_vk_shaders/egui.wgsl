@group(0) @binding(0)
var<uniform> screen_size: vec2<f32>;

@group(1) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(1) @binding(1)
var s_diffuse: sampler;

struct VertexInput {
    @location(0) pos: vec2<f32>,
    @location(1) uvs: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uvs: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn egui_vertex(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let screen_pos = in.pos / screen_size;
    out.clip_pos = vec4(
        screen_pos.x * 2.0 - 1.0,
        1.0 - (screen_pos.y * 2.0),
        0.0,
        1.0,
    );
    out.uvs = in.uvs;
    out.color = in.color;
    return out;
}

@fragment
fn egui_fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_sample = textureSample(t_diffuse, s_diffuse, in.uvs);
    return vec4(tex_sample * in.color);
    // return vec4(in.uvs, 0.0, 1.0);
    // let color = vec3(in.index / 100.0);
    // return vec4(color, 1.0);
}
