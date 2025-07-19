#![cfg_attr(feature = "graphics_backend_vulkan", allow(unused))]
#[cfg(feature = "graphics_backend_vulkan")]
use super::chunk_rc_consts::*;
#[cfg(feature = "graphics_backend_vulkan")]
use super::chunk_rc_types::BlockFaceInstance;
#[cfg(not(feature = "graphics_backend_vulkan"))]
use super::consts::*;
#[cfg(not(feature = "graphics_backend_vulkan"))]
use super::types::BlockFaceInstance;
#[cfg(target_arch = "spirv")]
use crate::RawAtlasImage;
#[cfg(not(feature = "graphics_backend_vulkan"))]
use core::marker::PhantomData;
use spirv_std::glam::*;
use spirv_std::image::Image2d;
#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::Float;
use spirv_std::ray_tracing::{AccelerationStructure, CommittedIntersection, RayFlags, RayQuery};
use spirv_std::{ray_query as spirv_ray_query, spirv};

#[cfg(not(target_arch = "spirv"))]
pub type RawAtlasImage = spirv_std::image::Image!(
    2D,
    format = rgba8,
    // type=f32,
    depth = false,
    sampled = false,
);

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TlasInstanceInfo {
    pub quads_info_offsets: [u32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RayTracedQuadInfo {
    pub uvs: [[u16; 2]; 4],
    pub packed_fields: RayTracedQuadPackedFields,
}

bitfield::bitfield! {
    // 0-7: Unused
    // 8-31: Tint colour
    #[repr(transparent)]
    #[derive(Clone, Copy)]
    pub struct RayTracedQuadPackedFields(u32);
    impl Debug;
    pub tint_colour, set_tint_colour: 31, 8;
}

// macro_rules! debug {
//     ($msg:literal $(, $($val:expr),*)?) => {
//         unsafe {
//             debug_printfln!($msg, $($val),*);
//         }
//     };
// }

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DebugInputInfo {
    pub inv_view_matrix: Mat4,
}

#[cfg(not(feature = "graphics_backend_vulkan"))]
#[spirv(compute(threads(16, 4)))]
pub fn raytrace_debug(
    #[spirv(descriptor_set = 0, binding = 0)] block_item_atlas: &Image2d,
    // #[spirv(descriptor_set = 0, binding = 1)] block_item_luma_atlas: &Image2d,
    #[spirv(descriptor_set = 0, binding = 2)] world_tlas: &AccelerationStructure,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] instances: &[TlasInstanceInfo],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] quads_info: &[RayTracedQuadInfo],
    #[spirv(descriptor_set = 1, binding = 0)] output_image: &RawAtlasImage,
    // Inputs
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(push_constant)] input_info: &DebugInputInfo,
) {
    let output_image_dims = uvec2(960, 540);
    let camera_global_pos_v4 = input_info.inv_view_matrix * Vec4::W;
    let camera_global_pos = camera_global_pos_v4.xyz() / camera_global_pos_v4.w;
    // Ray info
    let ray_start = camera_global_pos;
    let ray_end_view_space_v4 = vec4(
        (id.x as f32 / output_image_dims.x as f32) * 2.0 - 1.0,
        (id.y as f32 / output_image_dims.y as f32) * 2.0 - 1.0,
        0.9999,
        1.0,
    );
    let ray_end_v4 = input_info.inv_view_matrix * ray_end_view_space_v4;
    let ray_end = ray_end_v4.xyz() / ray_end_v4.w;
    let ray_diff = ray_end - ray_start;
    let ray_dir = ray_diff.normalize();
    let ray_len = ray_diff.length();

    // Ray trace
    spirv_ray_query!(let mut ray_query);
    let ray_hit_colour = unsafe {
        ray_query.initialize(
            world_tlas,
            RayFlags::CULL_BACK_FACING_TRIANGLES,
            0xFF,
            ray_start,
            0.0,
            ray_dir,
            ray_len,
        );
        while ray_query.proceed() {
            let instance_idx = ray_query.get_candidate_intersection_instance_id();
            let geometry_idx = ray_query.get_candidate_intersection_geometry_index();
            let primitive_idx = ray_query.get_candidate_intersection_primitive_index();
            let instance_info = instances[instance_idx as usize];
            let offset = instance_info.quads_info_offsets[geometry_idx as usize];
            let quad_info = quads_info[offset as usize + (primitive_idx / 2) as usize];
            let quad_uvs = quad_info.uvs;
            let uvs = if primitive_idx % 2 == 0 {
                [
                    U16Vec2::from(quad_uvs[1]),
                    U16Vec2::from(quad_uvs[0]),
                    U16Vec2::from(quad_uvs[2]),
                ]
            } else {
                [
                    U16Vec2::from(quad_uvs[3]),
                    U16Vec2::from(quad_uvs[1]),
                    U16Vec2::from(quad_uvs[2]),
                ]
            };
            let bary_uv: Vec2 = ray_query.get_candidate_intersection_barycentrics();
            let [u, v] = bary_uv.into();
            let w = 1.0 - u - v;
            let uvs: Vec2 = uvs[0].as_vec2() * w + uvs[1].as_vec2() * u + uvs[2].as_vec2() * v;
            let base_texture_colour = block_item_atlas.fetch(uvs.as_uvec2());
            if base_texture_colour.w == 1.0 {
                ray_query.confirm_intersection();
            }
        }
        let did_ray_hit =
            ray_query.get_committed_intersection_type() == CommittedIntersection::Triangle;
        if did_ray_hit {
            // TODO:
            // - Switch atlas to a storage texture
            // - Implement radiance cascade update pipelines
            // - Consider switching to ray tracing pipeline (needs implementing in vulkano)
            let instance_idx = ray_query.get_committed_intersection_instance_id();
            let geometry_idx = ray_query.get_committed_intersection_geometry_index();
            let primitive_idx = ray_query.get_committed_intersection_primitive_index();
            let instance_info = instances[instance_idx as usize];
            let offset = instance_info.quads_info_offsets[geometry_idx as usize];
            let quad_info = quads_info[offset as usize + (primitive_idx / 2) as usize];
            let quad_uvs = quad_info.uvs;
            let uvs = if primitive_idx % 2 == 0 {
                [
                    U16Vec2::from(quad_uvs[1]),
                    U16Vec2::from(quad_uvs[0]),
                    U16Vec2::from(quad_uvs[2]),
                ]
            } else {
                [
                    U16Vec2::from(quad_uvs[3]),
                    U16Vec2::from(quad_uvs[1]),
                    U16Vec2::from(quad_uvs[2]),
                ]
            };
            let bary_uv: Vec2 = ray_query.get_committed_intersection_barycentrics();
            let [u, v] = bary_uv.into();
            let w = 1.0 - u - v;
            let uvs: Vec2 = uvs[0].as_vec2() * w + uvs[1].as_vec2() * u + uvs[2].as_vec2() * v;
            let base_texture_colour = block_item_atlas.fetch(uvs.as_uvec2());
            let tint_rgb = quad_info.packed_fields.tint_colour();
            let tint = vec4(
                (tint_rgb & 0xFF) as f32 / 255.0,
                ((tint_rgb >> 8) & 0xFF) as f32 / 255.0,
                ((tint_rgb >> 16) & 0xFF) as f32 / 255.0,
                1.0,
            );
            base_texture_colour * tint
            // let luma_colour = block_item_luma_atlas.fetch(uvs.as_uvec2());
            // Vec4::from((luma_colour.xxx(), 1.0))
        } else {
            Vec4::ZERO
        }
    };

    let output_colour = ray_hit_colour;
    let store_y = output_image_dims.y - 1 - id.y;
    unsafe {
        output_image.write(uvec2(id.x, store_y), output_colour);
    }
}

// Cascade update shaders

#[cfg(not(feature = "graphics_backend_vulkan"))]
#[inline]
fn trace_ray(
    ray_query: &mut RayQuery,
    block_item_atlas: &Image2d,
    block_item_luma_atlas: &Image2d,
    world_tlas: &AccelerationStructure,
    instances: &[TlasInstanceInfo],
    quads_info: &[RayTracedQuadInfo],
    ray_start: Vec3,
    ray_dir: Vec3,
    ray_tmin: f32,
    ray_tmax: f32,
) -> Vec4 {
    unsafe {
        ray_query.initialize(
            world_tlas,
            // RayFlags::CULL_BACK_FACING_TRIANGLES,
            RayFlags::empty(),
            0xFF,
            ray_start,
            ray_tmin,
            ray_dir,
            ray_tmax,
        );
        while ray_query.proceed() {
            let instance_idx = ray_query.get_candidate_intersection_instance_id();
            let geometry_idx = ray_query.get_candidate_intersection_geometry_index();
            let primitive_idx = ray_query.get_candidate_intersection_primitive_index();
            let instance_info = instances[instance_idx as usize];
            let offset = instance_info.quads_info_offsets[geometry_idx as usize];
            let quad_info = quads_info[offset as usize + (primitive_idx / 2) as usize];
            let quad_uvs = quad_info.uvs;
            let uvs = if primitive_idx % 2 == 0 {
                [
                    U16Vec2::from(quad_uvs[1]),
                    U16Vec2::from(quad_uvs[0]),
                    U16Vec2::from(quad_uvs[2]),
                ]
            } else {
                [
                    U16Vec2::from(quad_uvs[3]),
                    U16Vec2::from(quad_uvs[1]),
                    U16Vec2::from(quad_uvs[2]),
                ]
            };
            let bary_uv: Vec2 = ray_query.get_candidate_intersection_barycentrics();
            let [u, v] = bary_uv.into();
            let w = 1.0 - u - v;
            let uvs: Vec2 = uvs[0].as_vec2() * w + uvs[1].as_vec2() * u + uvs[2].as_vec2() * v;
            let base_texture_colour = block_item_atlas.fetch(uvs.as_uvec2());
            if base_texture_colour.w == 1.0 {
                ray_query.confirm_intersection();
            }
        }
        let did_ray_hit =
            ray_query.get_committed_intersection_type() == CommittedIntersection::Triangle;
        if did_ray_hit {
            let instance_idx = ray_query.get_committed_intersection_instance_id();
            let geometry_idx = ray_query.get_committed_intersection_geometry_index();
            let primitive_idx = ray_query.get_committed_intersection_primitive_index();
            let instance_info = instances[instance_idx as usize];
            let offset = instance_info.quads_info_offsets[geometry_idx as usize];
            let quad_info = quads_info[offset as usize + (primitive_idx / 2) as usize];
            let quad_uvs = quad_info.uvs;
            let uvs = if primitive_idx % 2 == 0 {
                [
                    U16Vec2::from(quad_uvs[1]),
                    U16Vec2::from(quad_uvs[0]),
                    U16Vec2::from(quad_uvs[2]),
                ]
            } else {
                [
                    U16Vec2::from(quad_uvs[3]),
                    U16Vec2::from(quad_uvs[1]),
                    U16Vec2::from(quad_uvs[2]),
                ]
            };
            let bary_uv: Vec2 = ray_query.get_committed_intersection_barycentrics();
            let [u, v] = bary_uv.into();
            let w = 1.0 - u - v;
            let uvs: Vec2 = uvs[0].as_vec2() * w + uvs[1].as_vec2() * u + uvs[2].as_vec2() * v;
            let uv_coords = uvs.as_uvec2();
            let base_texture_colour = block_item_atlas.fetch(uv_coords);
            let tint_rgb = quad_info.packed_fields.tint_colour();
            let tint = vec4(
                (tint_rgb & 0xFF) as f32 / 255.0,
                ((tint_rgb >> 8) & 0xFF) as f32 / 255.0,
                ((tint_rgb >> 16) & 0xFF) as f32 / 255.0,
                1.0,
            );
            let luma_multiplier = block_item_luma_atlas.fetch(uv_coords).xxx() * 32.0;
            base_texture_colour * tint * Vec4::from((luma_multiplier, 1.0))
        } else {
            Vec4::ZERO
        }
    }
}

pub fn get_cascade_0_probe_local_position(probe_i: usize) -> Vec3 {
    let probe_x = probe_i % 16;
    let probe_z = probe_i / 16;
    let probe_x_pos = f32::mul_add(probe_x as f32, 1.0 / 16.0, 1.0 / 32.0);
    let probe_z_pos = f32::mul_add(probe_z as f32, 1.0 / 16.0, 1.0 / 32.0);
    vec3(probe_x_pos - 0.5, 0.5, 0.5 - probe_z_pos)
}

pub fn get_cascade_1_probe_local_position(probe_i: usize) -> Vec3 {
    let probe_x = probe_i % 8;
    let probe_z = probe_i / 8;
    let probe_x_pos = f32::mul_add(probe_x as f32, (1.0 - (1.0 / 16.0)) / 7.0, 1.0 / 32.0);
    let probe_z_pos = f32::mul_add(probe_z as f32, (1.0 - (1.0 / 16.0)) / 7.0, 1.0 / 32.0);
    vec3(probe_x_pos - 0.5, 0.5, 0.5 - probe_z_pos)
}

pub fn get_cascade_2_probe_local_position(probe_i: usize) -> Vec3 {
    let probe_x = probe_i % 4;
    let probe_z = probe_i / 4;
    let probe_x_pos = f32::mul_add(probe_x as f32, (1.0 - (1.0 / 16.0)) / 3.0, 1.0 / 32.0);
    let probe_z_pos = f32::mul_add(probe_z as f32, (1.0 - (1.0 / 16.0)) / 3.0, 1.0 / 32.0);
    vec3(probe_x_pos - 0.5, 0.5, 0.5 - probe_z_pos)
}

pub fn get_cascade_3_probe_local_position(probe_i: usize) -> Vec3 {
    let probe_x = probe_i % 2;
    let probe_z = probe_i / 2;
    let probe_x_pos = f32::mul_add(probe_x as f32, 1.0 - (1.0 / 16.0), 1.0 / 32.0);
    let probe_z_pos = f32::mul_add(probe_z as f32, 1.0 - (1.0 / 16.0), 1.0 / 32.0);
    vec3(probe_x_pos - 0.5, 0.5, 0.5 - probe_z_pos)
}

pub fn get_cascade_probe_local_position(probe_i: usize, probe_dim_len: usize) -> Vec3 {
    let probe_div = (probe_dim_len - 1) as f32;
    let probe_x = probe_i % 4;
    let probe_z = probe_i / 4;
    let probe_x_pos = f32::mul_add(probe_x as f32, (1.0 - (1.0 / 16.0)) / probe_div, 1.0 / 32.0);
    let probe_z_pos = f32::mul_add(probe_z as f32, (1.0 - (1.0 / 16.0)) / probe_div, 1.0 / 32.0);
    vec3(probe_x_pos - 0.5, 0.5, 0.5 - probe_z_pos)
}

pub fn get_ray_direction(num_rays: usize, ray_i: usize) -> Vec3 {
    let phi = core::f32::consts::PI * (f32::sqrt(5.0) - 1.0);
    let y = 1.0 - (ray_i as f32 / (num_rays * 2 - 1) as f32) * 2.0;
    let radius = f32::sqrt(1.0 - (y * y));
    let theta = phi * ray_i as f32;
    let x = theta.cos() * radius;
    let z = theta.sin() * radius;
    vec3(x, y, z).normalize()
}

pub fn rgb_to_hsv448(rgb: Vec3) -> u16 {
    let k = vec4(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);
    let p = Vec4::lerp(
        Vec4::from((rgb.zy(), k.wz())),
        Vec4::from((rgb.zy(), k.xy())),
        if rgb.z <= rgb.y { 1.0 } else { 0.0 },
    );
    let q = Vec4::lerp(
        Vec4::from((p.xyw(), rgb.x)),
        Vec4::from((rgb.x, p.yzx())),
        if p.x <= rgb.x { 1.0 } else { 0.0 },
    );
    let d = q.x - f32::min(q.w, q.y);
    let e = 1.0e-10;
    let h = f32::abs(q.z + (q.w - q.y) / (6.0 * d + e));
    let s = d / (q.x + e);
    let v = q.x;
    (h.clamp(0.0, 1.0) * 15.0).round() as u16
        | (((s.clamp(0.0, 1.0) * 15.0).round() as u16) << 4)
        | (((v.clamp(0.0, 1.0) * 255.0).round() as u16) << 8)
}

pub fn hsv448_to_rgb(packed_hsv: u16) -> Vec3 {
    let hsv = vec3(
        (packed_hsv & 0xF) as f32 / 15.0,
        ((packed_hsv >> 4) & 0xF) as f32 / 15.0,
        ((packed_hsv >> 8) & 0xFF) as f32 / 255.0,
    );
    let k = vec4(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    let p = Vec3::abs(Vec3::fract_gl(hsv.xxx() + k.xyz()) * 6.0 - k.www());
    Vec3::lerp(
        k.xxx(),
        (p - k.xxx()).clamp(Vec3::splat(0.0), Vec3::splat(1.0)),
        hsv.y,
    ) * hsv.z
}

pub fn rgba_vec4_to_8888(rgba: Vec4) -> u32 {
    (rgba.x.clamp(0.0, 1.0) * 255.0) as u32
        | (((rgba.y.clamp(0.0, 1.0) * 255.0) as u32) << 8)
        | (((rgba.z.clamp(0.0, 1.0) * 255.0) as u32) << 16)
        | (((rgba.w.clamp(0.0, 1.0) * 255.0) as u32) << 24)
}

pub fn rgba_8888_to_vec4(rgba: u32) -> Vec4 {
    vec4(
        (rgba & 0xFF) as f32 / 255.0,
        ((rgba >> 8) & 0xFF) as f32 / 255.0,
        ((rgba >> 16) & 0xFF) as f32 / 255.0,
        ((rgba >> 24) & 0xFF) as f32 / 255.0,
    )
}

pub fn rgba_vec4_to_10_10_10_2(rgba: Vec4) -> u32 {
    (rgba.x.clamp(0.0, 4.0) * 255.0) as u32
        | (((rgba.y.clamp(0.0, 4.0) * 255.0) as u32) << 10)
        | (((rgba.z.clamp(0.0, 4.0) * 255.0) as u32) << 20)
        | (((rgba.w.clamp(0.0, 1.0) * 3.0) as u32) << 30)
}

pub fn rgba_10_10_10_2_to_vec4(rgba: u32) -> Vec4 {
    vec4(
        (rgba & 0x3FF) as f32 / 255.0,
        ((rgba >> 10) & 0x3FF) as f32 / 255.0,
        ((rgba >> 20) & 0x3FF) as f32 / 255.0,
        ((rgba >> 30) & 0x3) as f32 / 3.0,
    )
}

#[derive(Clone, Copy, Debug)]
pub struct Cascade1NearestProbeInfo {
    pub probes: [Cascade1NearestProbe; 4],
    pub interp_divisor: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Cascade1NearestProbe {
    pub index: usize,
    pub interp_factor: f32,
}

cfg_if::cfg_if! {
    if #[cfg(feature = "graphics_backend_vulkan")] {
        macro_rules! cpu_dbg {
            ($($exprs:expr),* $(,)?) => {
                dbg!($($exprs),*)
            };
        }
    } else {
        macro_rules! cpu_dbg {
            ($expr:expr $(,)?) => {
                $expr
            };
            ($($exprs:expr),* $(,)?) => {
                ($($exprs,)*)
            };
        }
    }
}

pub fn get_nearest_cascade_1_probes(cascade_0_probe_pos: Vec2) -> Cascade1NearestProbeInfo {
    let pos_01 = vec2(cascade_0_probe_pos.x + 0.5, 0.5 - cascade_0_probe_pos.y);
    let pos_top_left = pos_01 - (1.0 / 32.0);
    let probe_xz_f32s = (pos_top_left * 7.0) / (1.0 - (1.0 / 16.0));
    let floor_xz = probe_xz_f32s.floor();
    let ceil_xz = probe_xz_f32s.ceil();
    let upper_left_probe_01 = (floor_xz * (1.0 - (1.0 / 16.0))) / 7.0;
    let lower_right_probe_01 = (ceil_xz * (1.0 - (1.0 / 16.0))) / 7.0;
    let upper_left_probe_diff = (pos_top_left - upper_left_probe_01).abs();
    let lower_right_probe_diff = (pos_top_left - lower_right_probe_01).abs();
    let upper_left_probe_interp_factor = lower_right_probe_diff.x * lower_right_probe_diff.y;
    let upper_right_probe_interp_factor = upper_left_probe_diff.x * lower_right_probe_diff.y;
    let lower_left_probe_interp_factor = lower_right_probe_diff.x * upper_left_probe_diff.y;
    let lower_right_probe_interp_factor = upper_left_probe_diff.x * upper_left_probe_diff.y;
    let upper_left_probe_xz = UVec2::min(floor_xz.as_uvec2(), UVec2::splat(7));
    let upper_right_probe_xz = UVec2::min(vec2(ceil_xz.x, floor_xz.y).as_uvec2(), UVec2::splat(7));
    let lower_left_probe_xz = UVec2::min(vec2(floor_xz.x, ceil_xz.y).as_uvec2(), UVec2::splat(7));
    let lower_right_probe_xz = UVec2::min(ceil_xz.as_uvec2(), UVec2::splat(7));
    let upper_left_probe_i = upper_left_probe_xz.y * 8 + u32::min(upper_left_probe_xz.x, 7);
    let upper_right_probe_i = upper_right_probe_xz.y * 8 + u32::min(upper_right_probe_xz.x, 7);
    let lower_left_probe_i = lower_left_probe_xz.y * 8 + u32::min(lower_left_probe_xz.x, 7);
    let lower_right_probe_i = lower_right_probe_xz.y * 8 + u32::min(lower_right_probe_xz.x, 7);
    let probes = [
        Cascade1NearestProbe {
            index: upper_left_probe_i as usize,
            interp_factor: upper_left_probe_interp_factor,
        },
        Cascade1NearestProbe {
            index: upper_right_probe_i as usize,
            interp_factor: upper_right_probe_interp_factor,
        },
        Cascade1NearestProbe {
            index: lower_left_probe_i as usize,
            interp_factor: lower_left_probe_interp_factor,
        },
        Cascade1NearestProbe {
            index: lower_right_probe_i as usize,
            interp_factor: lower_right_probe_interp_factor,
        },
    ];
    let probes_diff = (lower_right_probe_01 - upper_left_probe_01).abs();
    Cascade1NearestProbeInfo {
        probes,
        interp_divisor: probes_diff.x * probes_diff.y,
    }
}

pub fn get_nearest_cascade_2_probes(cascade_1_probe_pos: Vec2) -> Cascade1NearestProbeInfo {
    let pos_01 = vec2(cascade_1_probe_pos.x + 0.5, 0.5 - cascade_1_probe_pos.y);
    let pos_top_left = pos_01 - (1.0 / 32.0);
    let probe_xz_f32s = (pos_top_left * 3.0) / (1.0 - (1.0 / 16.0));
    let floor_xz = probe_xz_f32s.floor();
    let ceil_xz = probe_xz_f32s.ceil();
    let upper_left_probe_01 = (floor_xz * (1.0 - (1.0 / 16.0))) / 3.0;
    let lower_right_probe_01 = (ceil_xz * (1.0 - (1.0 / 16.0))) / 3.0;
    let upper_left_probe_diff = (pos_top_left - upper_left_probe_01).abs();
    let lower_right_probe_diff = (pos_top_left - lower_right_probe_01).abs();
    let upper_left_probe_interp_factor = lower_right_probe_diff.x * lower_right_probe_diff.y;
    let upper_right_probe_interp_factor = upper_left_probe_diff.x * lower_right_probe_diff.y;
    let lower_left_probe_interp_factor = lower_right_probe_diff.x * upper_left_probe_diff.y;
    let lower_right_probe_interp_factor = upper_left_probe_diff.x * upper_left_probe_diff.y;
    let upper_left_probe_xz = UVec2::min(floor_xz.as_uvec2(), UVec2::splat(3));
    let upper_right_probe_xz = UVec2::min(vec2(ceil_xz.x, floor_xz.y).as_uvec2(), UVec2::splat(3));
    let lower_left_probe_xz = UVec2::min(vec2(floor_xz.x, ceil_xz.y).as_uvec2(), UVec2::splat(3));
    let lower_right_probe_xz = UVec2::min(ceil_xz.as_uvec2(), UVec2::splat(3));
    let upper_left_probe_i = upper_left_probe_xz.y * 4 + u32::min(upper_left_probe_xz.x, 3);
    let upper_right_probe_i = upper_right_probe_xz.y * 4 + u32::min(upper_right_probe_xz.x, 3);
    let lower_left_probe_i = lower_left_probe_xz.y * 4 + u32::min(lower_left_probe_xz.x, 3);
    let lower_right_probe_i = lower_right_probe_xz.y * 4 + u32::min(lower_right_probe_xz.x, 3);
    let probes = [
        Cascade1NearestProbe {
            index: upper_left_probe_i as usize,
            interp_factor: upper_left_probe_interp_factor,
        },
        Cascade1NearestProbe {
            index: upper_right_probe_i as usize,
            interp_factor: upper_right_probe_interp_factor,
        },
        Cascade1NearestProbe {
            index: lower_left_probe_i as usize,
            interp_factor: lower_left_probe_interp_factor,
        },
        Cascade1NearestProbe {
            index: lower_right_probe_i as usize,
            interp_factor: lower_right_probe_interp_factor,
        },
    ];
    let probes_diff = (lower_right_probe_01 - upper_left_probe_01).abs();
    Cascade1NearestProbeInfo {
        probes,
        interp_divisor: probes_diff.x * probes_diff.y,
    }
}

pub fn get_nearest_cascade_3_probes(cascade_2_probe_pos: Vec2) -> Cascade1NearestProbeInfo {
    let pos_01 = vec2(cascade_2_probe_pos.x + 0.5, 0.5 - cascade_2_probe_pos.y);
    let pos_top_left = pos_01 - (1.0 / 32.0);
    let probe_xz_f32s = pos_top_left / (1.0 - (1.0 / 16.0));
    let floor_xz = probe_xz_f32s.floor();
    let ceil_xz = probe_xz_f32s.ceil();
    let upper_left_probe_01 = floor_xz * (1.0 - (1.0 / 16.0));
    let lower_right_probe_01 = ceil_xz * (1.0 - (1.0 / 16.0));
    let upper_left_probe_diff = (pos_top_left - upper_left_probe_01).abs();
    let lower_right_probe_diff = (pos_top_left - lower_right_probe_01).abs();
    let upper_left_probe_interp_factor = lower_right_probe_diff.x * lower_right_probe_diff.y;
    let upper_right_probe_interp_factor = upper_left_probe_diff.x * lower_right_probe_diff.y;
    let lower_left_probe_interp_factor = lower_right_probe_diff.x * upper_left_probe_diff.y;
    let lower_right_probe_interp_factor = upper_left_probe_diff.x * upper_left_probe_diff.y;
    let upper_left_probe_xz = UVec2::min(floor_xz.as_uvec2(), UVec2::splat(1));
    let upper_right_probe_xz = UVec2::min(vec2(ceil_xz.x, floor_xz.y).as_uvec2(), UVec2::splat(1));
    let lower_left_probe_xz = UVec2::min(vec2(floor_xz.x, ceil_xz.y).as_uvec2(), UVec2::splat(1));
    let lower_right_probe_xz = UVec2::min(ceil_xz.as_uvec2(), UVec2::splat(1));
    let upper_left_probe_i = upper_left_probe_xz.y * 2 + u32::min(upper_left_probe_xz.x, 1);
    let upper_right_probe_i = upper_right_probe_xz.y * 2 + u32::min(upper_right_probe_xz.x, 1);
    let lower_left_probe_i = lower_left_probe_xz.y * 2 + u32::min(lower_left_probe_xz.x, 1);
    let lower_right_probe_i = lower_right_probe_xz.y * 2 + u32::min(lower_right_probe_xz.x, 1);
    let probes = [
        Cascade1NearestProbe {
            index: upper_left_probe_i as usize,
            interp_factor: upper_left_probe_interp_factor,
        },
        Cascade1NearestProbe {
            index: upper_right_probe_i as usize,
            interp_factor: upper_right_probe_interp_factor,
        },
        Cascade1NearestProbe {
            index: lower_left_probe_i as usize,
            interp_factor: lower_left_probe_interp_factor,
        },
        Cascade1NearestProbe {
            index: lower_right_probe_i as usize,
            interp_factor: lower_right_probe_interp_factor,
        },
    ];
    let probes_diff = (lower_right_probe_01 - upper_left_probe_01).abs();
    Cascade1NearestProbeInfo {
        probes,
        interp_divisor: probes_diff.x * probes_diff.y,
    }
}

pub fn get_nearest_previous_cascade_probes(
    current_cascade_probe_pos: Vec2,
    previous_cascade_probe_dim_len: u32,
) -> Cascade1NearestProbeInfo {
    let prev_cascade_num_probes = previous_cascade_probe_dim_len.pow(2);
    let probe_factor_u32 = prev_cascade_num_probes - 1;
    let probe_factor_f32 = probe_factor_u32 as f32;
    let pos_01 = vec2(
        current_cascade_probe_pos.x + 0.5,
        0.5 - current_cascade_probe_pos.y,
    );
    let pos_top_left = pos_01 - (1.0 / 32.0);
    let probe_xz_f32s = (pos_top_left * probe_factor_f32) / (1.0 - (1.0 / 16.0));
    let floor_xz = probe_xz_f32s.floor();
    let ceil_xz = probe_xz_f32s.ceil();
    let upper_left_probe_01 = (floor_xz * (1.0 - (1.0 / 16.0))) / probe_factor_f32;
    let lower_right_probe_01 = (ceil_xz * (1.0 - (1.0 / 16.0))) / probe_factor_f32;
    let upper_left_probe_diff = (pos_top_left - upper_left_probe_01).abs();
    let lower_right_probe_diff = (pos_top_left - lower_right_probe_01).abs();
    let upper_left_probe_interp_factor = lower_right_probe_diff.x * lower_right_probe_diff.y;
    let upper_right_probe_interp_factor = upper_left_probe_diff.x * lower_right_probe_diff.y;
    let lower_left_probe_interp_factor = lower_right_probe_diff.x * upper_left_probe_diff.y;
    let lower_right_probe_interp_factor = upper_left_probe_diff.x * upper_left_probe_diff.y;
    let upper_left_probe_xz = UVec2::min(floor_xz.as_uvec2(), UVec2::splat(probe_factor_u32));
    let upper_right_probe_xz = UVec2::min(
        vec2(ceil_xz.x, floor_xz.y).as_uvec2(),
        UVec2::splat(probe_factor_u32),
    );
    let lower_left_probe_xz = UVec2::min(
        vec2(floor_xz.x, ceil_xz.y).as_uvec2(),
        UVec2::splat(probe_factor_u32),
    );
    let lower_right_probe_xz = UVec2::min(ceil_xz.as_uvec2(), UVec2::splat(probe_factor_u32));
    let upper_left_probe_i = upper_left_probe_xz.y * prev_cascade_num_probes
        + u32::min(upper_left_probe_xz.x, probe_factor_u32);
    let upper_right_probe_i = upper_right_probe_xz.y * prev_cascade_num_probes
        + u32::min(upper_right_probe_xz.x, probe_factor_u32);
    let lower_left_probe_i = lower_left_probe_xz.y * prev_cascade_num_probes
        + u32::min(lower_left_probe_xz.x, probe_factor_u32);
    let lower_right_probe_i = lower_right_probe_xz.y * prev_cascade_num_probes
        + u32::min(lower_right_probe_xz.x, probe_factor_u32);
    let probes = [
        Cascade1NearestProbe {
            index: upper_left_probe_i as usize,
            interp_factor: upper_left_probe_interp_factor,
        },
        Cascade1NearestProbe {
            index: upper_right_probe_i as usize,
            interp_factor: upper_right_probe_interp_factor,
        },
        Cascade1NearestProbe {
            index: lower_left_probe_i as usize,
            interp_factor: lower_left_probe_interp_factor,
        },
        Cascade1NearestProbe {
            index: lower_right_probe_i as usize,
            interp_factor: lower_right_probe_interp_factor,
        },
    ];
    let probes_diff = (lower_right_probe_01 - upper_left_probe_01).abs();
    Cascade1NearestProbeInfo {
        probes,
        interp_divisor: probes_diff.x * probes_diff.y,
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct CascadeUpdateInfo {
    pub subchunk_start_coords: [f32; 3],
    pub faces_start: u32,
    pub faces_len: u32,
    pub faces_dir_i: u32,
}

// macro_rules! define_cascade {
//     (
//         $name:ident,
//         $probe_dim_len:expr,
//         $previous_cascade_probe_dim_len:expr,
//         $next_cascade_dim_len:expr,
//         $num_rays:expr,
//         $previous_cascade_num_rays:expr,
//     ) => {
//         #[cfg(not(feature = "graphics_backend_vulkan"))]
//         #[inline(never)]
//         fn $name(
//             ray_query: &mut RayQuery,
//             block_item_atlas: &Image2d,
//             block_item_luma_atlas: &Image2d,
//             world_tlas: &AccelerationStructure,
//             instances: &[TlasInstanceInfo],
//             quads_info: &[RayTracedQuadInfo],
//             update_face_matrix: &Mat3A,
//             update_face_block_centre: &Vec3,
//             results: &mut [[u32; $previous_cascade_num_rays]; $probe_dim_len * $probe_dim_len],
//             next_cascade_results: &[[u32; $num_rays];
//                  $next_cascade_dim_len * $next_cascade_dim_len],
//             ray_length: f32,
//         ) {
//             const ZERO_DIRS: [Vec3; $previous_cascade_num_rays] =
//                 [Vec3::ZERO; $previous_cascade_num_rays];
//             let mut ray_dirs = ZERO_DIRS;
//             for i in 0..ray_dirs.len() {
//                 ray_dirs[i] = get_ray_direction($previous_cascade_num_rays, i).into();
//             }
//             let num_probes = $probe_dim_len * $probe_dim_len;
//             for probe_i in 0..num_probes {
//                 let probe_face_local_pos =
//                     get_cascade_probe_local_position(probe_i, $probe_dim_len);
//                 let probe_block_local_pos = *update_face_matrix * probe_face_local_pos;
//                 let probe_global_pos = *update_face_block_centre + probe_block_local_pos;
//                 // Cast rays
//                 const ZERO_RAY_RESULTS: [Vec4; $previous_cascade_num_rays] =
//                     [Vec4::ZERO; $previous_cascade_num_rays];
//                 let mut ray_results = ZERO_RAY_RESULTS;
//                 const ZERO_DOT_SUMS: [f32; $previous_cascade_num_rays] =
//                     [0.0; $previous_cascade_num_rays];
//                 let mut ray_dot_sums = ZERO_DOT_SUMS;
//                 for ray_i in 0..$num_rays {
//                     // Ray info
//                     let ray_dir = *update_face_matrix * get_ray_direction($num_rays, ray_i);
//                     let ray_start = probe_global_pos;
//                     let ray_hit_rgba = trace_ray(
//                         ray_query,
//                         block_item_atlas,
//                         block_item_luma_atlas,
//                         world_tlas,
//                         instances,
//                         quads_info,
//                         ray_start,
//                         ray_dir,
//                         // previous_cascade_ray_length,
//                         0.0,
//                         ray_length,
//                     );
//                     // Average nearest four next cascade probes, combine with this probe
//                     let next_cascade_probes = get_nearest_previous_cascade_probes(
//                         probe_face_local_pos.xz(),
//                         $next_cascade_dim_len,
//                     );
//                     let mut average_next_cascade_probe_rgb = Vec3::ZERO;
//                     let mut backup_colour = Vec3::default();
//                     for cascade_3_probe_i in 0..next_cascade_probes.probes.len() {
//                         let probe_info = next_cascade_probes.probes[cascade_3_probe_i];
//                         let ray_rgb =
//                             rgba_8888_to_vec4(next_cascade_results[probe_info.index][ray_i]).xyz();
//                         average_next_cascade_probe_rgb += ray_rgb * probe_info.interp_factor;
//                         backup_colour = ray_rgb;
//                     }
//                     let cascade_3_final_colour = if next_cascade_probes.interp_divisor < 0.0001 {
//                         backup_colour
//                     } else {
//                         average_next_cascade_probe_rgb / next_cascade_probes.interp_divisor
//                     };
//                     let contribution_rgb = cascade_3_final_colour * (1.0 - ray_hit_rgba.w);
//                     let ray_final_colour = ray_hit_rgba + Vec4::from((contribution_rgb, 0.0));
//                     for reduced_ray_i in 0..$previous_cascade_num_rays {
//                         let reduced_ray_dir = ray_dirs[reduced_ray_i];
//                         let facing_coef = Vec3::dot(reduced_ray_dir, ray_dir).clamp(0.0, 1.0);
//                         ray_results[reduced_ray_i] += ray_final_colour * facing_coef;
//                         ray_dot_sums[reduced_ray_i] += facing_coef;
//                     }
//                 }
//                 // Store result
//                 for ray_i in 0..$previous_cascade_num_rays {
//                     let probe_rgba = ray_results[ray_i] / ray_dot_sums[ray_i];
//                     results[probe_i][ray_i] = rgba_vec4_to_8888(probe_rgba);
//                 }
//             }
//         }
//     };
// }
//
// define_cascade!(
//     calculate_cascade_3,
//     2,
//     4,
//     1,
//     CASCADE_3_NUM_RAYS,
//     CASCADE_2_NUM_RAYS,
// );
//
// define_cascade!(
//     calculate_cascade_2,
//     4,
//     8,
//     2,
//     CASCADE_2_NUM_RAYS,
//     CASCADE_1_NUM_RAYS,
// );
//
// define_cascade!(
//     calculate_cascade_1,
//     8,
//     16,
//     4,
//     CASCADE_1_NUM_RAYS,
//     CASCADE_0_NUM_RAYS,
// );

type WorkingLightmap = [u32; 256 * (CASCADE_0_NUM_RAYS / 4)];

#[cfg(not(feature = "graphics_backend_vulkan"))]
// #[spirv(compute(threads(16)))]
#[spirv(compute(threads(1)))]
pub fn update_all_cascades(
    #[spirv(descriptor_set = 0, binding = 0)] block_item_atlas: &Image2d,
    #[spirv(descriptor_set = 0, binding = 1)] block_item_luma_atlas: &Image2d,
    #[spirv(descriptor_set = 0, binding = 2)] world_tlas: &AccelerationStructure,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] instances: &[TlasInstanceInfo],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] quads_info: &[RayTracedQuadInfo],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] updates: &[CascadeUpdateInfo],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] block_faces: &[BlockFaceInstance],
    #[spirv(storage_buffer, descriptor_set = 1, binding = 0)] output_lightmap: &mut [[u32; 256]],
    #[spirv(uniform, descriptor_set = 2, binding = 0)] face_matrices: &[Mat3A; 6],
    // #[spirv(workgroup)] working_lightmaps: &mut (WorkingLightmap, WorkingLightmap),
    // #[spirv(workgroup)] mut working_lightmap_1: &mut [u32; 256],
    // #[spirv(workgroup)] mut working_lightmap_2: &mut [u32; 256],
    #[spirv(workgroup_id)] workgroup_id: UVec3,
    #[spirv(local_invocation_id)] local_id: UVec3,
) {
    const WORKING_LIGHTMAP: WorkingLightmap = [0; 256 * (CASCADE_0_NUM_RAYS / 4)];
    // let mut working_lightmaps = (WORKING_LIGHTMAP, WORKING_LIGHTMAP);
    let update_info = updates[workgroup_id.y as usize];
    if workgroup_id.x >= update_info.faces_len {
        return;
    }
    spirv_ray_query!(let mut ray_query);
    // We're casting rays from a probe in a block face, so first we need the face info to calculate
    // the probe info.
    let update_face_matrix = face_matrices[update_info.faces_dir_i as usize];
    let update_face_i = update_info.faces_start + workgroup_id.x;
    let update_face_packed_fields = block_faces[update_face_i as usize].packed_fields;
    let update_face_offset = UVec3::new(
        update_face_packed_fields.x_offset(),
        update_face_packed_fields.y_offset(),
        update_face_packed_fields.z_offset(),
    )
    .as_vec3();
    let subchunk_start_coords = Vec3::from(update_info.subchunk_start_coords);
    let update_face_block_centre = subchunk_start_coords + update_face_offset + 0.5;

    // macro_rules! run_cascade {
    //     (
    //         let mut $results:ident;
    //         $previous_cascade_results:expr,
    //         $probe_dim_len:expr,
    //         $previous_cascade_probe_dim_len:expr,
    //         $ray_length:expr,
    //         $num_rays:expr,
    //         $previous_cascade_num_rays:expr $(,)?
    //     ) => {
    //         let mut $results;
    //         {
    //             const NUM_PROBES: usize = $probe_dim_len * $probe_dim_len;
    //             type ResultArray = [[u32; $previous_cascade_num_rays]; NUM_PROBES];
    //             const ZERO_RESULTS: ResultArray =
    //                 [[0; $previous_cascade_num_rays]; NUM_PROBES];
    //             $results = ZERO_RESULTS;
    //         }
    //         {
    //             // const _ASSERT1: () = assert!(
    //             //     $probe_dim_len * $probe_dim_len == { $previous_cascade_results.len() }
    //             // );
    //             const ZERO_DIRS: [Vec3; $previous_cascade_num_rays] =
    //                 [Vec3::ZERO; $previous_cascade_num_rays];
    //             let mut ray_dirs = ZERO_DIRS;
    //             const ZERO_RAY_RESULTS: [Vec4; $previous_cascade_num_rays] =
    //                 [Vec4::ZERO; $previous_cascade_num_rays];
    //             let mut ray_results = ZERO_RAY_RESULTS;
    //             const ZERO_DOT_SUMS: [f32; $previous_cascade_num_rays] =
    //                 [0.0; $previous_cascade_num_rays];
    //             let mut ray_dot_sums = ZERO_DOT_SUMS;
    //             calculate_cascade::<$num_rays, $previous_cascade_num_rays>(
    //                 block_item_atlas,
    //                 block_item_luma_atlas,
    //                 world_tlas,
    //                 instances,
    //                 quads_info,
    //                 &update_face_matrix,
    //                 &update_face_block_centre,
    //                 &mut $results,
    //                 $previous_cascade_results,
    //                 $probe_dim_len,
    //                 $previous_cascade_probe_dim_len,
    //                 $ray_length,
    //                 $num_rays,
    //                 $previous_cascade_num_rays,
    //                 &mut ray_dirs,
    //                 &mut ray_results,
    //                 &mut ray_dot_sums,
    //             );
    //         }
    //     };
    // }

    // macro_rules! run_cascade {
    //     (
    //         let mut $results:ident;
    //         $name:ident,
    //         $previous_cascade_results:expr,
    //         $probe_dim_len:expr,
    //         $previous_cascade_probe_dim_len:expr,
    //         $ray_length:expr,
    //         $num_rays:expr,
    //         $previous_cascade_num_rays:expr $(,)?
    //     ) => {
    //         let mut $results;
    //         {
    //             const NUM_PROBES: usize = $probe_dim_len * $probe_dim_len;
    //             type ResultArray = [[u32; $previous_cascade_num_rays]; NUM_PROBES];
    //             const ZERO_RESULTS: ResultArray =
    //                 [[0; $previous_cascade_num_rays]; NUM_PROBES];
    //             $results = ZERO_RESULTS;
    //         }
    //         $name(
    //             block_item_atlas,
    //             block_item_luma_atlas,
    //             world_tlas,
    //             instances,
    //             quads_info,
    //             &update_face_matrix,
    //             &update_face_block_centre,
    //             &mut $results,
    //             $previous_cascade_results,
    //             $ray_length,
    //         );
    //     };
    // }

    // const SKYBOX_RESULTS: [[u32; CASCADE_3_NUM_RAYS]; 1] =
    //     [[0; CASCADE_3_NUM_RAYS]; 1];
    // let skybox_results = SKYBOX_RESULTS;
    // run_cascade!(
    //     let mut cascade_3_results;
    //     calculate_cascade_3,
    //     &skybox_results,
    //     2,
    //     4,
    //     CASCADE_3_RAY_LENGTH,
    //     CASCADE_3_NUM_RAYS,
    //     CASCADE_2_NUM_RAYS,
    // );
    //
    // run_cascade!(
    //     let mut cascade_2_results;
    //     calculate_cascade_2,
    //     &cascade_3_results,
    //     4,
    //     8,
    //     CASCADE_2_RAY_LENGTH,
    //     CASCADE_2_NUM_RAYS,
    //     CASCADE_1_NUM_RAYS,
    // );
    //
    // run_cascade!(
    //     let mut cascade_1_results;
    //     calculate_cascade_1,
    //     &cascade_2_results,
    //     8,
    //     16,
    //     CASCADE_1_RAY_LENGTH,
    //     CASCADE_1_NUM_RAYS,
    //     CASCADE_0_NUM_RAYS,
    // );

    // // Calculate cascade 3
    // let cascade_3_results = &mut working_lightmaps.0;
    // // if local_id.x < 4 {
    // //     let probe_i = local_id.x as usize;
    // for probe_i in 0..4 {
    //     let probe_face_local_pos = get_cascade_3_probe_local_position(probe_i);
    //     let probe_block_local_pos = update_face_matrix * probe_face_local_pos;
    //     let probe_global_pos = update_face_block_centre + probe_block_local_pos;
    //     let cascade_2_ray_dirs = {
    //         const ZERO_DIRS: [Vec3; CASCADE_2_NUM_RAYS] = [Vec3::ZERO; CASCADE_2_NUM_RAYS];
    //         let mut dirs = ZERO_DIRS;
    //         for i in 0..CASCADE_2_NUM_RAYS {
    //             dirs[i] = get_ray_direction(CASCADE_2_NUM_RAYS, i).into();
    //         }
    //         dirs
    //     };
    //     // Cast rays
    //     const ZERO_RESULTS: [Vec4; CASCADE_2_NUM_RAYS] = [Vec4::ZERO; CASCADE_2_NUM_RAYS];
    //     let mut ray_results = ZERO_RESULTS;
    //     let mut ray_dot_sums: [f32; CASCADE_2_NUM_RAYS] = [0.0; CASCADE_2_NUM_RAYS];
    //     for ray_i in 0..CASCADE_3_NUM_RAYS {
    //         // Ray info
    //         let ray_dir = update_face_matrix * get_ray_direction(CASCADE_3_NUM_RAYS, ray_i);
    //         let ray_start = probe_global_pos;
    //         let ray_hit_rgba = trace_ray(
    //             ray_query,
    //             block_item_atlas,
    //             block_item_luma_atlas,
    //             world_tlas,
    //             instances,
    //             quads_info,
    //             ray_start,
    //             ray_dir,
    //             // CASCADE_2_RAY_LENGTH,
    //             0.0001,
    //             CASCADE_3_RAY_LENGTH,
    //         );
    //         // let facing_sun = if Vec3::dot(ray_dir, vec3(1.0, 4.0, 2.0).normalize()) >= 0.95 {
    //         //     1.0
    //         // } else {
    //         //     0.0
    //         // };
    //         // let skybox_colour = Vec3::lerp(Vec3::splat(0.75), Vec3::splat(1.0), facing_sun);
    //         // let ray_rgb = Vec3::lerp(skybox_colour, ray_rgb, ray_hit_rgba.w);
    //         for reduced_ray_i in 0..CASCADE_2_NUM_RAYS {
    //             let reduced_ray_dir = cascade_2_ray_dirs[reduced_ray_i];
    //             let facing_coef = Vec3::dot(reduced_ray_dir, ray_dir).clamp(0.0, 1.0);
    //             ray_results[reduced_ray_i] += ray_hit_rgba * facing_coef;
    //             ray_dot_sums[reduced_ray_i] += facing_coef;
    //         }
    //     }
    //     // Store result
    //     for ray_i in 0..CASCADE_2_NUM_RAYS {
    //         let probe_rgba = ray_results[ray_i] / ray_dot_sums[ray_i];
    //         cascade_3_results[probe_i * CASCADE_2_NUM_RAYS + ray_i] = rgba_vec4_to_10_10_10_2(probe_rgba);
    //     }
    // }
    //
    // // unsafe {
    // //     spirv_std::arch::workgroup_memory_barrier_with_group_sync();
    // // }
    //
    // // Calculate cascade 2
    // let cascade_2_results = &mut working_lightmaps.1;
    // // {
    // //     let probe_i = local_id.x as usize;
    // for probe_i in 0..16 {
    //     let probe_face_local_pos = get_cascade_2_probe_local_position(probe_i);
    //     let probe_block_local_pos = update_face_matrix * probe_face_local_pos;
    //     let probe_global_pos = update_face_block_centre + probe_block_local_pos;
    //     let cascade_1_ray_dirs = {
    //         const ZERO_DIRS: [Vec3; CASCADE_1_NUM_RAYS] = [Vec3::ZERO; CASCADE_1_NUM_RAYS];
    //         let mut dirs = ZERO_DIRS;
    //         for i in 0..CASCADE_1_NUM_RAYS {
    //             dirs[i] = get_ray_direction(CASCADE_1_NUM_RAYS, i).into();
    //         }
    //         dirs
    //     };
    //     // Cast rays
    //     const ZERO_RESULTS: [Vec4; CASCADE_1_NUM_RAYS] = [Vec4::ZERO; CASCADE_1_NUM_RAYS];
    //     let mut ray_results = ZERO_RESULTS;
    //     let mut ray_dot_sums: [f32; CASCADE_1_NUM_RAYS] = [0.0; CASCADE_1_NUM_RAYS];
    //     for ray_i in 0..CASCADE_2_NUM_RAYS {
    //         // Ray info
    //         let ray_dir = update_face_matrix * get_ray_direction(CASCADE_2_NUM_RAYS, ray_i);
    //         let ray_start = probe_global_pos;
    //         let ray_hit_rgba = trace_ray(
    //             ray_query,
    //             block_item_atlas,
    //             block_item_luma_atlas,
    //             world_tlas,
    //             instances,
    //             quads_info,
    //             ray_start,
    //             ray_dir,
    //             // CASCADE_1_RAY_LENGTH,
    //             0.0001,
    //             CASCADE_2_RAY_LENGTH,
    //         );
    //         // Average nearest four cascade 3 probes, combine with this probe
    //         let cascade_3_probes = get_nearest_cascade_3_probes(probe_face_local_pos.xz());
    //         let mut average_cascade_3_probe_rgb = Vec3::ZERO;
    //         let mut backup_colour = Vec3::default();
    //         for cascade_3_probe_i in 0..cascade_3_probes.probes.len() {
    //             let probe_info = cascade_3_probes.probes[cascade_3_probe_i];
    //             // let ray_rgb = rgba_10_10_10_2_to_vec4(cascade_3_results[probe_info.index][ray_i]).xyz();
    //             let ray_rgb =
    //                 rgba_10_10_10_2_to_vec4(cascade_3_results[probe_info.index * CASCADE_2_NUM_RAYS + ray_i]).xyz();
    //             average_cascade_3_probe_rgb += ray_rgb * probe_info.interp_factor;
    //             backup_colour = ray_rgb;
    //         }
    //         let cascade_3_final_colour = if cascade_3_probes.interp_divisor < 0.0001 {
    //             backup_colour
    //         } else {
    //             average_cascade_3_probe_rgb / cascade_3_probes.interp_divisor
    //         };
    //         let contribution_rgb = cascade_3_final_colour * (1.0 - ray_hit_rgba.w);
    //         let ray_final_colour = ray_hit_rgba + Vec4::from((contribution_rgb, 0.0));
    //         for reduced_ray_i in 0..CASCADE_1_NUM_RAYS {
    //             let reduced_ray_dir = cascade_1_ray_dirs[reduced_ray_i];
    //             let facing_coef = Vec3::dot(reduced_ray_dir, ray_dir).clamp(0.0, 1.0);
    //             ray_results[reduced_ray_i] += ray_final_colour * facing_coef;
    //             ray_dot_sums[reduced_ray_i] += facing_coef;
    //         }
    //     }
    //     // Store result
    //     for ray_i in 0..CASCADE_1_NUM_RAYS {
    //         let probe_rgba = ray_results[ray_i] / ray_dot_sums[ray_i];
    //         cascade_2_results[probe_i * CASCADE_1_NUM_RAYS + ray_i] = rgba_vec4_to_10_10_10_2(probe_rgba);
    //     }
    // }
    //
    // // unsafe {
    // //     spirv_std::arch::workgroup_memory_barrier_with_group_sync();
    // // }

    // TODO:
    // - Split block face into smaller square groups
    // - Adjust higher cascade probe positions to be in smaller squares
    // - Less stack space should be used, so can turn up cascade 0 rays

    // Calculate cascade 1
    // let cascade_1_results = &mut working_lightmaps.0;
    // for group_probe_i in 0..4 {
    //     let probe_i = local_id.x as usize * 4 + group_probe_i;
    let mut cascade_1_results = WORKING_LIGHTMAP;
    for probe_i in 0..64 {
        let probe_face_local_pos = get_cascade_1_probe_local_position(probe_i);
        let probe_block_local_pos = update_face_matrix * probe_face_local_pos;
        let probe_global_pos = update_face_block_centre + probe_block_local_pos;
        let cascade_0_ray_dirs = {
            let mut dirs: [Vec3; CASCADE_0_NUM_RAYS] = Default::default();
            for i in 0..CASCADE_0_NUM_RAYS {
                dirs[i] = get_ray_direction(CASCADE_0_NUM_RAYS, i).into();
            }
            dirs
        };
        // Cast rays
        let mut ray_results: [Vec4; CASCADE_0_NUM_RAYS] = Default::default();
        let mut ray_dot_sums: [f32; CASCADE_0_NUM_RAYS] = [0.0; CASCADE_0_NUM_RAYS];

        for ray_i in 0..CASCADE_1_NUM_RAYS {
            // Ray info
            let ray_dir = update_face_matrix * get_ray_direction(CASCADE_1_NUM_RAYS, ray_i);
            let ray_start = probe_global_pos;
            let ray_hit_rgba = trace_ray(
                ray_query,
                block_item_atlas,
                block_item_luma_atlas,
                world_tlas,
                instances,
                quads_info,
                ray_start,
                ray_dir,
                // CASCADE_0_RAY_LENGTH,
                0.0001,
                CASCADE_1_RAY_LENGTH,
            );
            // // Average nearest four cascade 2 probes, combine with this probe
            // let cascade_2_probes = get_nearest_cascade_2_probes(probe_face_local_pos.xz());
            // let mut average_cascade_2_probe_rgb = Vec3::ZERO;
            // let mut backup_colour = Vec3::default();
            // for cascade_2_probe_i in 0..cascade_2_probes.probes.len() {
            //     let probe_info = cascade_2_probes.probes[cascade_2_probe_i];
            //     // let ray_rgb = rgba_10_10_10_2_to_vec4(cascade_2_results[probe_info.index][ray_i]).xyz();
            //     let ray_rgb =
            //         rgba_10_10_10_2_to_vec4(cascade_2_results[probe_info.index * CASCADE_1_NUM_RAYS + ray_i]).xyz();
            //     average_cascade_2_probe_rgb += ray_rgb * probe_info.interp_factor;
            //     backup_colour = ray_rgb;
            // }
            // let cascade_2_final_colour = if cascade_2_probes.interp_divisor < 0.0001 {
            //     backup_colour
            // } else {
            //     average_cascade_2_probe_rgb / cascade_2_probes.interp_divisor
            // };
            // let contribution_rgb = cascade_2_final_colour * (1.0 - ray_hit_rgba.w);
            // let ray_final_colour = ray_hit_rgba + Vec4::from((contribution_rgb, 0.0));
            let ray_final_colour = ray_hit_rgba;
            for reduced_ray_i in 0..CASCADE_0_NUM_RAYS {
                let reduced_ray_dir = cascade_0_ray_dirs[reduced_ray_i];
                let facing_coef = Vec3::dot(reduced_ray_dir, ray_dir).clamp(0.0, 1.0);
                ray_results[reduced_ray_i] += ray_final_colour * facing_coef;
                ray_dot_sums[reduced_ray_i] += facing_coef;
            }
        }
        // Store result
        for ray_i in 0..CASCADE_0_NUM_RAYS {
            let probe_rgba = ray_results[ray_i] / ray_dot_sums[ray_i];
            cascade_1_results[probe_i * CASCADE_0_NUM_RAYS + ray_i] =
                rgba_vec4_to_10_10_10_2(probe_rgba);
        }
    }

    // unsafe {
    //     spirv_std::arch::workgroup_memory_barrier_with_group_sync();
    // }

    // Calculate cascade 0
    // for group_probe_i in 0..16 {
    //     let probe_i = local_id.x as usize * 16 + group_probe_i;
    for probe_i in 0..256 {
        let probe_face_local_pos = get_cascade_0_probe_local_position(probe_i);
        let probe_block_local_pos = update_face_matrix * probe_face_local_pos;
        let probe_global_pos = update_face_block_centre + probe_block_local_pos;

        // Cast rays, collect hits
        let mut result_acc = Vec4::ZERO;
        for ray_i in 0..CASCADE_0_NUM_RAYS {
            // Ray info
            let ray_dir = update_face_matrix * get_ray_direction(CASCADE_0_NUM_RAYS, ray_i);
            let ray_start = probe_global_pos;
            let ray_hit_colour = trace_ray(
                ray_query,
                block_item_atlas,
                block_item_luma_atlas,
                world_tlas,
                instances,
                quads_info,
                ray_start,
                ray_dir,
                0.0001,
                CASCADE_0_RAY_LENGTH,
            );
            // Average nearest four cascade 1 probes, combine with this probe
            let cascade_1_probes = get_nearest_cascade_1_probes(probe_face_local_pos.xz());
            let mut average_cascade_1_probe_rgb = Vec3::ZERO;
            let mut backup_colour = Vec3::default();
            for cascade_1_probe_i in 0..cascade_1_probes.probes.len() {
                let probe_info = cascade_1_probes.probes[cascade_1_probe_i];
                let ray_rgb = rgba_10_10_10_2_to_vec4(
                    cascade_1_results[probe_info.index * CASCADE_0_NUM_RAYS + ray_i],
                )
                .xyz();
                average_cascade_1_probe_rgb += ray_rgb * probe_info.interp_factor;
                backup_colour = ray_rgb;
            }
            let cascade_1_final_colour = if cascade_1_probes.interp_divisor < 0.0001 {
                backup_colour
            } else {
                average_cascade_1_probe_rgb / cascade_1_probes.interp_divisor
            };
            let contribution_rgb = cascade_1_final_colour * (1.0 - ray_hit_colour.w);
            let ray_final_colour = ray_hit_colour + Vec4::from((contribution_rgb, 0.0));
            result_acc += ray_final_colour / CASCADE_0_NUM_RAYS as f32;
        }
        // Store result
        let average = result_acc;
        // output_lightmap[update_face_i as usize][probe_i] = rgb_to_hsv448(average.xyz());
        output_lightmap[update_face_i as usize][probe_i] = rgba_vec4_to_10_10_10_2(average);
    }
}

#[cfg(not(feature = "graphics_backend_vulkan"))]
#[spirv(compute(threads(256)))]
pub fn single_pass_update(
    #[spirv(descriptor_set = 0, binding = 0)] block_item_atlas: &Image2d,
    #[spirv(descriptor_set = 0, binding = 1)] block_item_luma_atlas: &Image2d,
    #[spirv(descriptor_set = 0, binding = 2)] world_tlas: &AccelerationStructure,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] instances: &[TlasInstanceInfo],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] quads_info: &[RayTracedQuadInfo],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] updates: &[CascadeUpdateInfo],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] block_faces: &[BlockFaceInstance],
    #[spirv(storage_buffer, descriptor_set = 1, binding = 0)] output_lightmap: &mut [[u32; 256]],
    #[spirv(uniform, descriptor_set = 2, binding = 0)] face_matrices: &[Mat3A; 6],
    #[spirv(workgroup_id)] workgroup_id: UVec3,
    #[spirv(local_invocation_id)] local_id: UVec3,
) {
    let update_info = updates[workgroup_id.y as usize];
    if workgroup_id.x >= update_info.faces_len {
        return;
    }
    // We're casting rays from a probe in a block face, so first we need the face info to calculate
    // the probe info.
    let (update_face_matrix, ray_start) = {
        let update_face_i = update_info.faces_start + workgroup_id.x;
        let probe_i = local_id.x as usize;
        let update_face_matrix = face_matrices[update_info.faces_dir_i as usize];
        let update_face_packed_fields = block_faces[update_face_i as usize].packed_fields;
        let update_face_offset = UVec3::new(
            update_face_packed_fields.x_offset(),
            update_face_packed_fields.y_offset(),
            update_face_packed_fields.z_offset(),
        )
        .as_vec3();
        let subchunk_start_coords = Vec3::from(update_info.subchunk_start_coords);
        let update_face_block_centre = subchunk_start_coords + update_face_offset + 0.5;
        let probe_face_local_pos = get_cascade_0_probe_local_position(probe_i);
        let probe_block_local_pos = update_face_matrix * probe_face_local_pos;
        let probe_global_pos = update_face_block_centre + probe_block_local_pos;
        (update_face_matrix, probe_global_pos)
    };

    // Cast rays, collect hits
    const NUM_RAYS: usize = 32768;
    const RAY_LENGTH: f32 = 16.0;
    spirv_ray_query!(let mut ray_query);
    let mut result_acc = Vec4::ZERO;
    for ray_i in 0..NUM_RAYS {
        // Ray info
        let ray_dir = update_face_matrix * get_ray_direction(NUM_RAYS, ray_i);
        let ray_hit_colour = trace_ray(
            ray_query,
            block_item_atlas,
            block_item_luma_atlas,
            world_tlas,
            instances,
            quads_info,
            ray_start,
            ray_dir,
            0.0001,
            RAY_LENGTH,
        );
        let facing_sun = if Vec3::dot(ray_dir, vec3(1.0, 3.0, 2.0).normalize()) >= 0.99 {
            1.0
        } else {
            0.0
        };
        let skybox_colour = Vec3::lerp(Vec3::splat(0.0), Vec3::splat(16.0), facing_sun);
        let ray_rgb = Vec3::lerp(skybox_colour, ray_hit_colour.xyz(), ray_hit_colour.w);
        result_acc += Vec4::from((ray_rgb, 1.0)) / NUM_RAYS as f32;
    }

    // Store result
    let average = result_acc;
    let update_face_i = update_info.faces_start + workgroup_id.x;
    let probe_i = local_id.x as usize;
    // output_lightmap[update_face_i as usize][probe_i] = rgb_to_hsv448(average.xyz());
    // output_lightmap[update_face_i as usize][probe_i] = rgba_vec4_to_8888(average);
    output_lightmap[update_face_i as usize][probe_i] = rgba_vec4_to_10_10_10_2(average);
}

#[cfg(not(feature = "graphics_backend_vulkan"))]
#[spirv(compute(threads(1, 256)))]
pub fn update_cascade_0(
    #[spirv(descriptor_set = 0, binding = 0)] block_item_atlas: &Image2d,
    #[spirv(descriptor_set = 0, binding = 1)] block_item_luma_atlas: &Image2d,
    #[spirv(descriptor_set = 0, binding = 2)] world_tlas: &AccelerationStructure,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] instances: &[TlasInstanceInfo],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] quads_info: &[RayTracedQuadInfo],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] updates: &[CascadeUpdateInfo],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] block_faces: &[BlockFaceInstance],
    #[spirv(storage_buffer, descriptor_set = 1, binding = 0)] output_lightmap: &mut [[u16; 256]],
    #[rustfmt::skip]
    #[spirv(storage_buffer, descriptor_set = 1, binding = 1)]
    cascade_1_lightmap: &[[[u16; CASCADE_0_NUM_RAYS]; 64]],
    #[spirv(uniform, descriptor_set = 2, binding = 0)] face_matrices: &[Mat3A; 6],
    #[spirv(workgroup_id)] workgroup_id: UVec3,
    #[spirv(local_invocation_id)] local_id: UVec3,
) {
    let update_info = updates[workgroup_id.y as usize];
    if workgroup_id.x >= update_info.faces_len {
        return;
    }
    spirv_ray_query!(let mut ray_query);
    // We're casting rays from a probe in a block face, so first we need the face info to calculate
    // the probe info.
    let update_face_matrix = face_matrices[update_info.faces_dir_i as usize];
    let update_face_i = update_info.faces_start + workgroup_id.x;
    let update_face_packed_fields = block_faces[update_face_i as usize].packed_fields;
    let update_face_offset = UVec3::new(
        update_face_packed_fields.x_offset(),
        update_face_packed_fields.y_offset(),
        update_face_packed_fields.z_offset(),
    )
    .as_vec3();
    let subchunk_start_coords = Vec3::from(update_info.subchunk_start_coords);
    let update_face_block_centre = subchunk_start_coords + update_face_offset + 0.5;
    let probe_i = local_id.y as usize;
    let probe_face_local_pos = get_cascade_0_probe_local_position(probe_i);
    let probe_block_local_pos = update_face_matrix * probe_face_local_pos;
    let probe_global_pos = update_face_block_centre + probe_block_local_pos;

    // Cast rays, collect hits
    let mut result_acc = Vec4::ZERO;
    for ray_i in 0..CASCADE_0_NUM_RAYS {
        // Ray info
        let ray_dir = update_face_matrix * get_ray_direction(CASCADE_0_NUM_RAYS, ray_i);
        let ray_start = probe_global_pos;
        let ray_hit_colour = trace_ray(
            ray_query,
            block_item_atlas,
            block_item_luma_atlas,
            world_tlas,
            instances,
            quads_info,
            ray_start,
            ray_dir,
            0.0,
            CASCADE_0_RAY_LENGTH,
        );
        // Average nearest four cascade 1 probes, combine with this probe
        let cascade_1_probes = get_nearest_cascade_1_probes(probe_face_local_pos.xz());
        let mut average_cascade_1_probe_rgb = Vec3::ZERO;
        let mut backup_colour = Vec3::default();
        for cascade_1_probe_i in 0..cascade_1_probes.probes.len() {
            let probe_info = cascade_1_probes.probes[cascade_1_probe_i];
            // let face = &cascade_1_lightmap[update_face_i as usize];
            // let probe = &face[probe_info.index];
            // let ray_hsv448 = probe[ray_i];
            let ray_hsv448 = cascade_1_lightmap[update_face_i as usize][probe_info.index][ray_i];
            let ray_rgb = hsv448_to_rgb(ray_hsv448);
            average_cascade_1_probe_rgb += ray_rgb * probe_info.interp_factor;
            backup_colour = ray_rgb;
        }
        let cascade_1_final_colour = if cascade_1_probes.interp_divisor < 0.0001 {
            backup_colour
        } else {
            average_cascade_1_probe_rgb / cascade_1_probes.interp_divisor
        };
        let contribution_rgb = cascade_1_final_colour * (1.0 - ray_hit_colour.w);
        let ray_final_colour = ray_hit_colour + Vec4::from((contribution_rgb, 0.0));
        result_acc += ray_final_colour / CASCADE_0_NUM_RAYS as f32;
    }

    // Store result
    let average = result_acc;
    // let average = result_acc / NUM_RAYS as f32;
    output_lightmap[update_face_i as usize][probe_i] = rgb_to_hsv448(average.xyz());
}

#[cfg(not(feature = "graphics_backend_vulkan"))]
#[spirv(compute(threads(1, 64)))]
pub fn update_cascade_1(
    #[spirv(descriptor_set = 0, binding = 0)] block_item_atlas: &Image2d,
    #[spirv(descriptor_set = 0, binding = 1)] block_item_luma_atlas: &Image2d,
    #[spirv(descriptor_set = 0, binding = 2)] world_tlas: &AccelerationStructure,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] instances: &[TlasInstanceInfo],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] quads_info: &[RayTracedQuadInfo],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] updates: &[CascadeUpdateInfo],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] block_faces: &[BlockFaceInstance],
    #[rustfmt::skip]
    #[spirv(storage_buffer, descriptor_set = 1, binding = 0)]
    output_lightmap: &mut [[[u16; CASCADE_0_NUM_RAYS]; 64]],
    #[spirv(uniform, descriptor_set = 2, binding = 0)] face_matrices: &[Mat3A; 6],
    #[spirv(workgroup_id)] workgroup_id: UVec3,
    #[spirv(local_invocation_id)] local_id: UVec3,
) {
    let update_info = updates[workgroup_id.y as usize];
    if workgroup_id.x >= update_info.faces_len {
        return;
    }
    spirv_ray_query!(let mut ray_query);
    // We're casting rays from a probe in a block face, so first we need the face info to calculate
    // the probe info.
    let update_face_matrix = face_matrices[update_info.faces_dir_i as usize];
    let update_face_i = update_info.faces_start + workgroup_id.x;
    let update_face_packed_fields = block_faces[update_face_i as usize].packed_fields;
    let update_face_offset = UVec3::new(
        update_face_packed_fields.x_offset(),
        update_face_packed_fields.y_offset(),
        update_face_packed_fields.z_offset(),
    )
    .as_vec3();
    let subchunk_start_coords = Vec3::from(update_info.subchunk_start_coords);
    let update_face_block_centre = subchunk_start_coords + update_face_offset + 0.5;
    let probe_i = local_id.y as usize;
    let probe_face_local_pos = get_cascade_1_probe_local_position(probe_i);
    let probe_block_local_pos = update_face_matrix * probe_face_local_pos;
    let probe_global_pos = update_face_block_centre + probe_block_local_pos;
    for ray_i in 0..CASCADE_0_NUM_RAYS {
        output_lightmap[update_face_i as usize][probe_i][ray_i] = rgb_to_hsv448(Vec3::X);
    }

    // Cast rays, collect hits
    let cascade_0_ray_dirs = {
        let mut dirs: [Vec3; CASCADE_0_NUM_RAYS] = Default::default();
        for i in 0..CASCADE_0_NUM_RAYS {
            dirs[i] = get_ray_direction(CASCADE_0_NUM_RAYS, i).into();
        }
        dirs
    };
    let mut ray_results: [Vec3; CASCADE_0_NUM_RAYS] = Default::default();
    let mut ray_dot_sums: [f32; CASCADE_0_NUM_RAYS] = [0.0; CASCADE_0_NUM_RAYS];
    for ray_i in 0..CASCADE_1_NUM_RAYS {
        // Ray info
        let ray_dir = update_face_matrix * get_ray_direction(CASCADE_1_NUM_RAYS, ray_i);
        let ray_start = probe_global_pos;
        let ray_hit_rgba = trace_ray(
            ray_query,
            block_item_atlas,
            block_item_luma_atlas,
            world_tlas,
            instances,
            quads_info,
            ray_start,
            ray_dir,
            CASCADE_0_RAY_LENGTH,
            CASCADE_1_RAY_LENGTH,
        );
        let ray_rgb = ray_hit_rgba.xyz();
        // let facing_sun = if Vec3::dot(ray_dir, vec3(1.0, 4.0, 2.0).normalize()) >= 0.95 {
        //     1.0
        // } else {
        //     0.0
        // };
        // let skybox_colour = Vec3::lerp(Vec3::splat(0.75), Vec3::splat(1.0), facing_sun);
        // let ray_rgb = Vec3::lerp(skybox_colour, ray_rgb, ray_hit_rgba.w);
        for reduced_ray_i in 0..CASCADE_0_NUM_RAYS {
            let reduced_ray_dir = cascade_0_ray_dirs[reduced_ray_i];
            let facing_coef = Vec3::dot(reduced_ray_dir, ray_dir).clamp(0.0, 1.0);
            ray_results[reduced_ray_i] += ray_rgb * facing_coef;
            ray_dot_sums[reduced_ray_i] += facing_coef;
        }
    }

    // Store result
    for ray_i in 0..CASCADE_0_NUM_RAYS {
        let probe_rgb = ray_results[ray_i] / ray_dot_sums[ray_i];
        output_lightmap[update_face_i as usize][probe_i][ray_i] = rgb_to_hsv448(probe_rgb);
    }
}
