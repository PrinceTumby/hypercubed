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
@group(2) @binding(1)
var<uniform> x_rotation_matrices: array<mat3x3<f32>, 4>;
@group(2) @binding(2)
var<uniform> y_rotation_matrices: array<mat3x3<f32>, 4>;

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
    @location(11) matrix_indices: vec4<u32>,
    @location(12) tint_color: vec4<f32>,
}

struct VertexOutput {
    @invariant
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uvs: vec2<f32>,
    @location(2) tint_color: vec4<f32>,
    @location(3) tint_percentage: f32,
}

fn quantize_vec3_16k(vec: vec3<f32>) -> vec3<f32> {
    let quantize_amount: f32 = 65536.0;
    return round(vec * vec3(quantize_amount)) / vec3(quantize_amount);
}

@vertex
fn vs_main(
    vertex: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;
    let x_rotation_matrix = x_rotation_matrices[instance.matrix_indices[0]];
    let y_rotation_matrix = y_rotation_matrices[instance.matrix_indices[1]];
    let rotation_matrix = y_rotation_matrix * x_rotation_matrix;
    // Position
    // let rotated_pos = rotation_matrix * quantize_vec3_16k(vertex.local_pos);
    let rotated_pos = rotation_matrix * vertex.local_pos;
    let global_pos = rotated_pos + instance.pos + vec3(0.5, 0.5, 0.5);
    out.clip_pos = view_matrix * vec4(global_pos, 1.0);
    // UVs
    out.uvs = vec2<f32>(vertex.uvs) / atlas_size;
    // Normal
    out.normal = rotation_matrix * vertex.normal;
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
