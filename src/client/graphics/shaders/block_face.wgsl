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
    @builtin(vertex_index) index: u32,
    @location(0) subchunk_start_coords: vec3<f32>,
    @location(1) face_matrix_index: u32,
}

struct InstanceInput {
    @location(10) uvs: vec4<u32>,
    @location(11) packed_xyz_and_matrix_indices: vec2<u32>,
    @location(12) uv_rotation_and_padding: vec2<u32>,
}

struct VertexOutput {
    @invariant
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uvs: vec2<f32>,
}

const base_normal: vec3<f32> = vec3(0.0, 1.0, 0.0);

fn get_base_position(vertex_i: u32) -> vec3<f32> {
    var out: vec3<f32>;
    switch vertex_i % 4 {
        case 0u: {
            out = vec3<f32>(-0.5, 0.5, 0.5);
        }
        case 1u: {
            out = vec3<f32>(0.5, 0.5, 0.5);
        }
        case 2u: {
            out = vec3<f32>(-0.5, 0.5, -0.5);
        }
        case 3u, default: {
            out = vec3<f32>(0.5, 0.5, -0.5);
        }
    }
    return out;
}

fn get_uvs(vertex_i: u32, face_rotation_i: u32) -> vec2<f32> {
    var out: vec2<f32>;
    var uv_rotation_vec: vec4<u32>;
    switch face_rotation_i {
        case 0u: {
            uv_rotation_vec = vec4<u32>(0, 1, 2, 3);
        }
        case 1u: {
            uv_rotation_vec = vec4<u32>(1, 3, 0, 2);
        }
        case 2u: {
            uv_rotation_vec = vec4<u32>(3, 2, 1, 0);
        }
        case 3u, default: {
            uv_rotation_vec = vec4<u32>(2, 0, 3, 1);
        }
    }
    let new_vertex_i = uv_rotation_vec[vertex_i % 4];
    switch new_vertex_i {
        case 0u: {
            out = vec2<f32>(0.0, 1.0);
        }
        case 1u: {
            out = vec2<f32>(1.0, 1.0);
        }
        case 2u: {
            out = vec2<f32>(0.0, 0.0);
        }
        case 3u, default: {
            out = vec2<f32>(1.0, 0.0);
        }
    }
    return out;
}

@vertex
fn vs_main(
    block_vertex: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;
    // Unpack instance data
    let xyz_offset = vec3<f32>(
        f32(instance.packed_xyz_and_matrix_indices[0] & 0xF),
        f32((instance.packed_xyz_and_matrix_indices[0] >> 4) & 0xF),
        f32(instance.packed_xyz_and_matrix_indices[1] & 0xF),
    );
    let matrix_indices = vec2<u32>(
        (instance.packed_xyz_and_matrix_indices[1] >> 4) & 0x3,
        (instance.packed_xyz_and_matrix_indices[1] >> 6) & 0x3,
    );
    // Position
    let base_pos = get_base_position(block_vertex.index);
    let base_uvs = get_uvs(block_vertex.index, instance.uv_rotation_and_padding[0]);
    let face_matrix = face_matrices[block_vertex.face_matrix_index];
    let x_rotation_matrix = x_rotation_matrices[matrix_indices[0]];
    let y_rotation_matrix = y_rotation_matrices[matrix_indices[1]];
    let combined_matrix = y_rotation_matrix * x_rotation_matrix * face_matrix;
    let local_pos = combined_matrix * base_pos;
    let block_centre_pos = block_vertex.subchunk_start_coords + xyz_offset;
    let global_pos = local_pos + block_centre_pos + vec3(0.5, 0.5, 0.5);
    out.clip_pos = view_matrix * vec4(global_pos, 1.0);
    // UVs
    let start_uvs = vec2<f32>(instance.uvs.xy) / atlas_size;
    let end_uvs = vec2<f32>(instance.uvs.zw) / atlas_size;
    out.uvs = mix(start_uvs, end_uvs, base_uvs);
    // Normal
    out.normal = combined_matrix * base_normal;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_sample = textureSample(t_diffuse, s_diffuse, in.uvs);
    let light_source_dir = normalize(vec3(2.0, 5.0, 1.0));
    let lighting = dot(in.normal, light_source_dir);
    let light_coef = fma(lighting, 0.3, 0.4);
    return vec4(tex_sample.rgb * light_coef, 1.0);
}
