use spirv_std::glam::*;
use spirv_std::image::{Image2d, SampledImage};
use spirv_std::spirv;

const MOON_BASE_POSITIONS: [Vec3; 4] = [
    Vec3::new(-0.90453404, 0.30151135, -0.30151135),
    Vec3::new(-0.90453404, -0.30151135, -0.30151135),
    Vec3::new(-0.90453404, 0.30151135, 0.30151135),
    Vec3::new(-0.90453404, -0.30151135, 0.30151135),
];

fn calculate_moon_uvs(time_of_day: i32, vertex_i: u32) -> Vec2 {
    let moon_phase_i = ((time_of_day - 6000) / 24000) % 8;
    let uv_start_x = (moon_phase_i % 4) as f32 / 4.0;
    let uv_start_y = (moon_phase_i / 4) as f32 / 2.0;
    let uv_width = 1.0 / 4.0;
    let uv_height = 1.0 / 2.0;
    match vertex_i {
        0 => Vec2::new(uv_start_x, uv_start_y),
        1 => Vec2::new(uv_start_x + uv_width, uv_start_y),
        2 => Vec2::new(uv_start_x, uv_start_y + uv_height),
        _ => Vec2::new(uv_start_x + uv_width, uv_start_y + uv_height),
    }
}

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
    let base_pos = MOON_BASE_POSITIONS[in_vertex_i as usize % 4];
    *out_pos =
        Vec4::new(1.0, -1.0, 1.0, 1.0) * (render_info.sky_matrix * Vec4::from((base_pos, 1.0)));
    *out_uvs = calculate_moon_uvs(render_info.time_of_day as i32, in_vertex_i % 4);
}

#[spirv(fragment)]
pub fn fragment(
    // Bindings
    #[spirv(descriptor_set = 0, binding = 5)] moon_phases_image: &SampledImage<Image2d>,
    // Inputs
    in_uvs: Vec2,
    // Outputs
    out_colour: &mut Vec4,
) {
    *out_colour = moon_phases_image.sample(in_uvs);
}
