use super::block_face::calculate_light_rgb;
use super::types::CustomBlockVertex;
use spirv_std::glam::{Mat4, UVec2, UVec4, Vec2, Vec3, Vec4, Vec4Swizzles};
use spirv_std::image::Image2d;
#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::Float;
use spirv_std::{Sampler, spirv};

// NOTE: The code here was mostly written in a way that makes Rust GPU happy, hopefully it can be
//       rewritten to be more idiomatic as Rust GPU improves and is able to compile more code.

const BLOCK_FACE_INDICES: [usize; 6] = [1, 0, 2, 3, 1, 2];

#[spirv(vertex)]
pub fn vertex(
    // Bindings
    #[spirv(uniform, descriptor_set = 0, binding = 0)] view_matrix: &Mat4,
    #[spirv(uniform, descriptor_set = 1, binding = 2)] block_item_atlas_size: &Vec2,
    #[spirv(storage_buffer, descriptor_set = 3, binding = 0)] custom_block_faces: &[[CustomBlockVertex;
          4]],
    // Vertex inputs
    #[spirv(vertex_index)] in_vertex_index: u32,
    // Instance inputs
    // #[spirv(instance_index)]
    // in_instance_index: u32,
    in_instance_pos: Vec3,
    in_tint_colour: Vec4,
    in_light_level_pairs_1: UVec4,
    in_light_level_pairs_2_and_packed_fields: UVec4,
    // Outputs
    #[spirv(invariant, position)] out_pos: &mut Vec4,
    out_uvs: &mut Vec2,
    out_light_rgb: &mut Vec3,
) {
    // Pull vertex from faces buffer.
    let face_i = in_vertex_index as usize / 6;
    let face_vertex_i = BLOCK_FACE_INDICES[in_vertex_index as usize % 6];
    let in_vertex = custom_block_faces[face_i][face_vertex_i];
    let in_vertex_pos = Vec3::from(in_vertex.pos);
    let in_uvs = UVec2::new(in_vertex.uvs[0] as u32, in_vertex.uvs[1] as u32);
    let in_normal = Vec3::from(in_vertex.normal);
    let in_vertex_packed_fields = in_vertex.packed_fields;
    // Instance info
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
    // Light RGB
    // Includes block lighting, as well as some basic per-face directional shading.
    {
        // Block lighting, based on surrounding light levels.
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
        // Per-face directional shading.
        let light_source_dir = Vec3::new(2.0, 5.0, 1.0).normalize();
        let dir_lighting = Vec3::dot(in_normal, light_source_dir);
        let dir_light_coef = f32::mul_add(dir_lighting, 0.3, 0.7);
        let light_rgb = calculate_light_rgb(closest_light_pair & 0xF, closest_light_pair >> 4);
        // Tint
        #[cfg(target_arch = "spirv")]
        let tint_percentage = in_vertex_packed_fields.tinted_bit() as f32;
        // TODO: Unify the packed field methods so we don't have to do dirty hacks like this.
        #[cfg(not(target_arch = "spirv"))]
        let tint_percentage = {
            _ = in_vertex_packed_fields;
            1.0
        };
        let applied_tint_rgb = Vec3::lerp(Vec3::splat(1.0), in_tint_colour.xyz(), tint_percentage);
        *out_light_rgb = light_rgb * applied_tint_rgb * dir_light_coef;
    }
}

#[spirv(fragment)]
pub fn fragment(
    // Bindings
    #[spirv(descriptor_set = 1, binding = 0)] block_item_atlas: &Image2d,
    #[spirv(descriptor_set = 1, binding = 1)] block_item_atlas_sampler: &Sampler,
    // Inputs
    in_uvs: Vec2,
    in_light_rgb: Vec3,
    // Outputs
    out_colour: &mut Vec4,
) {
    let tex_sample = block_item_atlas.sample(*block_item_atlas_sampler, in_uvs);
    if tex_sample.w < 1.0 {
        spirv_std::arch::kill();
    }
    *out_colour = Vec4::from((tex_sample.xyz() * in_light_rgb, 1.0));
}
