use super::types::PackedFlags;
use spirv_std::glam::{Vec2, Vec2Swizzles, Vec3, Vec4, Vec4Swizzles};
use spirv_std::spirv;

#[spirv(vertex)]
pub fn vertex(
    // Bindings
    #[spirv(uniform, descriptor_set = 0, binding = 0)] render_info: &crate::RenderInfo,
    // Inputs
    #[spirv(vertex_index)] in_vertex_i: u32,
    in_p1: Vec3,
    in_p2: Vec3,
    in_size: f32,
    in_colour: Vec4,
    in_flags: PackedFlags,
    // Outputs
    #[spirv(invariant, position)] out_pos: &mut Vec4,
    #[spirv(flat)] out_colour: &mut Vec4,
    #[spirv(flat)] out_flags: &mut PackedFlags,
) {
    let p1_clip_pos = render_info.view_matrix * Vec4::from((in_p1, 1.0));
    let p2_clip_pos = render_info.view_matrix * Vec4::from((in_p2, 1.0));
    let clip_diff_norm = (p2_clip_pos.xy() - p1_clip_pos.xy()).normalize_or_zero();
    // Calculate the perpendicular vector offset.
    let offset_right =
        clip_diff_norm.yx() * Vec2::new(-1.0, 1.0) * in_size * render_info.recip_screen_size;
    let offset = if in_vertex_i.is_multiple_of(2) {
        offset_right
    } else {
        -offset_right
    };
    let mut vertex_pos = if in_vertex_i / 2 != 0 {
        p1_clip_pos
    } else {
        p2_clip_pos
    };
    vertex_pos += Vec4::from((offset * vertex_pos.w, 0.0, 0.0));
    *out_pos = Vec4::new(1.0, -1.0, 1.0, 1.0) * vertex_pos;
    *out_colour = in_colour;
    *out_flags = in_flags;
}

#[spirv(fragment(depth_replacing))]
pub fn fragment(
    // Inputs
    #[spirv(frag_coord)] in_pos: Vec4,
    #[spirv(flat)] in_colour: Vec4,
    #[spirv(flat)] in_flags: PackedFlags,
    // Outputs
    #[spirv(frag_depth)] out_depth: &mut f32,
    out_colour: &mut Vec4,
) {
    *out_depth = if in_flags.ignore_depth() {
        1.0
    } else {
        in_pos.z
    };
    *out_colour = Vec4::from((in_colour.xyz() * in_colour.w, in_colour.w));
}
