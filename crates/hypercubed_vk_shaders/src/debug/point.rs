use super::types::PackedFlags;
use spirv_std::glam::{Vec3, Vec4};
use spirv_std::spirv;

#[spirv(vertex)]
pub fn vertex(
    // Bindings
    #[spirv(uniform, descriptor_set = 0, binding = 0)] render_info: &crate::RenderInfo,
    // Inputs
    in_pos: Vec3,
    in_colour: Vec4,
    in_size: f32,
    in_packed_fields: PackedFlags,
    // Outputs
    #[spirv(invariant, position)] out_pos: &mut Vec4,
    #[spirv(point_size)] out_size: &mut f32,
    out_colour: &mut Vec4,
) {
    let mut pos =
        Vec4::new(1.0, -1.0, 1.0, 1.0) * (render_info.view_matrix * Vec4::from((in_pos, 1.0)));
    if in_packed_fields.ignore_depth() {
        pos.z = pos.z.signum();
    }
    *out_pos = pos;
    *out_size = in_size;
    *out_colour = in_colour;
}

#[spirv(fragment)]
pub fn fragment(in_colour: Vec4, out_colour: &mut Vec4) {
    *out_colour = in_colour * Vec4::ONE;
}
