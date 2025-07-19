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

@group(3) @binding(0)
var<storage, read> lightmap: array<array<u32, 128>>;

struct VertexInput {
    @builtin(vertex_index) index: u32,
    @location(0) subchunk_start_coords: vec3<f32>,
    @location(1) face_matrix_index: u32,
}

struct InstanceInput {
    @builtin(instance_index) index: u32,
    @location(10) uvs: vec4<u32>,
    /// 0-3: X offset
    /// 4-7: Y offset
    /// 8-11: Z offset
    /// 12-13: UV rotation
    /// 14: Emits light?
    /// 17-19: Unused
    /// 20-23: Sky light level
    /// 24-27: Block light level
    /// 28-31: Unused
    @location(11) packed_fields: u32,
}

struct VertexOutput {
    @invariant @builtin(position) clip_pos: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uvs: vec2<f32>,
    // @interpolate(flat)
    // @location(2) light_rgb: vec3<f32>,
    @location(2) face_xy: vec2<f32>,
    @interpolate(flat)
    @location(3) instance_i: u32,
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

fn get_face_xy(vertex_i: u32) -> vec2<f32> {
    var out: vec2<f32>;
    switch vertex_i % 4 {
        case 0u: {
            out = vec2<f32>(0.0, 0.0);
        }
        case 1u: {
            out = vec2<f32>(1.0, 0.0);
        }
        case 2u: {
            out = vec2<f32>(0.0, 1.0);
        }
        case 3u, default: {
            out = vec2<f32>(1.0, 1.0);
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

fn hsv448_to_rgb(packed_hsv: u32) -> vec3<f32> {
    let hsv = vec3<f32>(
        f32(packed_hsv & 0xFu) / 15.0,
        f32((packed_hsv >> 4u) & 0xFu) / 15.0,
        f32((packed_hsv >> 8u) & 0xFFu) / 255.0,
    );
    let k = vec4(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    let p = abs(fract(hsv.xxx + k.xyz) * 6.0 - k.www);
    return mix(k.xxx, clamp(p - k.xxx, vec3(0.0), vec3(1.0)), hsv.y) * hsv.z;
}

@vertex
fn vs_main(
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
    let start_uvs = vec2<f32>(instance.uvs.xy) / atlas_size;
    let end_uvs = vec2<f32>(instance.uvs.zw) / atlas_size;
    out.uvs = mix(start_uvs, end_uvs, base_uvs);
    // Normal
    out.normal = face_matrix * base_normal;
    // Light RGB
    // out.light_rgb = calculate_light_rgb(sky_light_level, block_light_level);
    out.face_xy = get_face_xy(block_vertex.index);
    out.instance_i = instance.index;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_sample = textureSample(t_diffuse, s_diffuse, in.uvs);
    let light_source_dir = normalize(vec3(2.0, 5.0, 1.0));
    let lighting = dot(in.normal, light_source_dir);
    let light_coef = fma(lighting, 0.3, 0.4);
    // Load lightmap pixel colour
    let light_uv_coords = floor(min(in.face_xy * 16.0, vec2(15.9)));
    let closest_probe_i = (u32(light_uv_coords.y) << 4u)
        | u32(light_uv_coords.x);
    let pixel_pair = lightmap[in.instance_i][closest_probe_i / 2];
    let shift_amount = (closest_probe_i & 1u) * 16u;
    let packed_hsv = (pixel_pair >> shift_amount) & 0xFFFFu;
    let light_rgb = hsv448_to_rgb(packed_hsv);
    // let light_rgb = vec3(1.0);
    return vec4(tex_sample.rgb * light_coef * light_rgb, 1.0);
}
