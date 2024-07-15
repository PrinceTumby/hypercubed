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
    @location(0) pos: vec3<f32>,
    @location(1) uvs: vec2<f32>,
    @location(2) normal: vec3<f32>,
}

struct InstanceInput {
    @location(10) matrix_0: vec4<f32>,
    @location(11) matrix_1: vec4<f32>,
    @location(12) matrix_2: vec4<f32>,
    @location(13) matrix_3: vec4<f32>,
    @location(14) pos: vec3<f32>,
    @location(15) uvs: vec4<u32>,
    @location(16) matrix_indices: vec4<u32>,
}

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uvs: vec2<f32>,
}

@vertex
fn vs_main(
    block_vertex: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;
    // Position
    let face_matrix = face_matrices[instance.matrix_indices[0]];
    let x_rotation_matrix = x_rotation_matrices[instance.matrix_indices[1]];
    let y_rotation_matrix = y_rotation_matrices[instance.matrix_indices[2]];
    let instance_matrix = mat4x4(
        instance.matrix_0,
        instance.matrix_1,
        instance.matrix_2,
        instance.matrix_3,
    );
    let face_pos = face_matrix * block_vertex.pos;
    let transformed_pos = instance_matrix * vec4(face_pos, 1.0);
    let rotated_pos = y_rotation_matrix * x_rotation_matrix * transformed_pos.xyz;
    let global_pos = rotated_pos + instance.pos + vec3(0.5, 0.5, 0.5);
    out.clip_pos = view_matrix * vec4(global_pos, 1.0);
    // UVs
    let start_uvs = vec2<f32>(instance.uvs.xy) / atlas_size;
    let end_uvs = vec2<f32>(instance.uvs.zw) / atlas_size;
    out.uvs = mix(start_uvs, end_uvs, block_vertex.uvs);
    // Normal
    let face_normal = face_matrix * block_vertex.normal;
    out.normal = y_rotation_matrix * x_rotation_matrix * face_matrix * block_vertex.normal;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var tex_sample = textureSample(t_diffuse, s_diffuse, in.uvs);
    if tex_sample.a < 1.0 {
        discard;
    }
    let light_source_dir = normalize(vec3(2.0, 5.0, 1.0));
    let lighting = dot(in.normal, light_source_dir);
    let light_coef = fma(lighting, 0.3, 0.4);
    return vec4(tex_sample.rgb * light_coef, 1.0);
}