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
    @location(0) local_pos: vec3<f32>,
    @location(1) uvs: vec2<u32>,
    @location(2) normal: vec3<f32>,
    /// 0: Tinted?
    /// 1-31: Unused
    @location(3) packed_fields: u32,
}

// TODO:
// - Pack in 7 light level pairs
// - Vertex shader: find closest two cubes, interpolate between them
struct InstanceInput {
    @location(4) pos: vec3<f32>,
    @location(5) tint_color: vec4<f32>,
    @location(6) light_level_pairs_1: vec4<u32>,
    /// Packed fields:
    /// 0: Emits light?
    /// 1-31: Unused
    @location(7) light_level_pairs_2_and_packed_fields: vec4<u32>,
}

struct VertexOutput {
    @invariant
    @builtin(position) clip_pos: vec4<f32>,
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
fn custom_block_vertex(
    vertex: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;
    // Position
    let global_pos = vertex.local_pos + instance.pos + vec3(0.5, 0.5, 0.5);
    out.clip_pos = view_matrix * vec4(global_pos, 1.0);
    // UVs
    let atlas_size_f32s = vec2<f32>(textureDimensions(atlas_texture));
    out.uvs = vec2<f32>(vertex.uvs) / atlas_size_f32s;
    // Normal
    out.normal = vertex.normal;
    // Tint
    out.tint_color = instance.tint_color;
    out.tint_percentage = f32(vertex.packed_fields & 1u);
    // Light RGB
    {
        let adjusted_pos = fma(vertex.normal, vec3(0.02), vertex.local_pos);
        var light_pair_positions = array(
            vec3(0.0, 0.0, 0.0),
            vec3(0.0, 1.01, 0.0),
            vec3(0.0, -1.01, 0.0),
            vec3(0.0, 0.0, -1.01),
            vec3(0.0, 0.0, 1.01),
            vec3(0.0, 1.01, 0.0),
            vec3(0.0, -1.01, 0.0),
        );
        let light_level_pairs_2 = instance.light_level_pairs_2_and_packed_fields;
        var light_pairs = array(
            vec2(instance.light_level_pairs_1[0] & 0xF, instance.light_level_pairs_1[0] >> 4),
            vec2(instance.light_level_pairs_1[1] & 0xF, instance.light_level_pairs_1[1] >> 4),
            vec2(instance.light_level_pairs_1[2] & 0xF, instance.light_level_pairs_1[2] >> 4),
            vec2(instance.light_level_pairs_1[3] & 0xF, instance.light_level_pairs_1[3] >> 4),
            vec2(light_level_pairs_2[0] & 0xF, light_level_pairs_2[0] >> 4),
            vec2(light_level_pairs_2[1] & 0xF, light_level_pairs_2[1] >> 4),
            vec2(light_level_pairs_2[2] & 0xF, light_level_pairs_2[2] >> 4),
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
fn custom_block_fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var tex_sample = textureSample(atlas_texture, atlas_sampler, in.uvs);
    if tex_sample.a < 1.0 {
        discard;
    }
    tex_sample *= mix(vec4(1.0, 1.0, 1.0, 1.0), in.tint_color, in.tint_percentage);
    let light_source_dir = normalize(vec3(2.0, 5.0, 1.0));
    let lighting = dot(in.normal, light_source_dir);
    let light_coef = fma(lighting, 0.3, 0.4);
    return vec4(tex_sample.rgb * light_coef * in.light_rgb, 1.0);
}
