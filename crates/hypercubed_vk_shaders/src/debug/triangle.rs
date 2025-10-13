use spirv_std::glam::{Mat4, Vec3, Vec4};
use spirv_std::spirv;
use super::types::PackedFlags;

#[spirv(vertex)]
pub fn vertex(
    // Bindings
    #[spirv(uniform, descriptor_set = 0, binding = 0)] view_matrix: &Mat4,
    // Inputs
    #[spirv(vertex_index)] in_vertex_index: u32,
    in_p1: Vec3,
    in_p2: Vec3,
    in_p3: Vec3,
    in_colour: Vec4,
    in_packed_fields: PackedFlags,
    // Outputs
    #[spirv(invariant, position)] out_pos: &mut Vec4,
    out_colour: &mut Vec4,
) {
    let vertex_pos = match in_vertex_index {
        0 => in_p1,
        1 => in_p2,
        2 | _ => in_p3,
    };
    let mut pos = Vec4::new(1.0, -1.0, 1.0, 1.0) * (*view_matrix * Vec4::from((vertex_pos, 1.0)));
    if in_packed_fields.ignore_depth() {
        pos.z = pos.z.signum();
    }
    *out_pos = pos;
    *out_colour = in_colour;
}

#[spirv(fragment)]
pub fn fragment(
    in_colour: Vec4,
    out_colour: &mut Vec4,
) {
    *out_colour = in_colour * Vec4::ONE;
}
