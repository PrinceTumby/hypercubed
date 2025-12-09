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
var<storage, read> custom_block_faces: array<array<Vertex, 4>>;

const BLOCK_FACE_INDICES: array<u32, 6> = array(1, 0, 2, 3, 1, 2);

struct Vertex {
    local_pos: array<f32, 3>,
    packed_uvs: u32,
    normal: array<f32, 3>,
    tint_percentage: f32,
}

// TODO:
// - Pack in 7 light level pairs
// - Vertex shader: find closest two cubes, interpolate between them
struct InstanceInput {
    @location(10) pos: vec3<f32>,
    @location(11) tint_color: vec4<f32>,
    @location(12) light_level_pairs_1: vec4<u32>,
    @location(13) light_level_pairs_2: vec4<u32>,
}

struct VertexOutput {
    @builtin(position) @invariant clip_pos: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uvs: vec2<f32>,
    @location(2) tint_color: vec4<f32>,
    @location(3) tint_percentage: f32,
    @location(4) light_rgb: vec3<f32>,
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
fn vs_main(
    @builtin(vertex_index) vertex_i: u32,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;
    // Pull vertex from faces buffer.
    let face_i = vertex_i / 6;
    let face_vertex_i = BLOCK_FACE_INDICES[vertex_i % 6];
    let vertex = custom_block_faces[face_i][face_vertex_i];
    let vertex_local_pos = vec3(vertex.local_pos[0], vertex.local_pos[1], vertex.local_pos[2]);
    let vertex_normal = vec3(vertex.normal[0], vertex.normal[1], vertex.normal[2]);
    let vertex_uvs = vec2(
        vertex.packed_uvs & 0xFFFFu,
        (vertex.packed_uvs >> 16u) & 0xFFFFu,
    );
    // Position
    let global_pos = vertex_local_pos + instance.pos + vec3(0.5);
    out.clip_pos = view_matrix * vec4(global_pos, 1.0);
    // UVs
    out.uvs = vec2<f32>(vertex_uvs) / atlas_size;
    // Normal
    out.normal = vertex_normal;
    // Tint
    out.tint_color = instance.tint_color;
    out.tint_percentage = vertex.tint_percentage;
    // Light RGB
    {
        let adjusted_pos = fma(vertex_normal, vec3(0.02), vertex_local_pos);
        var light_pair_positions = array(
            vec3(0.0, 0.0, 0.0),
            vec3(0.0, 1.01, 0.0),
            vec3(0.0, -1.01, 0.0),
            vec3(0.0, 0.0, -1.01),
            vec3(0.0, 0.0, 1.01),
            vec3(0.0, 1.01, 0.0),
            vec3(0.0, -1.01, 0.0),
        );
        var light_pairs = array(
            vec2(instance.light_level_pairs_1.x & 0xF, instance.light_level_pairs_1.x >> 4u),
            vec2(instance.light_level_pairs_1.y & 0xF, instance.light_level_pairs_1.y >> 4u),
            vec2(instance.light_level_pairs_1.z & 0xF, instance.light_level_pairs_1.z >> 4u),
            vec2(instance.light_level_pairs_1.w & 0xF, instance.light_level_pairs_1.w >> 4u),
            vec2(instance.light_level_pairs_2.x & 0xF, instance.light_level_pairs_2.x >> 4u),
            vec2(instance.light_level_pairs_2.y & 0xF, instance.light_level_pairs_2.y >> 4u),
            vec2(instance.light_level_pairs_2.z & 0xF, instance.light_level_pairs_2.z >> 4u),
        );
        var closest_dist: f32 = 1000000.0;
        var closest_light_pair_idx: u32;
        for (var i: u32 = 0; i < 7; i++) {
            let dist = distance(adjusted_pos, light_pair_positions[i]);
            let is_closer = dist < closest_dist;
            closest_dist = select(closest_dist, dist, is_closer);
            closest_light_pair_idx = select(closest_light_pair_idx, i, is_closer);
        }
        let closest_light_pair = light_pairs[closest_light_pair_idx];
        out.light_rgb = calculate_light_rgb(closest_light_pair[0], closest_light_pair[1]);
    }
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
    let light_coef = fma(lighting, 0.3, 0.7);
    return vec4(tex_sample.rgb * light_coef * in.light_rgb, 1.0);
}
