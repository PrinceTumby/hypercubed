@group(0) @binding(0)
var<uniform> view_matrix: mat4x4<f32>;

// Fraction of each texture atlas dimension that each square is.
// Calculated as `square_length / texture_atlas_dims`
@group(1) @binding(0)
var<uniform> atlas_size: vec2<f32>;
@group(1) @binding(1)
var t_diffuse: texture_2d<f32>;
@group(1) @binding(2)
var s_diffuse: sampler;

@group(2) @binding(0)
var<uniform> face_matrices: array<mat3x3<f32>, 6>;

struct VertexInput {
    @location(0) local_pos: vec3<f32>,
    @location(1) uvs: vec2<u32>,
    @location(2) normal: vec3<f32>,
    @location(3) tint_percentage: f32,
}

// struct VertexInput {
//     @location(0) pos: vec3<f32>,
//     @location(1) uvs: vec2<u32>,
//     @location(2) normal: vec3<f32>,
//     @location(3) tint: vec4<u32>,
// }

struct InstanceInput {
    @location(10) pos: vec3<f32>,
    @location(11) tint_color: vec4<f32>,
}

struct VertexOutput {
    @invariant
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uvs: vec2<f32>,
    @location(2) tint_color: vec4<f32>,
    @location(3) tint_percentage: f32,
}

@vertex
fn vs_main(
    vertex: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;
    // Position
    let global_pos = vertex.local_pos + instance.pos + vec3(0.5, 0.5, 0.5);
    out.clip_pos = view_matrix * vec4(global_pos, 1.0);
    // UVs
    out.uvs = vec2<f32>(vertex.uvs) / atlas_size;
    // Normal
    out.normal = vertex.normal;
    // Tint
    out.tint_color = instance.tint_color;
    out.tint_percentage = vertex.tint_percentage;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var tex_sample = textureSample(t_diffuse, s_diffuse, in.uvs);
    if tex_sample.a < 1.0 {
        discard;
    }
    tex_sample *= mix(vec4(1.0, 1.0, 1.0, 1.0), in.tint_color, in.tint_percentage);
    let light_source_dir = normalize(vec3(2.0, 5.0, 1.0));
    let lighting = dot(in.normal, light_source_dir);
    let light_coef = fma(lighting, 0.3, 0.4);
    return vec4(tex_sample.rgb * light_coef, 1.0);
}
