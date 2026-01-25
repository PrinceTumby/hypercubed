use super::super::TextureAtlas;
use super::{
    RENDER_MICRO_TILE_DIM, RENDER_MICRO_TILE_PIXEL_DIM, RENDER_PIXEL_GROUP_DIM, RENDER_TILE_DIM,
    RENDER_TILE_PIXEL_DIM, RenderMicroTileDepth, RenderMicroTileRgba, RenderPixelGroupDepth,
    RenderPixelGroupRgba, RenderTileBins, RenderTileHiZChain, RenderTileRgba, Rgba, Rgba8Ne,
    rgba_4xunorm8x16_to_u32x16, rgba_u32x16_to_unorm8x16,
};
use crate::basic_types::AxisDirection;
use crate::graphics::chunk::{HasSubchunkData, SubchunkConnectivity, SubchunkData};
use crate::{MAX_HEIGHT_I32, MIN_HEIGHT_I32, SUBCHUNK_AXIS_LEN, SUBCHUNK_AXIS_LEN_I32};
use ahash::AHasher;
use bitfield::bitfield;
use core::hash::Hasher;
use core::num::NonZeroU32;
use core::simd::prelude::*;
use fixedbitset::FixedBitSet;
use nalgebra::{Matrix4, Point2, Point3, Rotation3, Vector2, Vector3};
use portable_std::{FastHashMap, FastHashSet};
use resources::block::RightAngleRotation;
use resources::block::blockstate::{self, BlockOpacity};
use resources::block::model::{ModelIndex, ModelRegistry, ModelType, Tint};
use resources::identifier;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

// TODO: Extend Hi-Z occlusion culling.
// - Currently we're just checking if an entire triangle is completely behind the entire tile.
// - If we had a way to quickly compute the max dist for a micro-tile, then we could compare that
//   directly with the micro-tile Hi-Z value to skip entire micro-tiles.
// - For partial tile draws, compare all micro-tile max dists against Hi-Zs in SIMD, then AND with
//   visible micro-tile mask used for iteration.
// - Could do the same for whole tile draws, just by changing the for-loop to a masked loop.
// - Micro-tiles used as example here, but also applies to pixel groups.

pub struct Subchunk {
    pub dispatch_id: u64,
    pub start_coords: Point3<i32>,
    pub connectivity: SubchunkConnectivity,
    pub block_faces: Option<Box<[BlockFace]>>,
    /// Zero if the direction group contains no faces.
    pub block_face_group_lengths: [u16; 6],
    pub tinted_block_faces: Option<Box<[TintedBlockFace]>>,
    /// Zero if the direction group contains no faces.
    pub tinted_block_face_group_lengths: [u16; 6],
    pub custom_block_faces: Option<Box<[CustomBlockFace]>>,
}

impl HasSubchunkData for Subchunk {
    fn get_data(&self) -> SubchunkData {
        SubchunkData {
            start_coords: self.start_coords.into(),
            connectivity: self.connectivity,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BlockFace {
    uvs: [u16; 4],
    /// 0-3: X offset
    /// 4-7: Y offset
    /// 8-11: Z offset
    /// 12-15: Unused
    packed_xyz: u16,
    /// 0-1: UV rotation
    /// 2-7: Unused
    /// 8-11: Sky light level
    /// 12-15: Block light level
    uv_rotation_and_light_levels: [u8; 2],
}

impl BlockFace {
    pub fn new(
        subchunk_xyz: [u8; 3],
        uvs: [u16; 4],
        uv_rotation: RightAngleRotation,
        light_levels: [u8; 2],
    ) -> Self {
        debug_assert!(subchunk_xyz[0] < 16);
        debug_assert!(subchunk_xyz[1] < 16);
        debug_assert!(subchunk_xyz[2] < 16);
        debug_assert!(light_levels[0] < 16);
        debug_assert!(light_levels[1] < 16);
        Self {
            uvs,
            packed_xyz: (subchunk_xyz[0] as u16)
                | ((subchunk_xyz[1] as u16) << 4)
                | ((subchunk_xyz[2] as u16) << 8),
            uv_rotation_and_light_levels: [
                match uv_rotation {
                    RightAngleRotation::Zero => 0,
                    RightAngleRotation::Ninety => 1,
                    RightAngleRotation::OneEighty => 2,
                    RightAngleRotation::TwoSeventy => 3,
                },
                light_levels[0] | (light_levels[1] << 4),
            ],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TintedBlockFace {
    uvs: [u16; 4],
    tint_colour: [u8; 4],
    /// 0-3: X offset
    /// 4-7: Y offset
    /// 8-11: Z offset
    /// 12-15: Unused
    packed_xyz: u16,
    /// 0-3: UV rotation
    /// 4-7: Unused
    /// 8-11: Sky light level
    /// 12-15: Block light level
    uv_rotation_and_light_levels: [u8; 2],
}

impl TintedBlockFace {
    pub fn new(
        subchunk_xyz: [u8; 3],
        uvs: [u16; 4],
        uv_rotation: RightAngleRotation,
        light_levels: [u8; 2],
        tint_colour: [u8; 4],
    ) -> Self {
        debug_assert!(subchunk_xyz[0] < 16);
        debug_assert!(subchunk_xyz[1] < 16);
        debug_assert!(subchunk_xyz[2] < 16);
        debug_assert!(light_levels[0] < 16);
        debug_assert!(light_levels[1] < 16);
        Self {
            uvs,
            tint_colour,
            packed_xyz: (subchunk_xyz[0] as u16)
                | ((subchunk_xyz[1] as u16) << 4)
                | ((subchunk_xyz[2] as u16) << 8),
            uv_rotation_and_light_levels: [
                match uv_rotation {
                    RightAngleRotation::Zero => 0,
                    RightAngleRotation::Ninety => 1,
                    RightAngleRotation::OneEighty => 2,
                    RightAngleRotation::TwoSeventy => 3,
                },
                light_levels[0] | (light_levels[1] << 4),
            ],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CustomBlockFace {
    vertices: [CustomBlockVertex; 4],
    tint_rgb: [u8; 3],
    /// Directional light coefficient, calculated from the face normal.
    dir_light_coef_unorm8: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct CustomBlockVertex {
    /// Subchunk-local position, multiplied by 1024 and rounded to an [`i16`].
    subchunk_fixed_point_pos: [i16; 3],
    uvs: [u16; 2],
    /// 0-3: Sky light level
    /// 4-7: Block light level
    light_levels: u8,
}

impl CustomBlockFace {
    pub fn new(
        model_face: &resources::block::model::CustomModelFace,
        subchunk_xyz: [u8; 3],
        tint_rgba: [u8; 4],
        centre_light_levels: [u8; 2],
        neighbour_light_levels: [[u8; 2]; 6],
    ) -> Self {
        let subchunk_xyz_vec3 = Vector3::from(subchunk_xyz.map(|n| n as f32));
        // The tint colour will be multiplied into the final colour, so just make it one if the
        // face isn't tinted.
        let applied_tint_rgb = match model_face.tint {
            None => [0xFF, 0xFF, 0xFF],
            Some(Tint::Biome) => [tint_rgba[0], tint_rgba[1], tint_rgba[2]],
        };
        // Calculate per-face directional lighting.
        let light_source_dir = Vector3::new(2.0, 5.0, 1.0).normalize();
        let dir_lighting = Vector3::dot(&model_face.normal, &light_source_dir);
        let dir_light_coef = f32::mul_add(dir_lighting, 0.3, 0.7);
        let dir_light_coef_unorm8 = (dir_light_coef.clamp(0.0, 1.0) * 255.0).round() as u8;
        // Convert face vertices.
        let normal_offset = model_face.normal * 0.02;
        let vertices = model_face.vertices.map(|v| {
            // Calculate the vertex subchunk fixed-point position.
            let subchunk_pos = v.local_pos + subchunk_xyz_vec3 + Vector3::repeat(0.5);
            let subchunk_fixed_point_pos: [i16; 3] =
                (subchunk_pos * 1024.0).map(|n| n.round() as i16).into();
            // Calculate block light levels.
            let packed_light_levels = {
                // Try to find the block light levels that are "most applicable" for the current
                // vertex. The light levels sampled are the levels for the block itself, followed
                // by all six neighbours.
                let adjusted_pos = v.local_pos + normal_offset;
                let light_levels = [
                    centre_light_levels,
                    neighbour_light_levels[0],
                    neighbour_light_levels[1],
                    neighbour_light_levels[2],
                    neighbour_light_levels[3],
                    neighbour_light_levels[4],
                    neighbour_light_levels[5],
                ];
                let light_positions = [
                    Point3::new(0.0, 0.0, 0.0),
                    Point3::new(0.0, 1.01, 0.0),
                    Point3::new(0.0, -1.01, 0.0),
                    Point3::new(0.0, 0.0, -1.01),
                    Point3::new(0.0, 0.0, 1.01),
                    Point3::new(0.0, 1.01, 0.0),
                    Point3::new(0.0, -1.01, 0.0),
                ];
                let (closest_light_i, _closest_light_dist) =
                    light_positions.iter().enumerate().fold(
                        (0, f32::INFINITY),
                        |(current_closest_i, current_closest_dist), (i, pos)| {
                            let dist = (pos - adjusted_pos).magnitude_squared();
                            if dist < current_closest_dist {
                                (i, dist)
                            } else {
                                (current_closest_i, current_closest_dist)
                            }
                        },
                    );
                let closest_light_levels = light_levels[closest_light_i];
                (closest_light_levels[0] & 0xF) | ((closest_light_levels[1] & 0xF) << 4)
            };
            CustomBlockVertex {
                subchunk_fixed_point_pos,
                uvs: v.uvs,
                light_levels: packed_light_levels,
            }
        });
        Self {
            vertices,
            tint_rgb: applied_tint_rgb,
            dir_light_coef_unorm8,
        }
    }
}

#[inline(always)]
fn get_face_rotation(face_i: usize) -> Rotation3<f32> {
    assert!(face_i < 6);
    match face_i {
        // Top
        0 => Rotation3::identity(),
        // Bottom
        1 => Rotation3::from_euler_angles(core::f32::consts::PI, 0.0, 0.0),
        // North
        2 => {
            Rotation3::from_euler_angles(-core::f32::consts::FRAC_PI_2, 0.0, core::f32::consts::PI)
        }
        // South
        3 => Rotation3::from_euler_angles(core::f32::consts::FRAC_PI_2, 0.0, 0.0),
        // East
        4 => Rotation3::from_euler_angles(
            0.0,
            core::f32::consts::FRAC_PI_2,
            -core::f32::consts::FRAC_PI_2,
        ),
        // West
        5 => Rotation3::from_euler_angles(
            0.0,
            -core::f32::consts::FRAC_PI_2,
            core::f32::consts::FRAC_PI_2,
        ),
        _ => unreachable!(),
    }
}

#[inline(always)]
fn get_uvs(vertex_i: u8, face_rotation_i: u8) -> Vector2<f32> {
    let uv_rotation_vec: [u32; 4] = match face_rotation_i {
        0 => [0, 1, 2, 3],
        1 => [1, 3, 0, 2],
        2 => [3, 2, 1, 0],
        _ => [2, 0, 3, 1],
    };
    let new_vertex_i = uv_rotation_vec[vertex_i as usize % 4];
    match new_vertex_i {
        0 => Vector2::new(0.0, 1.0),
        1 => Vector2::new(1.0, 1.0),
        2 => Vector2::new(0.0, 0.0),
        _ => Vector2::new(1.0, 0.0),
    }
}

#[inline(always)]
pub fn calculate_light_rgb(sky_light_level: u8, block_light_level: u8) -> Vector3<f32> {
    let light_percentage = f32::clamp(
        // FIXME: I think sky light and block light have different maxima?
        f32::max(sky_light_level as f32, block_light_level as f32) / 14.0,
        0.001,
        1.0,
    );
    let gamma = 0.5;
    let light_gamma = light_percentage.powf(1.0 / gamma);
    Vector3::repeat(0.02).lerp(&Vector3::repeat(1.0), light_gamma)
}

const BASE_NORMAL: Vector3<f32> = Vector3::new(0.0, 1.0, 0.0);

#[derive(Clone, Copy, Debug)]
struct BlockClipVertex {
    pub pos: Point3<f32>,
    pub w: f32,
    pub uv: Point2<f32>,
}

#[derive(Clone, Debug)]
pub struct TileDrawCommand {
    tri_info: Arc<TriInfo>,
    ty: TileDrawCommandType,
}

#[derive(Clone, Debug)]
struct TriInfo {
    tri: [BlockClipVertex; 3],
    tri_edges: [ClipEdge; 3],
    rgba: Rgba8Ne,
    alpha_test: bool,
}

#[derive(Clone, Debug)]
enum TileDrawCommandType {
    /// The entire tile should be rasterised, so no per-micro-tile, per-pixel-group, or
    /// per-pixel rasterisation checks are required.
    WholeTile,
    /// Only part of the tile should be rasterised, so sub-tile rasterisation checks are
    /// required.
    PartialTile,
}

// TODO: Switch to fixed point.

/// An edge that restricts rasterisation to the "inside" half of the edge.
///
/// For `egui` meshes, a triangle is rasterised using seven clip edges: the three triangle edges
/// that restrict rasterisation to the inside of the triangle, and the four clipping rectangle
/// edges that restrict rasterisation to the inside of the clipping rectangle.
#[derive(Clone, Copy)]
struct ClipEdge {
    /// The A, B, and C parameters in the equation for the half-plane, which is either
    /// `Ax + By < C`, or `Ax + By <= C` (if `is_half_plane_closed` is true).
    pub half_plane_params: [f32; 3],
    pub flags: ClipEdgeFlags,
}

impl core::fmt::Debug for ClipEdge {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ClipEdge")
            .field(
                "half_plane_equation",
                &format_args!(
                    "ClipEdge({}x + {}y {} {})",
                    self.half_plane_params[0],
                    self.half_plane_params[1],
                    if self.flags.is_half_plane_closed() {
                        "<="
                    } else {
                        "<"
                    },
                    self.half_plane_params[2],
                ),
            )
            .field(
                "trivial_reject_corner",
                &[
                    self.flags.trivial_reject_offset_x(),
                    self.flags.trivial_reject_offset_y(),
                ],
            )
            .field(
                "trivial_accept_corner",
                &[
                    self.flags.trivial_accept_offset_x(),
                    self.flags.trivial_accept_offset_y(),
                ],
            )
            .finish()
    }
}

bitfield! {
    #[derive(Clone, Copy)]
    struct ClipEdgeFlags(u8);
    impl Debug;
    /// X offset of trivial reject corner on unit square.
    pub trivial_reject_offset_x, set_trivial_reject_offset_x: 0, 0;
    /// Y offset of trivial reject corner on unit square.
    pub trivial_reject_offset_y, set_trivial_reject_offset_y: 1, 1;
    /// X offset of trivial accept corner on unit square.
    pub trivial_accept_offset_x, set_trivial_accept_offset_x: 2, 2;
    /// Y offset of trivial accept corner on unit square.
    pub trivial_accept_offset_y, set_trivial_accept_offset_y: 3, 3;
    /// Whether the half-plane determined by this edge is a closed half-plane, where
    /// closed half-planes include points exactly on the edge.
    ///
    /// Determines whether the comparison used in the equation for the half-plane (see
    /// `half_plane_params`) is inclusive.
    pub is_half_plane_closed, set_is_half_plane_closed: 4;
}

// Clip a triangle in global-space against the camera's near clip plane.
// Ideally we'd do this with simple Sutherland-Hodgman clipping, but it seems to need to happen
// before perspective projection.
#[inline(always)]
fn near_clip_tri(
    out_clipped_tris: &mut Vec<[BlockClipVertex; 3]>,
    camera_near_plane: &(Vector3<f32>, f32),
    tri: [BlockClipVertex; 3],
) {
    let (camera_near_plane_normal, camera_near_plane_offset) = camera_near_plane;
    let tri_dists =
        tri.map(|point| point.pos.coords.dot(camera_near_plane_normal) + camera_near_plane_offset);
    let points_in_bounds = tri_dists.map(|dist| dist >= 0.0);
    // Seems to produce slightly better code than the commented code below.
    let num_points_in_bounds = match points_in_bounds {
        [false, false, false] => 0,
        [false, false, true] | [false, true, false] | [true, false, false] => 1,
        [false, true, true] | [true, false, true] | [true, true, false] => 2,
        [true, true, true] => 3,
    };
    // let num_points_in_bounds = points_in_bounds.map(|b| b as u8).iter().sum();
    #[inline(always)]
    fn calc_intersection(
        v0: &BlockClipVertex,
        v0_dist: f32,
        v1: &BlockClipVertex,
        v1_dist: f32,
    ) -> BlockClipVertex {
        let factor = v0_dist / (v0_dist - v1_dist);
        BlockClipVertex {
            pos: v0.pos + (factor * (v1.pos - v0.pos)),
            w: v0.w + (factor * (v1.w - v0.w)),
            uv: v0.uv + (factor * (v1.uv - v0.uv)),
        }
    }
    match num_points_in_bounds {
        // Don't generate any triangles if all points are out of bounds.
        0 => {}
        // Generate a single clipped triangle if two points are out of bounds.
        1 => {
            if points_in_bounds[0] {
                let new_v1 = calc_intersection(&tri[0], tri_dists[0], &tri[1], tri_dists[1]);
                let new_v2 = calc_intersection(&tri[0], tri_dists[0], &tri[2], tri_dists[2]);
                out_clipped_tris.push([tri[0], new_v1, new_v2]);
            } else if points_in_bounds[1] {
                let new_v0 = calc_intersection(&tri[1], tri_dists[1], &tri[0], tri_dists[0]);
                let new_v2 = calc_intersection(&tri[1], tri_dists[1], &tri[2], tri_dists[2]);
                out_clipped_tris.push([new_v0, tri[1], new_v2]);
            } else {
                let new_v0 = calc_intersection(&tri[2], tri_dists[2], &tri[0], tri_dists[0]);
                let new_v1 = calc_intersection(&tri[2], tri_dists[2], &tri[1], tri_dists[1]);
                out_clipped_tris.push([new_v0, new_v1, tri[2]]);
            }
        }
        // Generate two clipped triangles if one point is out of bounds.
        2 => {
            if !points_in_bounds[0] {
                let new_01 = calc_intersection(&tri[1], tri_dists[1], &tri[0], tri_dists[0]);
                let new_02 = calc_intersection(&tri[2], tri_dists[2], &tri[0], tri_dists[0]);
                out_clipped_tris.push([new_01, tri[1], tri[2]]);
                out_clipped_tris.push([new_01, tri[2], new_02]);
            } else if !points_in_bounds[1] {
                let new_01 = calc_intersection(&tri[0], tri_dists[0], &tri[1], tri_dists[1]);
                let new_12 = calc_intersection(&tri[2], tri_dists[2], &tri[1], tri_dists[1]);
                out_clipped_tris.push([tri[0], new_12, tri[2]]);
                out_clipped_tris.push([new_12, tri[0], new_01]);
            } else {
                let new_02 = calc_intersection(&tri[0], tri_dists[0], &tri[2], tri_dists[2]);
                let new_12 = calc_intersection(&tri[1], tri_dists[1], &tri[2], tri_dists[2]);
                out_clipped_tris.push([tri[0], tri[1], new_02]);
                out_clipped_tris.push([new_02, tri[1], new_12]);
            }
        }
        // No clipping required if no points are out of bounds.
        3 => out_clipped_tris.push(tri),
        _ => unreachable!(),
    }
}

#[multiversion::multiversion(targets(
    "x86_64+sse4.2+bmi1+bmi2+fma+lzcnt+movbe+avx512f+avx512vl+avx512dq+avx512bw",
    "x86_64+sse4.2+bmi1+bmi2+fma+lzcnt+movbe+avx2",
    "x86/i686+sse2",
    "arm+neon",
))]
#[tracing::instrument(skip_all)]
pub fn bin_subchunk(
    out_bins: &Mutex<&mut RenderTileBins>,
    (width, height): (NonZeroU32, NonZeroU32),
    tiles_per_row: usize,
    reversed_view_matrix: &Matrix4<f32>,
    camera_near_plane: &(Vector3<f32>, f32),
    block_item_atlas: &TextureAtlas,
    subchunk: &Subchunk,
) {
    let reversed_screen_matrix = reversed_view_matrix
        .append_nonuniform_scaling(&Vector3::new(1.0, -1.0, 1.0))
        .append_translation(&Vector3::new(1.0, 1.0, 0.0))
        .append_nonuniform_scaling(&Vector3::new(
            width.get() as f32 * 0.5,
            height.get() as f32 * 0.5,
            1.0,
        ));
    let atlas_texture = block_item_atlas.get_texture();
    let face_base_positions = [
        Point3::new(-0.5, 0.5, 0.5),
        Point3::new(0.5, 0.5, 0.5),
        Point3::new(-0.5, 0.5, -0.5),
        Point3::new(0.5, 0.5, -0.5),
    ];
    let subchunk_start_coords = subchunk.start_coords.cast::<f32>();
    let mut draw_cmd_batch: Vec<(usize, TileDrawCommand)> = Vec::new();
    let mut current_block_face_start: usize = 0;
    let mut current_tinted_block_face_start: usize = 0;
    let mut working_tris: Vec<[BlockClipVertex; 3]> = Vec::with_capacity(32);
    let mut clipped_tris: Vec<[BlockClipVertex; 3]> = Vec::with_capacity(32);
    for face_i in 0..6 {
        let face_rotation = get_face_rotation(face_i);
        let face_local_positions =
            face_base_positions.map(|pos| face_rotation.transform_point(&pos));
        // Bin basic block faces.
        let num_block_faces = subchunk.block_face_group_lengths[face_i] as usize;
        let start_block_face = current_block_face_start;
        current_block_face_start += num_block_faces;
        if let Some(subchunk_block_faces) = subchunk.block_faces.as_ref() {
            'block_face: for block_face in
                &subchunk_block_faces[start_block_face..][..num_block_faces]
            {
                // Calculate global position of block face vertices.
                let xyz_offset = Vector3::new(
                    (block_face.packed_xyz & 0x00F) as f32,
                    ((block_face.packed_xyz & 0x0F0) >> 4) as f32,
                    ((block_face.packed_xyz & 0xF00) >> 8) as f32,
                );
                let block_centre_pos: Point3<f32> = subchunk_start_coords + xyz_offset;
                let global_positions: [Point3<f32>; 4] = face_local_positions
                    .map(|local_pos| block_centre_pos + local_pos.coords + Vector3::repeat(0.5));
                // Calculate per-vertex UVs.
                let start_uvs = Point2::new(
                    block_face.uvs[0] as f32 / atlas_texture.width.get() as f32,
                    block_face.uvs[1] as f32 / atlas_texture.height.get() as f32,
                );
                let end_uvs = Point2::new(
                    block_face.uvs[2] as f32 / atlas_texture.width.get() as f32,
                    block_face.uvs[3] as f32 / atlas_texture.height.get() as f32,
                );
                let face_rotation_i = block_face.uv_rotation_and_light_levels[0] & 0b11;
                let vertex_uvs = [0, 1, 2, 3].map(|vertex_i| {
                    Point2::from(
                        start_uvs
                            + (end_uvs - start_uvs)
                                .component_mul(&get_uvs(vertex_i, face_rotation_i)),
                    )
                });
                // Calculate the face RGB colour.
                // Includes block lighting, as well as some basic per-face directional lighting.
                let normal = face_rotation.transform_vector(&BASE_NORMAL);
                let light_source_dir = Vector3::new(2.0, 5.0, 1.0).normalize();
                let dir_lighting = Vector3::dot(&normal, &light_source_dir);
                let dir_light_coef = (dir_lighting * 0.3) + 0.7;
                let sky_light_level = block_face.uv_rotation_and_light_levels[1] & 0x0F;
                let block_light_level = (block_face.uv_rotation_and_light_levels[1] & 0xF0) >> 4;
                let light_rgb = calculate_light_rgb(sky_light_level, block_light_level);
                let face_rgb = light_rgb * dir_light_coef;
                let face_rgba8 = Rgba::new(face_rgb.x, face_rgb.y, face_rgb.z, 1.0).to_rgba8ne();
                // Assemble face vertices into two triangles.
                debug_assert!(working_tris.is_empty());
                debug_assert!(clipped_tris.is_empty());
                // Values for `w` are temporary, and will be overwritten during perspective
                // projection.
                working_tris.push([
                    BlockClipVertex {
                        pos: global_positions[0],
                        w: 1.0,
                        uv: vertex_uvs[0],
                    },
                    BlockClipVertex {
                        pos: global_positions[2],
                        w: 1.0,
                        uv: vertex_uvs[2],
                    },
                    BlockClipVertex {
                        pos: global_positions[1],
                        w: 1.0,
                        uv: vertex_uvs[1],
                    },
                ]);
                working_tris.push([
                    BlockClipVertex {
                        pos: global_positions[1],
                        w: 1.0,
                        uv: vertex_uvs[1],
                    },
                    BlockClipVertex {
                        pos: global_positions[2],
                        w: 1.0,
                        uv: vertex_uvs[2],
                    },
                    BlockClipVertex {
                        pos: global_positions[3],
                        w: 1.0,
                        uv: vertex_uvs[3],
                    },
                ]);
                // Clip new triangles against the camera near plane.
                for tri in working_tris.drain(..) {
                    near_clip_tri(&mut clipped_tris, camera_near_plane, tri);
                }
                // Transform triangles to screen space, with reversed Z.
                for tri in &mut clipped_tris {
                    for point in tri.iter_mut() {
                        let screen_vec4 = reversed_screen_matrix * point.pos.coords.push(1.0);
                        let inv_w = 1.0 / screen_vec4.w;
                        point.pos = Point3::from(screen_vec4.xyz() * inv_w);
                        point.w = inv_w;
                    }
                    // If any of the clipped triangles are back-facing, then the entire block face
                    // must be back-facing, so skip it.
                    let a = tri[1].pos - tri[0].pos;
                    let b = tri[2].pos - tri[0].pos;
                    let z = (a.x * b.y) - (a.y * b.x);
                    if z <= 0.0 {
                        clipped_tris.clear();
                        continue 'block_face;
                    }
                }
                for tri in clipped_tris.drain(..) {
                    bin_block_face_tri(
                        &mut draw_cmd_batch,
                        (width, height),
                        tiles_per_row,
                        tri,
                        face_rgba8,
                        false,
                    );
                }
            }
        }
        // Bin tinted block faces.
        let num_tinted_block_faces = subchunk.tinted_block_face_group_lengths[face_i] as usize;
        let start_tinted_block_face = current_tinted_block_face_start;
        current_tinted_block_face_start += num_tinted_block_faces;
        if let Some(subchunk_tinted_block_faces) = subchunk.tinted_block_faces.as_ref() {
            'tinted_block_face: for block_face in
                &subchunk_tinted_block_faces[start_tinted_block_face..][..num_tinted_block_faces]
            {
                // Calculate global position of block face vertices.
                let xyz_offset = Vector3::new(
                    (block_face.packed_xyz & 0x00F) as f32,
                    ((block_face.packed_xyz & 0x0F0) >> 4) as f32,
                    ((block_face.packed_xyz & 0xF00) >> 8) as f32,
                );
                let block_centre_pos: Point3<f32> = subchunk_start_coords + xyz_offset;
                let global_positions: [Point3<f32>; 4] = face_local_positions
                    .map(|local_pos| block_centre_pos + local_pos.coords + Vector3::repeat(0.5));
                // Calculate per-vertex UVs.
                let start_uvs = Point2::new(
                    block_face.uvs[0] as f32 / atlas_texture.width.get() as f32,
                    block_face.uvs[1] as f32 / atlas_texture.height.get() as f32,
                );
                let end_uvs = Point2::new(
                    block_face.uvs[2] as f32 / atlas_texture.width.get() as f32,
                    block_face.uvs[3] as f32 / atlas_texture.height.get() as f32,
                );
                let face_rotation_i = block_face.uv_rotation_and_light_levels[0] & 0b11;
                let vertex_uvs = [0, 1, 2, 3].map(|vertex_i| {
                    Point2::from(
                        start_uvs
                            + (end_uvs - start_uvs)
                                .component_mul(&get_uvs(vertex_i, face_rotation_i)),
                    )
                });
                // Calculate the face RGB colour.
                // Includes block lighting, tinting, as well as some basic per-face directional
                // shading.
                let tint_colour = Rgba::from_rgba8(block_face.tint_colour);
                let normal = face_rotation.transform_vector(&BASE_NORMAL);
                let light_source_dir = Vector3::new(2.0, 5.0, 1.0).normalize();
                let dir_lighting = Vector3::dot(&normal, &light_source_dir);
                let dir_light_coef = (dir_lighting * 0.3) + 0.7;
                let sky_light_level = block_face.uv_rotation_and_light_levels[1] & 0x0F;
                let block_light_level = (block_face.uv_rotation_and_light_levels[1] & 0xF0) >> 4;
                let light_rgb = calculate_light_rgb(sky_light_level, block_light_level);
                let face_rgb = light_rgb * dir_light_coef;
                let face_rgba8 =
                    (Rgba::new(face_rgb.x, face_rgb.y, face_rgb.z, 1.0) * tint_colour).to_rgba8ne();
                // Assemble face vertices into two triangles.
                debug_assert!(working_tris.is_empty());
                debug_assert!(clipped_tris.is_empty());
                // Values for `w` are temporary, and will be overwritten during perspective
                // projection.
                working_tris.push([
                    BlockClipVertex {
                        pos: global_positions[0],
                        w: 1.0,
                        uv: vertex_uvs[0],
                    },
                    BlockClipVertex {
                        pos: global_positions[2],
                        w: 1.0,
                        uv: vertex_uvs[2],
                    },
                    BlockClipVertex {
                        pos: global_positions[1],
                        w: 1.0,
                        uv: vertex_uvs[1],
                    },
                ]);
                working_tris.push([
                    BlockClipVertex {
                        pos: global_positions[1],
                        w: 1.0,
                        uv: vertex_uvs[1],
                    },
                    BlockClipVertex {
                        pos: global_positions[2],
                        w: 1.0,
                        uv: vertex_uvs[2],
                    },
                    BlockClipVertex {
                        pos: global_positions[3],
                        w: 1.0,
                        uv: vertex_uvs[3],
                    },
                ]);
                // Clip new triangles against the camera near plane.
                for tri in working_tris.drain(..) {
                    near_clip_tri(&mut clipped_tris, camera_near_plane, tri);
                }
                // Transform triangles to screen space, with reversed Z.
                for tri in &mut clipped_tris {
                    for point in tri.iter_mut() {
                        let screen_vec4 = reversed_screen_matrix * point.pos.coords.push(1.0);
                        let inv_w = 1.0 / screen_vec4.w;
                        point.pos = Point3::from(screen_vec4.xyz() * inv_w);
                        point.w = inv_w;
                    }
                    // If any of the clipped triangles are back-facing, then the entire block face
                    // must be back-facing, so skip it.
                    let a = tri[1].pos - tri[0].pos;
                    let b = tri[2].pos - tri[0].pos;
                    let z = (a.x * b.y) - (a.y * b.x);
                    if z <= 0.0 {
                        clipped_tris.clear();
                        continue 'tinted_block_face;
                    }
                }
                for tri in clipped_tris.drain(..) {
                    bin_block_face_tri(
                        &mut draw_cmd_batch,
                        (width, height),
                        tiles_per_row,
                        tri,
                        face_rgba8,
                        true,
                    );
                }
            }
        }
    }
    // Bin custom block faces.
    if let Some(subchunk_custom_block_faces) = subchunk.custom_block_faces.as_ref() {
        'custom_block_face: for block_face in subchunk_custom_block_faces {
            // Calculate global position of block face vertices.
            let global_positions: [Point3<f32>; 4] = block_face.vertices.map(|v| {
                subchunk_start_coords
                    + Vector3::from(v.subchunk_fixed_point_pos.map(|n| n as f32 / 1024.0))
            });
            // Calculate per-vertex UVs.
            let vertex_uvs: [Point2<f32>; 4] = block_face.vertices.map(|v| {
                Point2::new(
                    v.uvs[0] as f32 / atlas_texture.width.get() as f32,
                    v.uvs[1] as f32 / atlas_texture.height.get() as f32,
                )
            });
            // Calculate the face RGB colour.
            // Includes block lighting, tinting, as well as some basic per-face directional
            // shading.
            let tint_colour = Rgba::from_rgb8(block_face.tint_rgb);
            let dir_light_coef = block_face.dir_light_coef_unorm8 as f32 / 255.0;
            let light_rgb = {
                // Calculate light RGBs for each vertex.
                let light_rgbs = block_face.vertices.map(|v| {
                    let sky_light_level = v.light_levels & 0x0F;
                    let block_light_level = (v.light_levels & 0xF0) >> 4;
                    calculate_light_rgb(sky_light_level, block_light_level)
                });
                // Average to a single face RGB.
                light_rgbs.iter().copied().reduce(|acc, e| acc + e).unwrap() / 4.0
            };
            let face_rgb = light_rgb * dir_light_coef;
            let face_rgba8 =
                (Rgba::new(face_rgb.x, face_rgb.y, face_rgb.z, 1.0) * tint_colour).to_rgba8ne();
            // Assemble face vertices into two triangles.
            debug_assert!(working_tris.is_empty());
            debug_assert!(clipped_tris.is_empty());
            // Values for `w` are temporary, and will be overwritten during perspective
            // projection.
            working_tris.push([
                BlockClipVertex {
                    pos: global_positions[0],
                    w: 1.0,
                    uv: vertex_uvs[0],
                },
                BlockClipVertex {
                    pos: global_positions[1],
                    w: 1.0,
                    uv: vertex_uvs[1],
                },
                BlockClipVertex {
                    pos: global_positions[2],
                    w: 1.0,
                    uv: vertex_uvs[2],
                },
            ]);
            working_tris.push([
                BlockClipVertex {
                    pos: global_positions[2],
                    w: 1.0,
                    uv: vertex_uvs[2],
                },
                BlockClipVertex {
                    pos: global_positions[1],
                    w: 1.0,
                    uv: vertex_uvs[1],
                },
                BlockClipVertex {
                    pos: global_positions[3],
                    w: 1.0,
                    uv: vertex_uvs[3],
                },
            ]);
            // Clip new triangles against the camera near plane.
            for tri in working_tris.drain(..) {
                near_clip_tri(&mut clipped_tris, camera_near_plane, tri);
            }
            // Transform triangles to screen space, with reversed Z.
            for tri in &mut clipped_tris {
                for point in tri.iter_mut() {
                    let screen_vec4 = reversed_screen_matrix * point.pos.coords.push(1.0);
                    let inv_w = 1.0 / screen_vec4.w;
                    point.pos = Point3::from(screen_vec4.xyz() * inv_w);
                    point.w = inv_w;
                }
                // If any of the clipped triangles are back-facing, then the entire block face
                // must be back-facing, so skip it.
                let a = tri[1].pos - tri[0].pos;
                let b = tri[2].pos - tri[0].pos;
                let z = (a.x * b.y) - (a.y * b.x);
                if z <= 0.0 {
                    clipped_tris.clear();
                    continue 'custom_block_face;
                }
            }
            for tri in clipped_tris.drain(..) {
                bin_block_face_tri(
                    &mut draw_cmd_batch,
                    (width, height),
                    tiles_per_row,
                    tri,
                    face_rgba8,
                    true,
                );
            }
        }
    }
    // Add batched draw commands to bins.
    {
        let span = tracing::trace_span!("add_batched_draw_cmds");
        let _enter = span.enter();
        let mut locked_bins = out_bins.lock().unwrap();
        for (bin_idx, draw_cmd) in draw_cmd_batch {
            locked_bins.bins[bin_idx].chunk_draw_cmds.push(draw_cmd);
        }
    }
}

#[inline(always)]
fn bin_block_face_tri(
    out_cmd_batch: &mut Vec<(usize, TileDrawCommand)>,
    (width, height): (NonZeroU32, NonZeroU32),
    tiles_per_row: usize,
    tri: [BlockClipVertex; 3],
    tri_rgba8: Rgba8Ne,
    alpha_test: bool,
) {
    let tri_edge_points = [
        (tri[0].pos, tri[1].pos),
        (tri[1].pos, tri[2].pos),
        (tri[2].pos, tri[0].pos),
    ];
    let tri_clip_edges: [ClipEdge; 3] = tri_edge_points.map(|(p1, p2)| {
        // Calculate the base X and Y coordinates of the trivial reject corner.
        // This is the corner of a unit square most likely to reside within the inside
        // half-plane of the edge.
        // Our screen coordinate system has +X going right, and +Y going down, so if we
        // have the top-left corner of a 64x64 pixel tile starting at some `tile_xy`, then
        // the trivial reject corner of the tile would be at
        // `tile_xy + 0.5 + (63.0 * trivial_reject_offset_base_xy)`.
        // The 0.5 offset is due to the actual bounding box that we want to use for a tile
        // being the bounding box of the pixels within a tile, and we currently use a
        // sample position inside each pixel of (0.5, 0.5), although MSAA support would
        // mean multiple sample positions per pixel (random(?), although definitely the
        // same for each pixel in a frame).
        // If the trivial reject corner resides in the closed half-plane outside the
        // triangle edge, then we know the entire tile is definitely outside the edge.
        // This then means that we can use this as a test, where if any of the tile's
        // trivial reject corners lies outside the corner's corresponding edge, then the
        // entire tile is definitely outside the triangle, and so we can skip it.
        let trivial_reject_offset_x: u8 = match p1.y > p2.y {
            false => 0,
            true => 1,
        };
        let trivial_reject_offset_y: u8 = match p1.x > p2.x {
            false => 1,
            true => 0,
        };
        // Calculate the base X and Y coordinates of the trivial accept corner.
        // This is the corner of a unit square most likely to reside within the outside
        // half-plane of the edge, and is opposite to the trivial reject corner.
        // The calculations for finding the corner of a 64x64 pixel tile are practically
        // identical (`tile_xy + 0.5 + (63.0 * trivial_accept_offset_base_xy)`).
        // If the trivial accept corner resides in the half-plane inside the triangle edge,
        // then we know that the entire tile is definitely inside the edge.
        // This then means that if all of the tile's trivial accept corners lie inside the
        // corner's corresponding edge, then the entire triangle is definitely inside the
        // triangle, and we can just rasterise the entire tile.
        let trivial_accept_offset_x: u8 = match p1.y > p2.y {
            false => 1,
            true => 0,
        };
        let trivial_accept_offset_y: u8 = match p1.x > p2.x {
            false => 0,
            true => 1,
        };
        // Calcluate the half-plane equation parameters `A`, `B`, and `c`.
        let param_a = p2.y - p1.y;
        let param_b = p1.x - p2.x;
        let param_c = (param_a * p1.x) + (param_b * p1.y);
        let half_plane_params = [param_a, param_b, param_c];
        // Determine whether the edge should be inclusive of points exactly on the edge.
        // If we were to imagine that we were rasterising an octagon (for demonstration
        // purposes):
        //
        // /---\
        // |   |
        // \___/
        //
        // The sides we want to rasterise using an inclusive bounds test would be:
        //
        // /---
        // |
        // \
        //
        // These are the "top", "top-left", "left", and "bottom-left" edges.
        let is_half_plane_closed = (p1.x >= p2.x && p1.y <= p2.y) || (p1.x < p2.x && p1.y < p2.y);
        ClipEdge {
            half_plane_params,
            flags: {
                let mut flags = ClipEdgeFlags(0);
                flags.set_trivial_reject_offset_x(trivial_reject_offset_x);
                flags.set_trivial_reject_offset_y(trivial_reject_offset_y);
                flags.set_trivial_accept_offset_x(trivial_accept_offset_x);
                flags.set_trivial_accept_offset_y(trivial_accept_offset_y);
                flags.set_is_half_plane_closed(is_half_plane_closed);
                flags
            },
        }
    });
    // Package up triangle info for use in tile bin draw commands.
    let tri_info = Arc::new(TriInfo {
        tri,
        tri_edges: tri_clip_edges,
        rgba: tri_rgba8,
        alpha_test,
    });
    // Calculate triangle bounding box.
    let mut bb_min_x = f32::INFINITY;
    let mut bb_min_y = f32::INFINITY;
    let mut bb_max_x = f32::NEG_INFINITY;
    let mut bb_max_y = f32::NEG_INFINITY;
    for v in &tri {
        bb_min_x = f32::min(bb_min_x, v.pos.x);
        bb_min_y = f32::min(bb_min_y, v.pos.y);
        bb_max_x = f32::max(bb_max_x, v.pos.x);
        bb_max_y = f32::max(bb_max_y, v.pos.y);
    }
    // Constrain bounding box to screen dimensions.
    bb_min_x = f32::max(bb_min_x, 0.0);
    bb_min_y = f32::max(bb_min_y, 0.0);
    bb_max_x = f32::min(bb_max_x, (width.get() - 1) as f32);
    bb_max_y = f32::min(bb_max_y, (height.get() - 1) as f32);
    // Convert bounding box from pixel coordinates to integer tile coordinates.
    let inv_render_tile_pixel_dim = 1.0 / RENDER_TILE_PIXEL_DIM as f32;
    let tile_bb_min_x = (bb_min_x * inv_render_tile_pixel_dim).floor() as usize;
    let tile_bb_min_y = (bb_min_y * inv_render_tile_pixel_dim).floor() as usize;
    let tile_bb_max_x = (bb_max_x * inv_render_tile_pixel_dim).floor() as usize;
    let tile_bb_max_y = (bb_max_y * inv_render_tile_pixel_dim).floor() as usize;
    for tile_y in tile_bb_min_y..=tile_bb_max_y {
        let tile_start_y = (tile_y * RENDER_TILE_PIXEL_DIM) as f32;
        'tile_loop: for tile_x in tile_bb_min_x..=tile_bb_max_x {
            let tile_start_x = (tile_x * RENDER_TILE_PIXEL_DIM) as f32;
            // Whether the tile passes every edge's trivial accept test, meaning that the
            // entire tile can be rasterised without testing inner micro-tiles.
            let mut whole_tile_accepted = true;
            for edge in &tri_clip_edges {
                // Calculate trivial reject and accept corner positions for this
                // (tile, edge) combination.
                let tile_trivial_reject_x = tile_start_x
                    + (RENDER_TILE_PIXEL_DIM * edge.flags.trivial_reject_offset_x() as usize)
                        as f32;
                let tile_trivial_reject_y = tile_start_y
                    + (RENDER_TILE_PIXEL_DIM * edge.flags.trivial_reject_offset_y() as usize)
                        as f32;
                let tile_trivial_accept_x = tile_start_x
                    + (RENDER_TILE_PIXEL_DIM * edge.flags.trivial_accept_offset_x() as usize)
                        as f32;
                let tile_trivial_accept_y = tile_start_y
                    + (RENDER_TILE_PIXEL_DIM * edge.flags.trivial_accept_offset_y() as usize)
                        as f32;
                // Test the trivial reject corner.
                // If this corner lies outside of the edge's half-plane, then we can be
                // certain that the entire tile lies outside the triangle, and can be
                // skipped.
                let [a, b, c] = edge.half_plane_params;
                let reject_test_val = (a * tile_trivial_reject_x) + (b * tile_trivial_reject_y);
                let should_reject_tile = if edge.flags.is_half_plane_closed() {
                    reject_test_val > c
                } else {
                    reject_test_val >= c
                };
                if should_reject_tile {
                    continue 'tile_loop;
                }
                // Test the trivial accept corner.
                let accept_test_val = (a * tile_trivial_accept_x) + (b * tile_trivial_accept_y);
                let whole_tile_still_accepted = if edge.flags.is_half_plane_closed() {
                    accept_test_val <= c
                } else {
                    accept_test_val < c
                };
                whole_tile_accepted &= whole_tile_still_accepted;
            }
            // Add draw command to batch.
            let bin_idx = (tile_y * tiles_per_row) + tile_x;
            out_cmd_batch.push((
                bin_idx,
                TileDrawCommand {
                    tri_info: tri_info.clone(),
                    ty: if whole_tile_accepted {
                        TileDrawCommandType::WholeTile
                    } else {
                        TileDrawCommandType::PartialTile
                    },
                },
            ));
        }
    }
}

#[multiversion::multiversion(targets(
    "x86_64+sse4.2+bmi1+bmi2+fma+lzcnt+movbe+avx512f+avx512vl+avx512dq+avx512bw",
    "x86_64+sse4.2+bmi1+bmi2+fma+lzcnt+movbe+avx2",
    "x86/i686+sse2",
    "arm+neon",
))]
#[tracing::instrument(skip_all)]
pub fn render_tile(
    out_tile: &mut RenderTileRgba,
    out_tile_hi_z: &mut RenderTileHiZChain,
    (tile_x, tile_y): (usize, usize),
    block_item_atlas: &TextureAtlas,
    draw_cmds: &mut Vec<TileDrawCommand>,
) {
    // Sort draw commands from closest to farthest, to improve Hi-Z effectiveness.
    // We use a stable sort, so that overlayed block faces stay in order.
    draw_cmds.sort_by(|a, b| {
        let a_sum_z = a.tri_info.tri.iter().map(|v| v.pos.z).sum();
        let b_sum_z = b.tri_info.tri.iter().map(|v| v.pos.z).sum();
        // We use reversed Z, so we want to sort in descending order.
        f32::total_cmp(&a_sum_z, &b_sum_z).reverse()
    });
    let atlas_texture = &block_item_atlas.texture;
    let tile_start_x = tile_x * RENDER_TILE_PIXEL_DIM;
    let tile_start_y = tile_y * RENDER_TILE_PIXEL_DIM;
    for draw_cmd in draw_cmds {
        let tri_info = &draw_cmd.tri_info;
        let tri = &tri_info.tri;
        match draw_cmd.ty {
            TileDrawCommandType::WholeTile => {
                // If all of the triangle's points are behind the tile's current minimum Z, then
                // it's fully occluded, and we can skip it.
                if tri.iter().all(|v| v.pos.z < out_tile_hi_z.tile) {
                    continue;
                }
                for micro_tile_i in 0..RENDER_TILE_DIM.pow(2) {
                    let micro_tile_y = micro_tile_i / RENDER_TILE_DIM;
                    let micro_tile_x = micro_tile_i % RENDER_TILE_DIM;
                    let micro_tile: &mut RenderMicroTileRgba =
                        &mut out_tile[micro_tile_y][micro_tile_x];
                    let micro_tile_depth: &mut RenderMicroTileDepth =
                        &mut out_tile_hi_z.pixel[micro_tile_i];
                    let micro_tile_start_x =
                        tile_start_x + (micro_tile_x * RENDER_MICRO_TILE_PIXEL_DIM);
                    let micro_tile_start_y =
                        tile_start_y + (micro_tile_y * RENDER_MICRO_TILE_PIXEL_DIM);
                    #[allow(clippy::needless_range_loop)]
                    for pixel_group_i in 0..RENDER_MICRO_TILE_DIM.pow(2) {
                        let pixel_group_y = pixel_group_i / RENDER_MICRO_TILE_DIM;
                        let pixel_group_x = pixel_group_i % RENDER_MICRO_TILE_DIM;
                        let pixel_group: &mut RenderPixelGroupRgba =
                            &mut micro_tile[pixel_group_y][pixel_group_x];
                        let pixel_group_depth: &mut RenderPixelGroupDepth =
                            &mut micro_tile_depth[pixel_group_i];
                        let pixel_group_start_x =
                            micro_tile_start_x + (pixel_group_x * RENDER_PIXEL_GROUP_DIM);
                        let pixel_group_start_y =
                            micro_tile_start_y + (pixel_group_y * RENDER_PIXEL_GROUP_DIM);
                        // Collect individual pixel X and Y coords into vectors, so we can
                        // process up to 16 pixels in parallel (i.e. if the hardware
                        // supports 512-bit SIMD, then an entire pixel group can be
                        // processed).
                        #[rustfmt::skip]
                        let pixel_x_offsets = f32x16::from_array([
                            0., 1., 2., 3.,
                            0., 1., 2., 3.,
                            0., 1., 2., 3.,
                            0., 1., 2., 3.,
                        ]);
                        #[rustfmt::skip]
                        let pixel_y_offsets = f32x16::from_array([
                            0., 0., 0., 0.,
                            1., 1., 1., 1.,
                            2., 2., 2., 2.,
                            3., 3., 3., 3.,
                        ]);
                        let sample_offsets = f32x16::splat(0.5);
                        let pixel_xs: f32x16 = pixel_x_offsets
                            + f32x16::splat(pixel_group_start_x as f32)
                            + sample_offsets;
                        let pixel_ys: f32x16 = pixel_y_offsets
                            + f32x16::splat(pixel_group_start_y as f32)
                            + sample_offsets;
                        // Calculate screen-space barycentrics per pixel.
                        let (screen_bary_us, screen_bary_vs, screen_bary_ws) = {
                            // Calculate screen-space barycentrics based on this implementation:
                            // <https://gamedev.stackexchange.com/a/63203>
                            let v0: Vector3<f32> = tri[1].pos - tri[0].pos;
                            let v1: Vector3<f32> = tri[2].pos - tri[0].pos;
                            let v2_xs: f32x16 = pixel_xs - f32x16::splat(tri[0].pos.x);
                            let v2_ys: f32x16 = pixel_ys - f32x16::splat(tri[0].pos.y);
                            let denom_recip: f32 = 1.0 / ((v0.x * v1.y) - (v1.x * v0.y));
                            let ss_vs: f32x16 = ((v2_xs * f32x16::splat(v1.y))
                                - (v2_ys * f32x16::splat(v1.x)))
                                * f32x16::splat(denom_recip);
                            let ss_ws: f32x16 = ((v2_ys * f32x16::splat(v0.x))
                                - (v2_xs * f32x16::splat(v0.y)))
                                * f32x16::splat(denom_recip);
                            let ss_us: f32x16 = f32x16::splat(1.0) - ss_vs - ss_ws;
                            (ss_us, ss_vs, ss_ws)
                        };
                        // Calculate perspective-correct barycentrics per pixel.
                        let (bary_us, bary_vs, bary_ws) = {
                            let unnorm_us = screen_bary_us * f32x16::splat(tri[0].w);
                            let unnorm_vs = screen_bary_vs * f32x16::splat(tri[1].w);
                            let unnorm_ws = screen_bary_ws * f32x16::splat(tri[2].w);
                            let recip_sum =
                                f32x16::splat(1.0) / (unnorm_us + unnorm_vs + unnorm_ws);
                            (
                                unnorm_us * recip_sum,
                                unnorm_vs * recip_sum,
                                unnorm_ws * recip_sum,
                            )
                        };
                        // Interpolate UVs.
                        let pixel_us = (f32x16::splat(tri[0].uv.x) * bary_us)
                            + (f32x16::splat(tri[1].uv.x) * bary_vs)
                            + (f32x16::splat(tri[2].uv.x) * bary_ws);
                        let pixel_vs = (f32x16::splat(tri[0].uv.y) * bary_us)
                            + (f32x16::splat(tri[1].uv.y) * bary_vs)
                            + (f32x16::splat(tri[2].uv.y) * bary_ws);
                        // Interpolate depths.
                        let pixel_depths = (f32x16::splat(tri[0].pos.z) * screen_bary_us)
                            + (f32x16::splat(tri[1].pos.z) * screen_bary_vs)
                            + (f32x16::splat(tri[2].pos.z) * screen_bary_ws);
                        // Depth-test pixels.
                        let old_pixel_depths = f32x16::from_array(*pixel_group_depth);
                        let mut visible_pixels = pixel_depths.simd_ge(old_pixel_depths);
                        // Convert tri RGBA to Unorm8s.
                        let [
                            pixel_tri_reds,
                            pixel_tri_greens,
                            pixel_tri_blues,
                            pixel_tri_alphas,
                        ] = rgba_u32x16_to_unorm8x16(u32x16::splat(tri_info.rgba));
                        // Sample atlas texture for each pixel.
                        let [
                            pixel_tex_reds,
                            pixel_tex_greens,
                            pixel_tex_blues,
                            pixel_tex_alphas,
                        ] = atlas_texture.sample_nearest_simd16(pixel_us, pixel_vs);
                        // Alpha test pixels.
                        if tri_info.alpha_test {
                            visible_pixels &= pixel_tex_alphas.0.simd_gt(u8x16::splat(0x00)).cast();
                        }
                        // Calculate final pixel colours.
                        let new_pixel_reds = pixel_tri_reds * pixel_tex_reds;
                        let new_pixel_greens = pixel_tri_greens * pixel_tex_greens;
                        let new_pixel_blues = pixel_tri_blues * pixel_tex_blues;
                        let new_pixel_alphas = pixel_tri_alphas * pixel_tex_alphas;
                        // Convert final pixel colours to RGBA8, and write to tile.
                        let new_pixel_rgbas = rgba_4xunorm8x16_to_u32x16([
                            new_pixel_reds,
                            new_pixel_greens,
                            new_pixel_blues,
                            new_pixel_alphas,
                        ]);
                        new_pixel_rgbas.store_select(pixel_group.as_mut_array(), visible_pixels);
                        // Update pixel depths.
                        pixel_depths.store_select(pixel_group_depth, visible_pixels);
                        // Update pixel group occlusion culling info.
                        out_tile_hi_z.pixel_group[micro_tile_i][pixel_group_i] =
                            f32x16::from_array(*pixel_group_depth).reduce_min();
                    }
                    // Update micro-tile occlusion culling info.
                    out_tile_hi_z.micro_tile[micro_tile_i] =
                        f32x16::from_array(out_tile_hi_z.pixel_group[micro_tile_i]).reduce_min();
                }
                // Update tile occlusion culling info.
                out_tile_hi_z.tile = f32x16::from_array(out_tile_hi_z.micro_tile).reduce_min();
            }
            TileDrawCommandType::PartialTile => {
                // If all of the triangle's points are behind the tile's current minimum Z, then
                // it's fully occluded, and we can skip it.
                if tri.iter().all(|v| v.pos.z < out_tile_hi_z.tile) {
                    continue;
                }
                let tri_edges = tri_info.tri_edges;
                #[rustfmt::skip]
                let x_offsets = u32x16::from_array([
                    0, 1, 2, 3,
                    0, 1, 2, 3,
                    0, 1, 2, 3,
                    0, 1, 2, 3,
                ]);
                #[rustfmt::skip]
                let y_offsets = u32x16::from_array([
                    0, 0, 0, 0,
                    1, 1, 1, 1,
                    2, 2, 2, 2,
                    3, 3, 3, 3,
                ]);
                let micro_tile_start_xs = u32x16::splat(tile_start_x as u32)
                    + (x_offsets * u32x16::splat(RENDER_MICRO_TILE_PIXEL_DIM as u32));
                let micro_tile_start_ys = u32x16::splat(tile_start_y as u32)
                    + (y_offsets * u32x16::splat(RENDER_MICRO_TILE_PIXEL_DIM as u32));
                // Find micro-tiles partially or fully inside every clip edge.
                // TODO: Might be worth doing trivial accept checks at each level as well.
                let mut visible_micro_tiles = mask32x16::splat(true);
                for edge in &tri_edges {
                    // Calculate trivial reject corner positions for this (micro-tile, edge)
                    // combination.
                    let micro_tile_trivial_reject_xs = micro_tile_start_xs
                        + u32x16::splat(
                            RENDER_MICRO_TILE_PIXEL_DIM as u32
                                * edge.flags.trivial_reject_offset_x() as u32,
                        );
                    let micro_tile_trivial_reject_ys = micro_tile_start_ys
                        + u32x16::splat(
                            RENDER_MICRO_TILE_PIXEL_DIM as u32
                                * edge.flags.trivial_reject_offset_y() as u32,
                        );
                    // Test the trivial reject corner.
                    // If this corner lies outside of the edge's half-plane, then we can be certain
                    // that the entire micro-tile lies outside the triangle, and can be skipped.
                    let [a, b, c] = edge.half_plane_params;
                    let reject_test_vals = (f32x16::splat(a)
                        * micro_tile_trivial_reject_xs.cast::<f32>())
                        + (f32x16::splat(b) * micro_tile_trivial_reject_ys.cast::<f32>());
                    let edge_visible_micro_tiles = if edge.flags.is_half_plane_closed() {
                        reject_test_vals.simd_le(f32x16::splat(c))
                    } else {
                        reject_test_vals.simd_lt(f32x16::splat(c))
                    };
                    visible_micro_tiles &= edge_visible_micro_tiles;
                }
                // Iterate through visible micro-tiles.
                while let Some(micro_tile_i) = visible_micro_tiles.first_set() {
                    visible_micro_tiles.set(micro_tile_i, false);
                    let micro_tile_y = micro_tile_i / RENDER_MICRO_TILE_DIM;
                    let micro_tile_x = micro_tile_i % RENDER_MICRO_TILE_DIM;
                    let micro_tile: &mut RenderMicroTileRgba =
                        &mut out_tile[micro_tile_y][micro_tile_x];
                    let micro_tile_depth: &mut RenderMicroTileDepth =
                        &mut out_tile_hi_z.pixel[micro_tile_i];
                    let micro_tile_start_x = micro_tile_start_xs[micro_tile_i];
                    let micro_tile_start_y = micro_tile_start_ys[micro_tile_i];
                    let pixel_group_start_xs = u32x16::splat(micro_tile_start_x)
                        + (x_offsets * u32x16::splat(RENDER_PIXEL_GROUP_DIM as u32));
                    let pixel_group_start_ys = u32x16::splat(micro_tile_start_y)
                        + (y_offsets * u32x16::splat(RENDER_PIXEL_GROUP_DIM as u32));
                    // Find pixel groups partially or fully inside every clip edge.
                    let mut visible_pixel_groups = mask32x16::splat(true);
                    for edge in &tri_edges {
                        // Calculate trivial reject corner positions for this (pixel group, edge)
                        // combination.
                        let pixel_group_trivial_reject_xs = pixel_group_start_xs
                            + u32x16::splat(
                                RENDER_PIXEL_GROUP_DIM as u32
                                    * edge.flags.trivial_reject_offset_x() as u32,
                            );
                        let pixel_group_trivial_reject_ys = pixel_group_start_ys
                            + u32x16::splat(
                                RENDER_PIXEL_GROUP_DIM as u32
                                    * edge.flags.trivial_reject_offset_y() as u32,
                            );
                        // Test the trivial reject corner.
                        // If this corner lies outside of the edge's half-plane, then we can be
                        // certain that the entire pixel group lies outside the triangle, and can
                        // be skipped.
                        let [a, b, c] = edge.half_plane_params;
                        let reject_test_vals = (f32x16::splat(a)
                            * pixel_group_trivial_reject_xs.cast::<f32>())
                            + (f32x16::splat(b) * pixel_group_trivial_reject_ys.cast::<f32>());
                        let edge_visible_pixel_groups = if edge.flags.is_half_plane_closed() {
                            reject_test_vals.simd_le(f32x16::splat(c))
                        } else {
                            reject_test_vals.simd_lt(f32x16::splat(c))
                        };
                        visible_pixel_groups &= edge_visible_pixel_groups;
                    }
                    // Iterate through visible pixel groups.
                    while let Some(pixel_group_i) = visible_pixel_groups.first_set() {
                        visible_pixel_groups.set(pixel_group_i, false);
                        let pixel_group_y = pixel_group_i / RENDER_PIXEL_GROUP_DIM;
                        let pixel_group_x = pixel_group_i % RENDER_PIXEL_GROUP_DIM;
                        let pixel_group: &mut RenderPixelGroupRgba =
                            &mut micro_tile[pixel_group_y][pixel_group_x];
                        let pixel_group_depth: &mut RenderPixelGroupDepth =
                            &mut micro_tile_depth[pixel_group_i];
                        let pixel_group_start_x = pixel_group_start_xs[pixel_group_i];
                        let pixel_group_start_y = pixel_group_start_ys[pixel_group_i];
                        let pixel_xs = (u32x16::splat(pixel_group_start_x) + x_offsets)
                            .cast::<f32>()
                            + f32x16::splat(0.5);
                        let pixel_ys = (u32x16::splat(pixel_group_start_y) + y_offsets)
                            .cast::<f32>()
                            + f32x16::splat(0.5);
                        // Find pixels inside all clip edges.
                        let mut visible_pixels = mask32x16::splat(true);
                        for edge in &tri_edges {
                            // Test the trivial reject corner.
                            // If this corner lies outside of the edge's half-plane, then we can be
                            // certain that the entire pixel lies outside the triangle, and can be
                            // skipped.
                            let [a, b, c] = edge.half_plane_params;
                            let reject_test_vals =
                                (f32x16::splat(a) * pixel_xs) + (f32x16::splat(b) * pixel_ys);
                            let edge_visible_pixels = if edge.flags.is_half_plane_closed() {
                                reject_test_vals.simd_le(f32x16::splat(c))
                            } else {
                                reject_test_vals.simd_lt(f32x16::splat(c))
                            };
                            visible_pixels &= edge_visible_pixels;
                        }
                        // Calculate screen-space barycentrics per pixel.
                        let (screen_bary_us, screen_bary_vs, screen_bary_ws) = {
                            // Calculate screen-space barycentrics based on this implementation:
                            // <https://gamedev.stackexchange.com/a/63203>
                            let v0: Vector3<f32> = tri[1].pos - tri[0].pos;
                            let v1: Vector3<f32> = tri[2].pos - tri[0].pos;
                            let v2_xs: f32x16 = pixel_xs - f32x16::splat(tri[0].pos.x);
                            let v2_ys: f32x16 = pixel_ys - f32x16::splat(tri[0].pos.y);
                            let denom_recip: f32 = 1.0 / ((v0.x * v1.y) - (v1.x * v0.y));
                            let ss_vs: f32x16 = ((v2_xs * f32x16::splat(v1.y))
                                - (v2_ys * f32x16::splat(v1.x)))
                                * f32x16::splat(denom_recip);
                            let ss_ws: f32x16 = ((v2_ys * f32x16::splat(v0.x))
                                - (v2_xs * f32x16::splat(v0.y)))
                                * f32x16::splat(denom_recip);
                            let ss_us: f32x16 = f32x16::splat(1.0) - ss_vs - ss_ws;
                            (ss_us, ss_vs, ss_ws)
                        };
                        // Calculate perspective-correct barycentrics per pixel.
                        let (bary_us, bary_vs, bary_ws) = {
                            let unnorm_us = screen_bary_us * f32x16::splat(tri[0].w);
                            let unnorm_vs = screen_bary_vs * f32x16::splat(tri[1].w);
                            let unnorm_ws = screen_bary_ws * f32x16::splat(tri[2].w);
                            let recip_sum =
                                f32x16::splat(1.0) / (unnorm_us + unnorm_vs + unnorm_ws);
                            (
                                unnorm_us * recip_sum,
                                unnorm_vs * recip_sum,
                                unnorm_ws * recip_sum,
                            )
                        };
                        // Interpolate UVs.
                        let pixel_us = (f32x16::splat(tri[0].uv.x) * bary_us)
                            + (f32x16::splat(tri[1].uv.x) * bary_vs)
                            + (f32x16::splat(tri[2].uv.x) * bary_ws);
                        let pixel_vs = (f32x16::splat(tri[0].uv.y) * bary_us)
                            + (f32x16::splat(tri[1].uv.y) * bary_vs)
                            + (f32x16::splat(tri[2].uv.y) * bary_ws);
                        // Interpolate depths.
                        let pixel_depths = (f32x16::splat(tri[0].pos.z) * screen_bary_us)
                            + (f32x16::splat(tri[1].pos.z) * screen_bary_vs)
                            + (f32x16::splat(tri[2].pos.z) * screen_bary_ws);
                        // Convert tri RGBA to Unorm8s.
                        let [
                            pixel_tri_reds,
                            pixel_tri_greens,
                            pixel_tri_blues,
                            pixel_tri_alphas,
                        ] = rgba_u32x16_to_unorm8x16(u32x16::splat(tri_info.rgba));
                        // Sample atlas texture for each pixel.
                        let [
                            pixel_tex_reds,
                            pixel_tex_greens,
                            pixel_tex_blues,
                            pixel_tex_alphas,
                        ] = atlas_texture.sample_nearest_simd16_masked(
                            pixel_us,
                            pixel_vs,
                            visible_pixels,
                        );
                        // Alpha test pixels.
                        if tri_info.alpha_test {
                            visible_pixels &= pixel_tex_alphas.0.simd_gt(u8x16::splat(0x00)).cast();
                        }
                        // Depth-test pixels.
                        // Only loading depth values for pixels which could be visible has the
                        // potential to save us a small amount of memory bandwidth.
                        let loaded_pixel_group_depths = f32x16::load_select(
                            pixel_group_depth,
                            visible_pixels,
                            f32x16::splat(0.0),
                        );
                        visible_pixels &= pixel_depths.simd_ge(loaded_pixel_group_depths);
                        // Calculate final pixel colours.
                        let new_pixel_reds = pixel_tri_reds * pixel_tex_reds;
                        let new_pixel_greens = pixel_tri_greens * pixel_tex_greens;
                        let new_pixel_blues = pixel_tri_blues * pixel_tex_blues;
                        let new_pixel_alphas = pixel_tri_alphas * pixel_tex_alphas;
                        // Convert final pixel colours to RGBA8, and write to tile.
                        let new_pixel_rgbas = rgba_4xunorm8x16_to_u32x16([
                            new_pixel_reds,
                            new_pixel_greens,
                            new_pixel_blues,
                            new_pixel_alphas,
                        ]);
                        new_pixel_rgbas.store_select(pixel_group.as_mut_array(), visible_pixels);
                        // Update pixel depths.
                        pixel_depths.store_select(pixel_group_depth, visible_pixels);
                        // Update pixel group occlusion culling info.
                        out_tile_hi_z.pixel_group[micro_tile_i][pixel_group_i] =
                            f32x16::from_array(*pixel_group_depth).reduce_min();
                    }
                    // Update micro-tile occlusion culling info.
                    out_tile_hi_z.micro_tile[micro_tile_i] =
                        f32x16::from_array(out_tile_hi_z.pixel_group[micro_tile_i]).reduce_min();
                }
                // Update tile occlusion culling info.
                out_tile_hi_z.tile = f32x16::from_array(out_tile_hi_z.micro_tile).reduce_min();
            }
        }
    }
}

#[tracing::instrument(skip_all)]
pub fn process_subchunk(
    block_registry: &resources::block::Registry,
    model_registry: &ModelRegistry,
    raw_chunks: &FastHashMap<[i32; 2], Arc<crate::RawChunk>>,
    pending_subchunk_tx: &Sender<Option<([i32; 3], Subchunk)>>,
    subchunk_coords: [i32; 3],
    dispatch_id: u64,
) {
    let spruce_leaves_registry_index = block_registry
        .get_index_from_identifier(&identifier!("minecraft:spruce_leaves"))
        .unwrap();
    let [subchunk_x, subchunk_y, subchunk_z] = subchunk_coords;
    let Some(chunk) = &raw_chunks.get(&[subchunk_x, subchunk_z]) else {
        pending_subchunk_tx.send(None).unwrap();
        return;
    };
    let chunk_section = &chunk.sections[usize::try_from(subchunk_y).unwrap()];
    if chunk_section.block_count == 0 {
        pending_subchunk_tx.send(None).unwrap();
        return;
    }
    // Skip chunks with missing neighbours, so that for every chunk we actually render, it
    // has all its neighbours to decide whether border faces should be rendered.
    // I believe Minecraft does the same.
    {
        let surrounding_chunk_coords = [
            [subchunk_x - 1, subchunk_z],
            [subchunk_x + 1, subchunk_z],
            [subchunk_x, subchunk_z - 1],
            [subchunk_x, subchunk_z + 1],
        ];
        for neighbour_chunk in surrounding_chunk_coords {
            if !raw_chunks.contains_key(&neighbour_chunk) {
                pending_subchunk_tx.send(None).unwrap();
                return;
            }
        }
    }
    let mut block_faces: [Vec<BlockFace>; 6] = Default::default();
    let mut tinted_block_faces: [Vec<TintedBlockFace>; 6] = Default::default();
    let mut custom_block_faces: Vec<CustomBlockFace> = Vec::new();
    for y in 0..SUBCHUNK_AXIS_LEN {
        let global_y_i32 = (SUBCHUNK_AXIS_LEN_I32 * subchunk_y) + y as i32 + MIN_HEIGHT_I32;
        let global_y = global_y_i32 as f32;
        for z in 0..SUBCHUNK_AXIS_LEN {
            let global_z_i32 = (SUBCHUNK_AXIS_LEN_I32 * subchunk_z) + z as i32;
            let global_z = global_z_i32 as f32;
            for x in 0..SUBCHUNK_AXIS_LEN {
                let global_x_i32 = (SUBCHUNK_AXIS_LEN_I32 * subchunk_x) + x as i32;
                let global_x = global_x_i32 as f32;
                let global_palette_index = chunk_section.block_states.get(x, y, z);
                let blockstate_info = &block_registry[global_palette_index];
                let model_idx = match &blockstate_info.model_data {
                    blockstate::ModelData::Single(model_idx) => *model_idx,
                    blockstate::ModelData::RandomChoice(models) => 'model_blk: {
                        // Find weight for model by hashed position.
                        let mut block_hasher = AHasher::default();
                        block_hasher.write_i32(global_x_i32);
                        block_hasher.write_i32(global_y_i32);
                        block_hasher.write_i32(global_z_i32);
                        let hash = block_hasher.finish();
                        let mut current_percentage = (hash % 65537) as f32 / 65536.0;
                        for variant in models.iter() {
                            if current_percentage <= variant.weight {
                                break 'model_blk variant.model;
                            } else {
                                current_percentage -= variant.weight;
                            }
                        }
                        // Should be unreachable
                        let variant = &models[models.len() - 1];
                        variant.model
                    }
                };
                let block_opacity = blockstate_info.extra_info.opacity;
                let direction_map = [
                    (x as i32, y as i32 + 1, z as i32),
                    (x as i32, y as i32 - 1, z as i32),
                    (x as i32, y as i32, z as i32 - 1),
                    (x as i32, y as i32, z as i32 + 1),
                    (x as i32 + 1, y as i32, z as i32),
                    (x as i32 - 1, y as i32, z as i32),
                ];
                let mut face_cull_map = [false; 6];
                let mut face_light_map = [[0u8; 2]; 6];
                for (i, (x, y, z)) in direction_map.into_iter().enumerate() {
                    let check_global_y = (SUBCHUNK_AXIS_LEN_I32 * subchunk_y + y) + MIN_HEIGHT_I32;
                    let check_chunk = match [x, z].iter().any(|n| !(0..=15).contains(n)) {
                        false => chunk,
                        true => match (x, z) {
                            (-1, _) => &raw_chunks[&[subchunk_x - 1, subchunk_z]],
                            (16, _) => &raw_chunks[&[subchunk_x + 1, subchunk_z]],
                            (_, -1) => &raw_chunks[&[subchunk_x, subchunk_z - 1]],
                            (_, 16) => &raw_chunks[&[subchunk_x, subchunk_z + 1]],
                            _ => unreachable!(),
                        },
                    };
                    // Get lighting
                    {
                        let light_section = check_chunk
                            .lighting
                            .get_section(
                                MIN_HEIGHT_I32,
                                check_global_y.div_euclid(SUBCHUNK_AXIS_LEN_I32),
                            )
                            .unwrap();
                        let (x, y, z) = (
                            ((x + SUBCHUNK_AXIS_LEN_I32) % SUBCHUNK_AXIS_LEN_I32) as usize,
                            y.rem_euclid(16) as usize,
                            ((z + SUBCHUNK_AXIS_LEN_I32) % SUBCHUNK_AXIS_LEN_I32) as usize,
                        );
                        face_light_map[i] = light_section.get(x, y, z);
                    }
                    if !(MIN_HEIGHT_I32..=MAX_HEIGHT_I32).contains(&check_global_y) {
                        continue;
                    }
                    let check_sections = &check_chunk.sections;
                    let indexing_section = &check_sections[usize::try_from(
                        (SUBCHUNK_AXIS_LEN_I32 * subchunk_y + y) / SUBCHUNK_AXIS_LEN_I32,
                    )
                    .unwrap()];
                    let (x, y, z) = (
                        ((x + SUBCHUNK_AXIS_LEN_I32) % SUBCHUNK_AXIS_LEN_I32) as usize,
                        y as usize,
                        ((z + SUBCHUNK_AXIS_LEN_I32) % SUBCHUNK_AXIS_LEN_I32) as usize,
                    );
                    let global_palette_index = indexing_section.block_states.get(x, y % 16, z);
                    let neighbour_blockstate_info = &block_registry[global_palette_index];
                    let neighbour_block_opacity = neighbour_blockstate_info.extra_info.opacity;
                    face_cull_map[i] = match (block_opacity, neighbour_block_opacity) {
                        (_, BlockOpacity::Opaque) => true,
                        (BlockOpacity::Glass, BlockOpacity::Glass) => true,
                        (BlockOpacity::GlassPane, BlockOpacity::GlassPane) => true,
                        (_, _) => false,
                    };
                }
                // Spruce Leaves are hardcoded, so override tint colour here.
                let tint_color = match blockstate_info.block_index {
                    ident if ident == spruce_leaves_registry_index => [0x61, 0x99, 0x61, 0xFF],
                    _ => [0x91, 0xBD, 0x59, 0xFF],
                };
                process_subchunk_model(
                    &mut block_faces,
                    &mut tinted_block_faces,
                    &mut custom_block_faces,
                    model_registry,
                    chunk,
                    block_opacity,
                    face_cull_map,
                    face_light_map,
                    tint_color,
                    [subchunk_x, subchunk_y, subchunk_z],
                    [global_x, global_y, global_z],
                    [x, y, z],
                    model_idx,
                );
            }
        }
    }
    // Runs a variant of Minecraft's cave culling algorithm, specifically the connected
    // face generation.
    // Outlined here: https://tomcc.github.io/2014/08/31/visibility-1.html
    let connectivity = 'connected_faces: {
        use crate::protocol::chunk::Palette;
        // If we can immediately tell all the subchunk blocks are opaque, skip this entire
        // process and just return that no subchunk faces are connected.
        match chunk_section.block_states.palette() {
            Palette::SingleValue(global_palette_index) => {
                let blockstate_info = &block_registry[*global_palette_index];
                break 'connected_faces match blockstate_info.extra_info.opacity {
                    BlockOpacity::Opaque => SubchunkConnectivity::empty(),
                    _ => SubchunkConnectivity::full(),
                };
            }
            Palette::Palette(indices) => {
                let mut num_opaque = 0;
                for global_palette_index in indices {
                    let blockstate_info = &block_registry[*global_palette_index];
                    if blockstate_info.extra_info.opacity == BlockOpacity::Opaque {
                        num_opaque += 1;
                    }
                }
                if num_opaque == 0 {
                    break 'connected_faces SubchunkConnectivity::full();
                } else if num_opaque == indices.len() {
                    break 'connected_faces SubchunkConnectivity::empty();
                }
            }
            Palette::Direct => {}
        }
        #[repr(transparent)]
        #[derive(Clone, Copy)]
        struct FaceSet(pub u8);
        impl FaceSet {
            pub fn empty() -> Self {
                Self(0)
            }

            pub fn add_dir(&mut self, dir: AxisDirection) {
                self.0 |= 1 << (dir as u8);
            }

            pub fn get_directions(&self) -> [(AxisDirection, bool); 6] {
                [
                    AxisDirection::Down,
                    AxisDirection::Up,
                    AxisDirection::North,
                    AxisDirection::South,
                    AxisDirection::West,
                    AxisDirection::East,
                ]
                .map(|dir| (dir, self.0 & (1 << (dir as u8)) != 0))
            }
        }
        let mut current_group: usize = 0;
        let mut current_group_faces = FaceSet::empty();
        let mut group_faces: Vec<FaceSet> = Vec::new();
        // Y major, then Z, then X.
        let mut unchecked_blocks = FixedBitSet::with_capacity(SUBCHUNK_AXIS_LEN.pow(3));
        #[inline]
        fn coords_to_bit_idx(coords: [i8; 3]) -> usize {
            let [x, y, z] = coords.map(|n| n as usize);
            y * SUBCHUNK_AXIS_LEN.pow(2) + z * SUBCHUNK_AXIS_LEN + x
        }
        unchecked_blocks.clear();
        // Add all non-opaque blocks
        for x in 0..SUBCHUNK_AXIS_LEN {
            for y in 0..SUBCHUNK_AXIS_LEN {
                for z in 0..SUBCHUNK_AXIS_LEN {
                    let global_palette_index = chunk_section.block_states.get(x, y, z);
                    let blockstate_info = &block_registry[global_palette_index];
                    if blockstate_info.extra_info.opacity != BlockOpacity::Opaque {
                        let bit_index = coords_to_bit_idx([x, y, z].map(|n| n as i8));
                        unchecked_blocks.insert(bit_index);
                    }
                }
            }
        }
        // Flood fill from each non-opaque block, to split all the blocks into groups.
        let mut queue: FastHashSet<[i8; 3]> = FastHashSet::new();
        while !queue.is_empty() || !unchecked_blocks.is_clear() {
            let [x, y, z] = queue
                .iter()
                .copied()
                .next()
                .inspect(|coord| {
                    queue.remove(coord);
                })
                .unwrap_or_else(|| {
                    // No more blocks in queue, make a new group and grab a new block
                    // that hasn't been checked yet.
                    let coord = {
                        let bit_index = unchecked_blocks.minimum().unwrap();
                        [
                            (bit_index & 0xF) as i8,
                            ((bit_index >> 8) & 0xF) as i8,
                            ((bit_index >> 4) & 0xF) as i8,
                        ]
                    };
                    group_faces.push(current_group_faces);
                    current_group += 1;
                    current_group_faces = FaceSet::empty();
                    coord
                });
            unchecked_blocks.remove(coords_to_bit_idx([x, y, z]));
            let surrounding_block_coords = [
                [x - 1, y, z],
                [x + 1, y, z],
                [x, y, z - 1],
                [x, y, z + 1],
                [x, y - 1, z],
                [x, y + 1, z],
            ];
            for new_coord in surrounding_block_coords {
                let [new_x, new_y, new_z] = new_coord;
                // If fill escapes subchunk, add escaping face to group
                if new_x < 0 {
                    current_group_faces.add_dir(AxisDirection::West);
                } else if new_x >= SUBCHUNK_AXIS_LEN as i8 {
                    current_group_faces.add_dir(AxisDirection::East);
                } else if new_y < 0 {
                    current_group_faces.add_dir(AxisDirection::Down);
                } else if new_y >= SUBCHUNK_AXIS_LEN as i8 {
                    current_group_faces.add_dir(AxisDirection::Up);
                } else if new_z < 0 {
                    current_group_faces.add_dir(AxisDirection::North);
                } else if new_z >= SUBCHUNK_AXIS_LEN as i8 {
                    current_group_faces.add_dir(AxisDirection::South);
                } else if unchecked_blocks.contains(coords_to_bit_idx(new_coord)) {
                    queue.insert(new_coord);
                }
            }
        }
        group_faces.push(current_group_faces);
        // Add connected faces for each group to subchunk connectivity
        let mut subchunk_connectivity = SubchunkConnectivity::empty();
        for face_set in group_faces {
            let directions = face_set.get_directions();
            for (face_1, face_1_in_set) in directions {
                if !face_1_in_set {
                    continue;
                }
                for (face_2, face_2_in_set) in directions {
                    if !face_2_in_set {
                        continue;
                    }
                    subchunk_connectivity.add_connection(&face_1, &face_2);
                }
            }
        }
        subchunk_connectivity
    };
    let start_coords = [
        SUBCHUNK_AXIS_LEN_I32 * subchunk_x,
        SUBCHUNK_AXIS_LEN_I32 * subchunk_y + MIN_HEIGHT_I32,
        SUBCHUNK_AXIS_LEN_I32 * subchunk_z,
    ];
    let (block_faces, block_face_group_lengths) = {
        let mut faces = Vec::new();
        let mut face_group_lens = [0; 6];
        for (group_i, face_group) in block_faces.into_iter().enumerate() {
            face_group_lens[group_i] = face_group.len().try_into().unwrap();
            faces.extend(face_group);
        }
        (faces.into_boxed_slice(), face_group_lens)
    };
    let (tinted_block_faces, tinted_block_face_group_lengths) = {
        let mut faces = Vec::new();
        let mut face_group_lens = [0; 6];
        for (group_i, face_group) in tinted_block_faces.into_iter().enumerate() {
            face_group_lens[group_i] = face_group.len().try_into().unwrap();
            faces.extend(face_group);
        }
        (faces.into_boxed_slice(), face_group_lens)
    };
    pending_subchunk_tx
        .send(Some((
            subchunk_coords,
            Subchunk {
                dispatch_id,
                start_coords: start_coords.into(),
                connectivity,
                block_faces: if block_faces.is_empty() {
                    None
                } else {
                    Some(block_faces)
                },
                block_face_group_lengths,
                tinted_block_faces: if tinted_block_faces.is_empty() {
                    None
                } else {
                    Some(tinted_block_faces)
                },
                tinted_block_face_group_lengths,
                custom_block_faces: if custom_block_faces.is_empty() {
                    None
                } else {
                    Some(custom_block_faces.into_boxed_slice())
                },
            },
        )))
        .unwrap();
}

#[allow(clippy::too_many_arguments)]
fn process_subchunk_model(
    block_faces: &mut [Vec<BlockFace>; 6],
    tinted_block_faces: &mut [Vec<TintedBlockFace>; 6],
    custom_block_faces: &mut Vec<CustomBlockFace>,
    model_registry: &ModelRegistry,
    chunk: &crate::RawChunk,
    block_opacity: BlockOpacity,
    face_cull_map: [bool; 6],
    face_light_map: [[u8; 2]; 6],
    tint_color: [u8; 4],
    [subchunk_x, subchunk_y, subchunk_z]: [i32; 3],
    [global_x, global_y, global_z]: [f32; 3],
    [x, y, z]: [usize; 3],
    model_idx: ModelIndex,
) {
    let model = &model_registry[model_idx];
    match model {
        ModelType::None => {}
        ModelType::Block(info) => {
            match block_opacity {
                BlockOpacity::Opaque => {
                    for i in 0..6 {
                        if face_cull_map[i] {
                            continue;
                        }
                        block_faces[i].push(BlockFace::new(
                            [x as u8, y as u8, z as u8],
                            info.per_face_atlas_uvs[i],
                            info.per_face_uv_rotations[i],
                            face_light_map[i],
                        ));
                    }
                }
                _ => {
                    for i in 0..6 {
                        if face_cull_map[i] {
                            continue;
                        }
                        tinted_block_faces[i].push(TintedBlockFace::new(
                            [x as u8, y as u8, z as u8],
                            info.per_face_atlas_uvs[i],
                            info.per_face_uv_rotations[i],
                            face_light_map[i],
                            // Block doesn't have any tint, so just use opaque
                            // white as a null value.
                            [0xFF; 4],
                        ));
                    }
                }
            }
        }
        ModelType::TintedBlock(info) => {
            for i in 0..6 {
                if face_cull_map[i] {
                    continue;
                }
                tinted_block_faces[i].push(TintedBlockFace::new(
                    [x as u8, y as u8, z as u8],
                    info.per_face_atlas_uvs[i],
                    info.per_face_uv_rotations[i],
                    face_light_map[i],
                    tint_color,
                ));
            }
        }
        ModelType::OverlayedBlock(info) => {
            for face in &info.faces {
                if face_cull_map[face.face_i as usize] {
                    continue;
                }
                if let Some(tint) = face.tint {
                    assert!(tint == Tint::Biome, "TODO: Alternative tints");
                    tinted_block_faces[face.face_i as usize].push(TintedBlockFace::new(
                        [x as u8, y as u8, z as u8],
                        face.atlas_uvs,
                        face.uv_rotation,
                        face_light_map[face.face_i as usize],
                        tint_color,
                    ));
                } else {
                    block_faces[face.face_i as usize].push(BlockFace::new(
                        [x as u8, y as u8, z as u8],
                        face.atlas_uvs,
                        face.uv_rotation,
                        face_light_map[face.face_i as usize],
                    ));
                }
            }
        }
        ModelType::Liquid(_info) => {
            // TODO:
        }
        ModelType::Other(info) => {
            let [start, len]: [usize; 2] = info.start_face_and_len.map(|n| n.try_into().unwrap());
            let faces = &model_registry.custom_block_faces[start..start + len];
            let light_section = chunk
                .lighting
                .get_section(MIN_HEIGHT_I32, subchunk_y + (MIN_HEIGHT_I32 / 16))
                .unwrap();
            custom_block_faces.extend(faces.iter().map(|face| {
                CustomBlockFace::new(
                    face,
                    [x as u8, y as u8, z as u8],
                    tint_color,
                    light_section.get(x, y, z),
                    face_light_map,
                )
            }));
        }
        ModelType::Composite(parts) => {
            for part in parts {
                process_subchunk_model(
                    block_faces,
                    tinted_block_faces,
                    custom_block_faces,
                    model_registry,
                    chunk,
                    block_opacity,
                    face_cull_map,
                    face_light_map,
                    tint_color,
                    [subchunk_x, subchunk_y, subchunk_z],
                    [global_x, global_y, global_z],
                    [x, y, z],
                    part.model_idx,
                );
            }
        }
    }
}
