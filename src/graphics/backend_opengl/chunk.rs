use super::{SubchunkDataStorage, gl};
use crate::basic_types::AxisDirection;
use crate::graphics::chunk::{HasSubchunkData, SubchunkConnectivity, SubchunkData};
use crate::{MAX_HEIGHT_I32, MIN_HEIGHT_I32, SUBCHUNK_AXIS_LEN, SUBCHUNK_AXIS_LEN_I32};
use ahash::AHasher;
use core::hash::Hasher;
use fixedbitset::FixedBitSet;
use gl::buffer::BufferType;
use nalgebra::{Matrix4, Point3, Rotation3, Vector3, Vector4};
use portable_std::{FastHashMap, FastHashSet};
use resources::block::RightAngleRotation;
use resources::block::blockstate::{self, BlockOpacity};
use resources::block::model::{ModelIndex, ModelType};
use resources::block::model::{ModelRegistry, Tint};
use resources::identifier;
use std::sync::Arc;
use std::sync::mpsc::Sender;

pub struct Subchunk {
    pub dispatch_id: u64,
    pub start_coords: [i32; 3],
    /// Will be `None` if the subchunk contains no faces.
    pub buffer: Option<gl::buffer::batch_collected::Buffer>,
    /// Equal to `u32::MAX` if the direction group contains no faces.
    /// The seventh group is always rendered, containing faces not in a direction group.
    pub group_start_vertices: [u32; 7],
    pub group_vertex_counts: [u32; 7],
    pub connectivity: SubchunkConnectivity,
}

impl HasSubchunkData for Subchunk {
    fn get_data(&self) -> SubchunkData {
        SubchunkData {
            start_coords: self.start_coords,
            connectivity: self.connectivity,
        }
    }
}

#[inline]
pub fn face_matrix_rotations() -> [Rotation3<f32>; 6] {
    [
        // Top
        Rotation3::identity(),
        // Bottom
        Rotation3::from_euler_angles(core::f32::consts::PI, 0.0, 0.0),
        // North
        Rotation3::from_euler_angles(-core::f32::consts::FRAC_PI_2, 0.0, core::f32::consts::PI),
        // South
        Rotation3::from_euler_angles(core::f32::consts::FRAC_PI_2, 0.0, 0.0),
        // East
        Rotation3::from_euler_angles(
            0.0,
            core::f32::consts::FRAC_PI_2,
            -core::f32::consts::FRAC_PI_2,
        ),
        // West
        Rotation3::from_euler_angles(
            0.0,
            -core::f32::consts::FRAC_PI_2,
            core::f32::consts::FRAC_PI_2,
        ),
    ]
}

pub fn generate_subchunk_matrix(subchunk_start_coords: [i32; 3]) -> [[f32; 4]; 4] {
    let subchunk_start_coords = subchunk_start_coords.map(|n| n as f32);
    // TODO: Apply a translation to the middle of the subchunk, so we can get more range out of an
    //       i16.
    Matrix4::identity()
        .append_scaling(1.0 / 1024.0)
        .append_translation(&Vector3::from(subchunk_start_coords))
        .into()
}

#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct BlockVertex {
    /// Local subchunk position, multiplied by 1024, rounded to an [`i16`].
    pub subchunk_fixed_point_pos: [i16; 3],
    // FIXME: This should be `i16`.
    pub uvs: [u16; 2],
    pub colour_rgba: [u8; 4],
}

// TODO: Switch to using GL_QUADS.
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(transparent)]
pub struct BlockFace(pub [BlockVertex; 6]);

impl BlockFace {
    pub fn new_basic(
        subchunk_xyz: [u8; 3],
        uvs: [u16; 4],
        uv_rotation: RightAngleRotation,
        light_levels: [u8; 2],
        face_i: usize,
    ) -> Self {
        debug_assert!(subchunk_xyz[0] < 16);
        debug_assert!(subchunk_xyz[1] < 16);
        debug_assert!(subchunk_xyz[2] < 16);
        debug_assert!(light_levels[0] < 16);
        debug_assert!(light_levels[1] < 16);
        debug_assert!(face_i < 6);
        // Calculate the vertex subchunk positions.
        let base_positions: [Vector3<f32>; 4] = [
            Vector3::new(-0.5, 0.5, 0.5),
            Vector3::new(0.5, 0.5, 0.5),
            Vector3::new(-0.5, 0.5, -0.5),
            Vector3::new(0.5, 0.5, -0.5),
        ];
        let face_matrix = face_matrix_rotations()[face_i];
        let subchunk_xyz_vec3 = Vector3::from(subchunk_xyz.map(|n| n as f32));
        let subchunk_fixed_point_positions: [[i16; 3]; 4] = base_positions
            // Rotate by the face matrix.
            .map(|face_local_pos| face_matrix.transform_vector(&face_local_pos))
            // Add the XYZ offset of the block within the subchunk, as well as the 0.5 offset.
            .map(|block_local_pos| (block_local_pos + subchunk_xyz_vec3).add_scalar(0.5))
            // Scale...
            .map(|subchunk_local_pos| subchunk_local_pos * 1024.0)
            // ...and convert to a fixed-point `i16`.
            .map(|scaled_pos| <[f32; 3]>::from(scaled_pos).map(|n| n.round() as i16));
        // Calculate the vertex UVs.
        let base_uvs = [
            // NOTE: This flips the UV coordinates, which seems to be needed?
            [uvs[0], uvs[3]],
            [uvs[2], uvs[3]],
            [uvs[0], uvs[1]],
            [uvs[2], uvs[1]],
        ];
        let rotated_uv_indices = match uv_rotation {
            RightAngleRotation::Zero => [0, 1, 2, 3],
            RightAngleRotation::Ninety => [1, 3, 0, 2],
            RightAngleRotation::OneEighty => [3, 2, 1, 0],
            RightAngleRotation::TwoSeventy => [2, 0, 3, 1],
        };
        let rotated_uvs = rotated_uv_indices.map(|i| base_uvs[i]);
        // Calculate block lighting.
        let block_light_rgb_vec3 = Self::calculate_light_rgb_vec3(light_levels);
        // Calculate per-face directional lighting.
        let face_normal = face_matrix.transform_vector(&Vector3::y());
        let dir_light_coef = Self::calculate_dir_light_coef(face_normal);
        // Calculate final face colour.
        let colour_rgb_vec3 = block_light_rgb_vec3 * dir_light_coef;
        let colour_rgba_vec4 = colour_rgb_vec3.push(1.0);
        let colour_rgba: [u8; 4] = colour_rgba_vec4.map(|n| (n * 255.0).round() as u8).into();
        // Assemble the 4 vertices making up a face.
        let face_vertices: [BlockVertex; 4] = core::array::from_fn(|i| BlockVertex {
            subchunk_fixed_point_pos: subchunk_fixed_point_positions[i],
            uvs: rotated_uvs[i],
            colour_rgba,
        });
        // Return face as 2 triangles.
        Self([0, 1, 2, 1, 3, 2].map(|i| face_vertices[i]))
    }

    pub fn new_tinted(
        subchunk_xyz: [u8; 3],
        uvs: [u16; 4],
        uv_rotation: RightAngleRotation,
        light_levels: [u8; 2],
        tint_rgba: [u8; 4],
        face_i: usize,
    ) -> Self {
        debug_assert!(subchunk_xyz[0] < 16);
        debug_assert!(subchunk_xyz[1] < 16);
        debug_assert!(subchunk_xyz[2] < 16);
        debug_assert!(light_levels[0] < 16);
        debug_assert!(light_levels[1] < 16);
        debug_assert!(face_i < 6);
        // Calculate the vertex subchunk positions.
        let base_positions: [Vector3<f32>; 4] = [
            Vector3::new(-0.5, 0.5, 0.5),
            Vector3::new(0.5, 0.5, 0.5),
            Vector3::new(-0.5, 0.5, -0.5),
            Vector3::new(0.5, 0.5, -0.5),
        ];
        let face_matrix = face_matrix_rotations()[face_i];
        let subchunk_xyz_vec3 = Vector3::from(subchunk_xyz.map(|n| n as f32));
        let subchunk_fixed_point_positions: [[i16; 3]; 4] = base_positions
            // Rotate by the face matrix.
            .map(|face_local_pos| face_matrix.transform_vector(&face_local_pos))
            // Add the XYZ offset of the block within the subchunk, as well as the 0.5 offset.
            .map(|block_local_pos| (block_local_pos + subchunk_xyz_vec3).add_scalar(0.5))
            // Scale...
            .map(|subchunk_local_pos| subchunk_local_pos * 1024.0)
            // ...and convert to a fixed-point `i16`.
            .map(|scaled_pos| <[f32; 3]>::from(scaled_pos).map(|n| n.round() as i16));
        // Calculate the vertex UVs.
        let base_uvs = [
            // NOTE: This flips the UV coordinates, which seems to be needed?
            [uvs[0], uvs[3]],
            [uvs[2], uvs[3]],
            [uvs[0], uvs[1]],
            [uvs[2], uvs[1]],
        ];
        let rotated_uv_indices = match uv_rotation {
            RightAngleRotation::Zero => [0, 1, 2, 3],
            RightAngleRotation::Ninety => [1, 3, 0, 2],
            RightAngleRotation::OneEighty => [3, 2, 1, 0],
            RightAngleRotation::TwoSeventy => [2, 0, 3, 1],
        };
        let rotated_uvs = rotated_uv_indices.map(|i| base_uvs[i]);
        // Calculate block lighting.
        let block_light_rgb_vec3 = Self::calculate_light_rgb_vec3(light_levels);
        // Calculate per-face directional lighting.
        let face_normal = face_matrix.transform_vector(&Vector3::y());
        let dir_light_coef = Self::calculate_dir_light_coef(face_normal);
        // Calculate final face colour.
        let light_rgb_vec3 = block_light_rgb_vec3 * dir_light_coef;
        let light_rgba_vec4 = light_rgb_vec3.push(1.0);
        let tint_rgba_vec4 = Vector4::from(tint_rgba.map(|x| x as f32 / 255.0));
        let colour_rgba_vec4 = light_rgba_vec4.component_mul(&tint_rgba_vec4);
        let colour_rgba: [u8; 4] = colour_rgba_vec4.map(|n| (n * 255.0).round() as u8).into();
        // Assemble the 4 vertices making up a face.
        let face_vertices: [BlockVertex; 4] = core::array::from_fn(|i| BlockVertex {
            subchunk_fixed_point_pos: subchunk_fixed_point_positions[i],
            uvs: rotated_uvs[i],
            colour_rgba,
        });
        // Return face as 2 triangles.
        Self([0, 1, 2, 1, 3, 2].map(|i| face_vertices[i]))
    }

    pub fn new_custom(
        model_face: &resources::block::model::CustomModelFace,
        subchunk_xyz: [u8; 3],
        tint_rgba: [u8; 4],
        centre_light_levels: [u8; 2],
        neighbour_light_levels: [[u8; 2]; 6],
    ) -> Self {
        let subchunk_xyz_vec3 = Vector3::from(subchunk_xyz.map(|n| n as f32));
        let tint_rgba_vec4 = Vector4::from(tint_rgba.map(|n| n as f32 / 255.0));
        // The tint colour will be multiplied into the final colour RGBA, so just make it one
        // if the vertex isn't tinted.
        let applied_tint_rgba_vec4 = match model_face.tint {
            None => Vector4::repeat(1.0),
            Some(Tint::Biome) => tint_rgba_vec4,
        };
        // Calculate per-face directional lighting.
        let light_source_dir = Vector3::new(2.0, 5.0, 1.0).normalize();
        let dir_lighting = Vector3::dot(&model_face.normal, &light_source_dir);
        let dir_light_coef = f32::mul_add(dir_lighting, 0.3, 0.7);
        // Convert face vertices.
        let normal_offset = model_face.normal * 0.02;
        let face_vertices = model_face.vertices.map(|v| {
            // Calculate the vertex subchunk fixed-point position.
            let subchunk_pos = v.local_pos + subchunk_xyz_vec3 + Vector3::repeat(0.5);
            let subchunk_fixed_point_pos: [i16; 3] =
                (subchunk_pos * 1024.0).map(|n| n.round() as i16).into();
            // Calculate block lighting.
            let block_light_rgb_vec3 = {
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
                Self::calculate_light_rgb_vec3(closest_light_levels)
            };
            // Combine the block lighting with the per-face directional lighting, expand to RGBA.
            let light_rgb_vec3 = block_light_rgb_vec3 * dir_light_coef;
            let light_rgba_vec4 = light_rgb_vec3.push(1.0);
            // Final vertex colour is the tint colour combined with lighting.
            let colour_rgba_vec4 = applied_tint_rgba_vec4.component_mul(&light_rgba_vec4);
            let colour_rgba: [u8; 4] = colour_rgba_vec4.map(|n| (n * 255.0).round() as u8).into();
            BlockVertex {
                subchunk_fixed_point_pos,
                uvs: v.uvs,
                colour_rgba,
            }
        });
        // Return face as 2 triangles.
        Self([1, 0, 2, 1, 2, 3].map(|i| face_vertices[i]))
    }

    pub(super) fn calculate_light_rgb_vec3(light_levels: [u8; 2]) -> Vector3<f32> {
        debug_assert!(light_levels[0] < 16);
        debug_assert!(light_levels[1] < 16);
        let [sky_light_level, block_light_level] = light_levels;
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

    fn calculate_dir_light_coef(normal: Vector3<f32>) -> f32 {
        let light_source_dir = Vector3::new(2.0, 5.0, 1.0).normalize();
        let dir_lighting = Vector3::dot(&normal, &light_source_dir);
        f32::mul_add(dir_lighting, 0.3, 0.7)
    }
}

#[derive(Debug)]
pub struct RawSubchunk {
    pub dispatch_id: u64,
    pub subchunk_coords: [i32; 3],
    pub start_coords: [i32; 3],
    pub face_groups: [Vec<BlockFace>; 7],
    pub connectivity: SubchunkConnectivity,
}

#[tracing::instrument(skip_all)]
pub fn process_subchunk(
    block_registry: &resources::block::Registry,
    model_registry: &ModelRegistry,
    raw_chunks: &FastHashMap<[i32; 2], Arc<crate::RawChunk>>,
    pending_subchunk_tx: &Sender<Option<RawSubchunk>>,
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
    let mut face_groups: [Vec<BlockFace>; 7] = Default::default();
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
                // Spruce Leaves are hardcoded, so override tint colour here
                let tint_color = match blockstate_info.block_index {
                    ident if ident == spruce_leaves_registry_index => [0x61, 0x99, 0x61, 0xFF],
                    _ => [0x91, 0xBD, 0x59, 0xFF],
                };
                process_subchunk_model(
                    &mut face_groups,
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
    let connectivity = 'connectivity: {
        use crate::protocol::chunk::Palette;
        // If we can immediately tell all the subchunk blocks are opaque, skip this entire
        // process and just return that no subchunk faces are connected.
        match chunk_section.block_states.palette() {
            Palette::SingleValue(global_palette_index) => {
                let blockstate_info = &block_registry[*global_palette_index];
                break 'connectivity match blockstate_info.extra_info.opacity {
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
                    break 'connectivity SubchunkConnectivity::full();
                } else if num_opaque == indices.len() {
                    break 'connectivity SubchunkConnectivity::empty();
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
    pending_subchunk_tx
        .send(Some(RawSubchunk {
            dispatch_id,
            subchunk_coords,
            start_coords,
            face_groups,
            connectivity,
        }))
        .unwrap();
}

#[tracing::instrument(skip_all)]
pub unsafe fn finalise_subchunk(
    subchunk_data_storage: &mut SubchunkDataStorage,
    raw_subchunk: RawSubchunk,
) {
    unsafe {
        let subchunk_coords = raw_subchunk.subchunk_coords;
        let mut faces = Vec::new();
        let mut group_start_vertices = [u32::MAX; 7];
        let mut group_vertex_counts = [0; 7];
        for (i, face_group) in raw_subchunk
            .face_groups
            .into_iter()
            .enumerate()
            .filter(|(_i, group)| !group.is_empty())
        {
            group_start_vertices[i] = (faces.len() * 6).try_into().unwrap();
            group_vertex_counts[i] = (face_group.len() * 6).try_into().unwrap();
            faces.extend(face_group);
        }
        let buffer = if !faces.is_empty() {
            let [buffer] = gl::buffer::batch_collected::Buffer::make_array();
            buffer.bind(BufferType::ArrayBuffer);
            let faces_bytes: &[u8] = bytemuck::cast_slice(&faces);
            gl::buffer::set_current_buffer_data_raw(
                BufferType::ArrayBuffer,
                faces_bytes.len().try_into().unwrap(),
                faces_bytes.as_ptr() as *const (),
                gl::buffer::DataUsageHint::StaticDraw,
            );
            Some(buffer)
        } else {
            None
        };
        subchunk_data_storage.subchunks.insert(
            subchunk_coords,
            Subchunk {
                dispatch_id: raw_subchunk.dispatch_id,
                start_coords: raw_subchunk.start_coords,
                buffer,
                group_start_vertices,
                group_vertex_counts,
                connectivity: raw_subchunk.connectivity,
            },
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn process_subchunk_model(
    face_groups: &mut [Vec<BlockFace>; 7],
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
                        face_groups[i].push(BlockFace::new_basic(
                            [x as u8, y as u8, z as u8],
                            info.per_face_atlas_uvs[i],
                            info.per_face_uv_rotations[i],
                            face_light_map[i],
                            i,
                        ));
                    }
                }
                _ => {
                    for i in 0..6 {
                        if face_cull_map[i] {
                            continue;
                        }
                        face_groups[i].push(BlockFace::new_tinted(
                            [x as u8, y as u8, z as u8],
                            info.per_face_atlas_uvs[i],
                            info.per_face_uv_rotations[i],
                            face_light_map[i],
                            // Block doesn't have any tint, so just use opaque
                            // white as a null value.
                            [0xFF; 4],
                            i,
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
                face_groups[i].push(BlockFace::new_tinted(
                    [x as u8, y as u8, z as u8],
                    info.per_face_atlas_uvs[i],
                    info.per_face_uv_rotations[i],
                    face_light_map[i],
                    tint_color,
                    i,
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
                    face_groups[face.face_i as usize].push(BlockFace::new_tinted(
                        [x as u8, y as u8, z as u8],
                        face.atlas_uvs,
                        face.uv_rotation,
                        face_light_map[face.face_i as usize],
                        tint_color,
                        face.face_i as usize,
                    ));
                } else {
                    face_groups[face.face_i as usize].push(BlockFace::new_basic(
                        [x as u8, y as u8, z as u8],
                        face.atlas_uvs,
                        face.uv_rotation,
                        face_light_map[face.face_i as usize],
                        face.face_i as usize,
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
            face_groups[6].reserve(faces.len());
            let light_section = chunk
                .lighting
                .get_section(MIN_HEIGHT_I32, subchunk_y + (MIN_HEIGHT_I32 / 16))
                .unwrap();
            for face in faces {
                face_groups[6].push(BlockFace::new_custom(
                    face,
                    [x as u8, y as u8, z as u8],
                    tint_color,
                    light_section.get(x, y, z),
                    face_light_map,
                ));
            }
        }
        ModelType::Composite(parts) => {
            for part in parts {
                process_subchunk_model(
                    face_groups,
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
