use spirv_std::glam::*;
use spirv_std::image::{Image2d, SampledImage};
use spirv_std::spirv;

const SUN_BASE_POSITIONS: [Vec3; 4] = [
    Vec3::new(0.90453404, 0.30151135, -0.30151135),
    Vec3::new(0.90453404, -0.30151135, -0.30151135),
    Vec3::new(0.90453404, 0.30151135, 0.30151135),
    Vec3::new(0.90453404, -0.30151135, 0.30151135),
];

const SUN_UVS: [Vec2; 4] = [
    Vec2::new(0.0, 0.0),
    Vec2::new(1.0, 0.0),
    Vec2::new(0.0, 1.0),
    Vec2::new(1.0, 1.0),
];

#[spirv(vertex)]
pub fn vertex(
    // Bindings
    #[spirv(uniform, descriptor_set = 0, binding = 0)] render_info: &crate::RenderInfo,
    // Vertex inputs
    #[spirv(vertex_index)] in_vertex_i: u32,
    // Outputs
    #[spirv(invariant, position)] out_pos: &mut Vec4,
    out_uvs: &mut Vec2,
) {
    let base_pos = SUN_BASE_POSITIONS[in_vertex_i as usize % 4];
    *out_pos =
        Vec4::new(1.0, -1.0, 1.0, 1.0) * (render_info.sky_matrix * Vec4::from((base_pos, 1.0)));
    *out_uvs = SUN_UVS[in_vertex_i as usize % 4];
}

#[spirv(fragment)]
pub fn fragment(
    // Bindings
    #[spirv(descriptor_set = 0, binding = 4)] sun_image: &SampledImage<Image2d>,
    // Inputs
    in_uvs: Vec2,
    // Outputs
    out_colour: &mut Vec4,
) {
    *out_colour = sun_image.sample(in_uvs);
}
