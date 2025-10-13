use super::block_face::calculate_light_rgb;
use super::types::CustomBlockVertexFieldsGpu;
use spirv_std::glam::{Mat4, UVec2, UVec4, Vec2, Vec3, Vec4, Vec4Swizzles};
use spirv_std::image::Image2d;
#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::Float;
use spirv_std::{spirv, Sampler};

// NOTE: The code here was mostly written in a way that makes Rust GPU happy, hopefully it can be
//       rewritten to be more idiomatic as Rust GPU improves and is able to compile more code.

#[spirv(vertex)]
pub fn vertex(
    // Bindings
    #[spirv(uniform, descriptor_set = 0, binding = 0)] view_matrix: &Mat4,
    #[spirv(uniform, descriptor_set = 1, binding = 2)] block_item_atlas_size: &Vec2,
    // Vertex inputs
    in_vertex_pos: Vec3,
    in_uvs: UVec2,
    in_normal: Vec3,
    in_vertex_packed_fields: CustomBlockVertexFieldsGpu,
    // Instance inputs
    // #[spirv(instance_index)]
    // in_instance_index: u32,
    in_instance_pos: Vec3,
    in_tint_colour: Vec4,
    in_light_level_pairs_1: UVec4,
    in_light_level_pairs_2_and_packed_fields: UVec4,
    // Outputs
    #[spirv(invariant, position)] out_pos: &mut Vec4,
    out_normal: &mut Vec3,
    out_uvs: &mut Vec2,
    out_tint_colour: &mut Vec4,
    out_tint_percentage: &mut f32,
    out_light_rgb: &mut Vec3,
) {
    let light_pairs = [
        in_light_level_pairs_1.x,
        in_light_level_pairs_1.y,
        in_light_level_pairs_1.z,
        in_light_level_pairs_1.w,
        in_light_level_pairs_2_and_packed_fields.x,
        in_light_level_pairs_2_and_packed_fields.y,
        in_light_level_pairs_2_and_packed_fields.z,
    ];
    // Position
    let global_pos = in_vertex_pos + in_instance_pos + Vec3::splat(0.5);
    *out_pos = Vec4::new(1.0, -1.0, 1.0, 1.0) * (*view_matrix * Vec4::from((global_pos, 1.0)));
    // UVs
    *out_uvs = in_uvs.as_vec2() / *block_item_atlas_size;
    // Normal
    *out_normal = in_normal;
    // Tint
    *out_tint_colour = in_tint_colour;
    *out_tint_percentage = in_vertex_packed_fields.tinted_bit() as f32;
    // Light RGB
    {
        let adjusted_pos = Vec3::mul_add(in_normal, Vec3::splat(0.02), in_vertex_pos);
        let light_pair_positions = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 1.01, 0.0),
            Vec3::new(0.0, -1.01, 0.0),
            Vec3::new(0.0, 0.0, -1.01),
            Vec3::new(0.0, 0.0, 1.01),
            Vec3::new(0.0, 1.01, 0.0),
            Vec3::new(0.0, -1.01, 0.0),
        ];
        let mut closest_dist: f32 = f32::INFINITY;
        let mut closest_light_pair_idx: usize = 0;
        for i in 0..7 {
            let dist = Vec3::distance_squared(adjusted_pos, light_pair_positions[i]);
            let is_closer = dist < closest_dist;
            closest_dist = if is_closer { dist } else { closest_dist };
            closest_light_pair_idx = if is_closer { i } else { closest_light_pair_idx };
        }
        let closest_light_pair = light_pairs[closest_light_pair_idx];
        *out_light_rgb = calculate_light_rgb(closest_light_pair & 0xF, closest_light_pair >> 4);
    }
}

#[spirv(fragment)]
pub fn fragment(
    // Bindings
    #[spirv(descriptor_set = 1, binding = 0)] block_item_atlas: &Image2d,
    #[spirv(descriptor_set = 1, binding = 1)] block_item_atlas_sampler: &Sampler,
    // Inputs
    in_normal: Vec3,
    in_uvs: Vec2,
    in_tint_colour: Vec4,
    in_tint_percentage: f32,
    in_light_rgb: Vec3,
    // Outputs
    out_colour: &mut Vec4,
) {
    let tex_sample = block_item_atlas.sample(*block_item_atlas_sampler, in_uvs);
    if tex_sample.w < 1.0 {
        spirv_std::arch::kill();
    }
    let tex_sample = tex_sample * Vec4::lerp(Vec4::splat(1.0), in_tint_colour, in_tint_percentage);
    let light_source_dir = Vec3::new(2.0, 5.0, 1.0).normalize();
    let lighting = Vec3::dot(in_normal, light_source_dir);
    let light_coef = f32::mul_add(lighting, 0.3, 0.4);
    *out_colour = Vec4::from((tex_sample.xyz() * light_coef * in_light_rgb, 1.0));
}
