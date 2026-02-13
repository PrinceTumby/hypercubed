use super::types::BlockFaceInstanceFields;
use spirv_std::glam::*;
use spirv_std::image::{Image2d, SampledImage};
#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::Float;
use spirv_std::spirv;

pub const BASE_NORMAL: Vec3 = Vec3::new(0.0, 1.0, 0.0);

pub fn get_base_position(vertex_i: u32) -> Vec3 {
    match vertex_i % 4 {
        0 => Vec3::new(-0.5, 0.5, 0.5),
        1 => Vec3::new(0.5, 0.5, 0.5),
        2 => Vec3::new(-0.5, 0.5, -0.5),
        _ => Vec3::new(0.5, 0.5, -0.5),
    }
}

pub fn get_uvs(vertex_i: u32, face_rotation_i: u32) -> Vec2 {
    let uv_rotation_vec: [u32; 4] = match face_rotation_i {
        0 => [0, 1, 2, 3],
        1 => [1, 3, 0, 2],
        2 => [3, 2, 1, 0],
        _ => [2, 0, 3, 1],
    };
    let new_vertex_i = uv_rotation_vec[vertex_i as usize % 4];
    match new_vertex_i {
        0 => vec2(0.0, 1.0),
        1 => vec2(1.0, 1.0),
        2 => vec2(0.0, 0.0),
        _ => vec2(1.0, 0.0),
    }
}

#[spirv(vertex)]
pub fn vertex(
    // Bindings
    #[spirv(uniform, descriptor_set = 0, binding = 0)] render_info: &crate::RenderInfo,
    #[spirv(descriptor_set = 0, binding = 1)] lightmap: &crate::LightmapImage,
    // Vertex inputs
    #[spirv(vertex_index)] in_vertex_index: u32,
    in_subchunk_start_coords: Vec3,
    in_face_matrix_index: u32,
    // Instance inputs
    in_uvs: UVec4,
    in_packed_fields: BlockFaceInstanceFields,
    // Outputs
    #[spirv(invariant, position)] out_pos: &mut Vec4,
    out_uvs: &mut Vec2,
    #[spirv(flat)] out_light_rgb: &mut Vec3,
) {
    // Unpack instance data.
    let xyz_offset = Vec3::new(
        in_packed_fields.x_offset() as f32,
        in_packed_fields.y_offset() as f32,
        in_packed_fields.z_offset() as f32,
    );
    let sky_light_level = in_packed_fields.sky_light_level();
    let block_light_level = in_packed_fields.block_light_level();
    // Position
    let base_pos = get_base_position(in_vertex_index);
    let face_matrix = render_info.face_matrices[in_face_matrix_index as usize];
    let local_pos = face_matrix * base_pos;
    let block_centre_pos = in_subchunk_start_coords + xyz_offset;
    let global_pos = local_pos + block_centre_pos + Vec3::splat(0.5);
    *out_pos =
        Vec4::new(1.0, -1.0, 1.0, 1.0) * (render_info.view_matrix * Vec4::from((global_pos, 1.0)));
    // UVs
    let start_uvs = in_uvs.xy().as_vec2() * render_info.recip_block_item_atlas_size;
    let end_uvs = in_uvs.zw().as_vec2() * render_info.recip_block_item_atlas_size;
    let face_rotation_i = in_packed_fields.uv_rotation();
    let base_uvs = get_uvs(in_vertex_index, face_rotation_i);
    *out_uvs = Vec2::new(
        start_uvs.x.lerp(end_uvs.x, base_uvs.x),
        start_uvs.y.lerp(end_uvs.y, base_uvs.y),
    );
    // Light RGB
    // Includes block lighting, as well as some basic per-face directional shading.
    let normal = face_matrix * BASE_NORMAL;
    let light_source_dir = Vec3::new(2.0, 5.0, 1.0).normalize();
    let dir_lighting = Vec3::dot(normal, light_source_dir);
    let dir_light_coef = f32::mul_add(dir_lighting, 0.3, 0.7);
    let lightmap_rgb = lightmap
        .fetch((sky_light_level as u32 * 16) + block_light_level as u32)
        .xyz();
    *out_light_rgb = lightmap_rgb * dir_light_coef;
}

#[spirv(fragment)]
pub fn fragment(
    // Bindings
    #[spirv(descriptor_set = 0, binding = 2)] block_item_atlas: &SampledImage<Image2d>,
    // Inputs
    in_uvs: Vec2,
    #[spirv(flat)] in_light_rgb: Vec3,
    // Outputs
    out_colour: &mut Vec4,
) {
    let tex_sample = block_item_atlas.sample(in_uvs);
    *out_colour = Vec4::from((tex_sample.xyz() * in_light_rgb, 1.0));
}
