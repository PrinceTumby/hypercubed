@group(0) @binding(0)
var<uniform> view_matrix: mat4x4<f32>;

// Fraction of each texture atlas dimension that each square is.
// Calculated as `square_length / texture_atlas_dims`
@group(1) @binding(0)
var atlas_texture: texture_2d<f32>;
@group(1) @binding(1)
var atlas_sampler: sampler;

@group(2) @binding(0)
var<uniform> face_matrices: array<mat3x3<f32>, 6>;

struct VertexInput {
    @builtin(vertex_index) index: u32,
    @location(0) subchunk_start_coords: vec3<f32>,
    @location(1) face_matrix_index: u32,
}

struct InstanceInput {
    @location(2) uvs: vec4<u32>,
    @location(3) tint_color: vec4<f32>,
    /// 0-3: X offset
    /// 4-7: Y offset
    /// 8-11: Z offset
    /// 12-13: UV rotation
    /// 14: Emits light?
    /// 17-19: Unused
    /// 20-23: Sky light level
    /// 24-27: Block light level
    /// 28-31: Unused
    @location(4) packed_fields: u32,
}

struct VertexOutput {
    @invariant
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uvs: vec2<f32>,
    @interpolate(flat)
    @location(2) light_rgb: vec3<f32>,
    @interpolate(flat)
    @location(3) tint_color: vec4<f32>,
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
    var out: vec2<f32>;
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

fn calculate_light_rgb(sky_light_level: u32, block_light_level: u32) -> vec3<f32> {
    let light_percentage = clamp(
        f32(max(sky_light_level, block_light_level)) / 14.0,
        0.001,
        1.0
    );
    let gamma = 0.5;
    let light_gamma = pow(light_percentage, 1.0 / gamma);
    return mix(vec3(0.02), vec3(1.0), light_gamma);
}

@vertex
fn tinted_block_face_vertex(
    block_vertex: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;
    // Unpack instance data
    let xyz_offset = vec3<f32>(
        f32(instance.packed_fields & 0xFu),
        f32((instance.packed_fields >> 4u) & 0xFu),
        f32((instance.packed_fields >> 8u) & 0xFu),
    );
    let sky_light_level = (instance.packed_fields >> 20u) & 0xFu;
    let block_light_level = (instance.packed_fields >> 24u) & 0xFu;
    // Position
    let base_pos = get_base_position(block_vertex.index);
    let face_rotation_i = (instance.packed_fields >> 12u) & 0x3u;
    let base_uvs = get_uvs(block_vertex.index, face_rotation_i);
    let face_matrix = face_matrices[block_vertex.face_matrix_index];
    let local_pos = face_matrix * base_pos;
    let block_centre_pos = block_vertex.subchunk_start_coords + xyz_offset;
    let global_pos = local_pos + block_centre_pos + vec3(0.5, 0.5, 0.5);
    out.clip_pos = view_matrix * vec4(global_pos, 1.0);
    // UVs
    let atlas_size_f32s = vec2<f32>(textureDimensions(atlas_texture));
    let start_uvs = vec2<f32>(instance.uvs.xy) / atlas_size_f32s;
    let end_uvs = vec2<f32>(instance.uvs.zw) / atlas_size_f32s;
    out.uvs = mix(start_uvs, end_uvs, base_uvs);
    // Normal
    out.normal = face_matrix * base_normal;
    // Light RGB
    out.light_rgb = calculate_light_rgb(sky_light_level, block_light_level);
    // Tint Color
    out.tint_color = instance.tint_color;
    return out;
}

@fragment
fn tinted_block_face_fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var tex_sample = textureSample(atlas_texture, atlas_sampler, in.uvs);
    if tex_sample.a < 1.0 {
        discard;
    }
    tex_sample *= in.tint_color;
    let light_source_dir = normalize(vec3(2.0, 5.0, 1.0));
    let lighting = dot(in.normal, light_source_dir);
    let light_coef = fma(lighting, 0.3, 0.4);
    return vec4(tex_sample.rgb * light_coef * in.light_rgb, 1.0);
}
