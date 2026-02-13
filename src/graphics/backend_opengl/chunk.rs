use super::{SubchunkDataStorage, gl};
use crate::graphics::chunk::{HasSubchunkData, SubchunkConnectivity, SubchunkData};
use crate::{MIN_HEIGHT_I32, SUBCHUNK_AXIS_LEN_I32};
use gl::buffer::BufferType;
use nalgebra::{Matrix4, Point3, Rotation3, Vector3, Vector4};
use portable_std::FastHashMap;
use resources::block::RightAngleRotation;
use resources::block::blockstate::BlockOpacity;
use resources::block::model::{ModelIndex, ModelType};
use resources::block::model::{ModelRegistry, Tint};
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
    pub tint_colour_and_dir_light_rgba: [u8; 4],
    /// `[Sky light level, Block light level]`
    pub light_levels: [u8; 2],
}

#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(transparent)]
pub struct BlockFace(pub [BlockVertex; 4]);

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
        // Calculate per-face directional lighting.
        let face_normal = face_matrix.transform_vector(&Vector3::y());
        let dir_light_coef = Self::calculate_dir_light_coef(face_normal);
        // Calculate final face colour.
        let tint_colour_and_dir_light_rgba: [u8; 4] = [(dir_light_coef * 255.0).round() as u8; 4];
        // Assemble vertices.
        let face_vertices: [BlockVertex; 4] = core::array::from_fn(|i| BlockVertex {
            subchunk_fixed_point_pos: subchunk_fixed_point_positions[i],
            uvs: rotated_uvs[i],
            tint_colour_and_dir_light_rgba,
            light_levels,
        });
        Self([0, 1, 3, 2].map(|i| face_vertices[i]))
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
        // Calculate per-face directional lighting.
        let face_normal = face_matrix.transform_vector(&Vector3::y());
        let dir_light_coef = Self::calculate_dir_light_coef(face_normal);
        // Calculate final face colour.
        let tint_rgba_vec4 = Vector4::from(tint_rgba.map(|x| x as f32 / 255.0));
        let colour_rgba_vec4 = tint_rgba_vec4 * dir_light_coef;
        let tint_colour_and_dir_light_rgba: [u8; 4] =
            colour_rgba_vec4.map(|n| (n * 255.0).round() as u8).into();
        // Assemble vertices.
        let face_vertices: [BlockVertex; 4] = core::array::from_fn(|i| BlockVertex {
            subchunk_fixed_point_pos: subchunk_fixed_point_positions[i],
            uvs: rotated_uvs[i],
            tint_colour_and_dir_light_rgba,
            light_levels,
        });
        Self([0, 1, 3, 2].map(|i| face_vertices[i]))
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
            let light_levels = {
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
                light_levels[closest_light_i]
            };
            // Combine tint colour with directional lighting.
            let tint_colour_and_dir_light_rgba_vec4 = applied_tint_rgba_vec4 * dir_light_coef;
            let tint_colour_and_dir_light_rgba: [u8; 4] = tint_colour_and_dir_light_rgba_vec4
                .map(|n| (n * 255.0).round() as u8)
                .into();
            BlockVertex {
                subchunk_fixed_point_pos,
                uvs: v.uvs,
                tint_colour_and_dir_light_rgba,
                light_levels,
            }
        });
        Self([2, 3, 1, 0].map(|i| face_vertices[i]))
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
    let [subchunk_x, subchunk_y, subchunk_z] = subchunk_coords;
    let mut face_groups: [Vec<BlockFace>; 7] = Default::default();
    let Some(connectivity) = crate::graphics::chunk::process_subchunk_models(
        block_registry,
        model_registry,
        raw_chunks,
        subchunk_coords,
        |model_processing_args| {
            let crate::graphics::chunk::ModelProcessingArgs {
                model_registry,
                chunk,
                block_opacity,
                face_cull_map,
                face_light_map,
                tint_color,
                subchunk_xyz,
                global_xyz,
                xyz,
                model_idx,
            } = model_processing_args;
            process_subchunk_model(
                &mut face_groups,
                model_registry,
                chunk,
                block_opacity,
                face_cull_map,
                face_light_map,
                tint_color,
                subchunk_xyz,
                global_xyz,
                xyz,
                model_idx,
            );
        },
    ) else {
        // Skip subchunk if `process_subchunk_models` returns that it's invisible.
        return;
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
            group_start_vertices[i] = (faces.len() * 4).try_into().unwrap();
            group_vertex_counts[i] = (face_group.len() * 4).try_into().unwrap();
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
