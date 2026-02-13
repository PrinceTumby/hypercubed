use spirv_std::glam::*;
use spirv_std::spirv;

#[spirv(vertex)]
pub fn vertex(
    // Bindings
    #[spirv(uniform, descriptor_set = 0, binding = 0)] render_info: &crate::RenderInfo,
    // Vertex inputs
    #[spirv(vertex_index)] in_vertex_i: u32,
    // Instance inputs
    in_p1: Vec3,
    in_p2: Vec3,
    in_p3: Vec3,
    in_p4: Vec3,
    // Outputs
    #[spirv(invariant, position)] out_pos: &mut Vec4,
    #[spirv(flat)] out_star_brightness: &mut f32,
) {
    let global_pos = [in_p1, in_p2, in_p3, in_p4][in_vertex_i as usize % 4];
    *out_pos =
        Vec4::new(1.0, -1.0, 1.0, 1.0) * (render_info.sky_matrix * Vec4::from((global_pos, 1.0)));
    *out_star_brightness = render_info.star_brightness;
}

#[spirv(fragment)]
pub fn fragment(
    // Inputs
    in_star_brightness: f32,
    // Outputs
    out_colour: &mut Vec4,
) {
    *out_colour = Vec3::splat(in_star_brightness).extend(0.0);
}
