// XXX: DEBUG
#![cfg_attr(feature = "graphics_backend_software", allow(unused))]

use crate::basic_types::AxisDirection;
use crate::client::graphics::{self, GraphicsResources};
use crate::client::{
    ClientPlayStateUpdate, MAX_HEIGHT_I32, MIN_HEIGHT_I32, RawChunk, RawCustomBlockGroup,
    RawSubchunk, SUBCHUNK_AXIS_LEN, SUBCHUNK_AXIS_LEN_I32,
};
#[cfg(feature = "graphics_backend_vulkan")]
use crate::client::{RayTracedQuadInfo, RayTracedQuadPackedFields};
use crate::identifier;
use crate::protocol::chunk::{
    self as protocol_chunk, ChunkSection, ChunkSectionLightChannelInfoMut, LightType,
};
use crate::resource::block::{GlobalPaletteIndex, RightAngleRotation};
use crate::resource::block::blockstate::{self, BlockOpacity, SkyLightOpacity};
use crate::resource::block::model::{ModelType, Tint};
use ahash::{AHashMap, AHashSet, AHasher};
use fixedbitset::FixedBitSet;
use ordered_float::NotNan;
use smallvec::SmallVec;
use std::collections::VecDeque;
use std::hash::Hasher;
use std::sync::{Arc, mpsc};

#[cfg(feature = "graphics_backend_software")]
pub fn process_subchunks(
    _graphics_resources: &GraphicsResources,
    _raw_chunks: &AHashMap<[i32; 2], Arc<RawChunk>>,
    _play_state_update_tx: mpsc::Sender<ClientPlayStateUpdate>,
    _subchunks: &[[i32; 3]],
    _update_id: usize,
) {
    // TODO:
}

#[cfg(not(feature = "graphics_backend_software"))]
pub fn process_subchunks(
    graphics_resources: &GraphicsResources,
    raw_chunks: &AHashMap<[i32; 2], Arc<RawChunk>>,
    play_state_update_tx: mpsc::Sender<ClientPlayStateUpdate>,
    subchunks: &[[i32; 3]],
    update_id: usize,
) {
    let spruce_leaves_registry_index = graphics_resources
        .block_registry
        .get_index_from_identifier(&identifier!("minecraft:spruce_leaves"))
        .unwrap();
    let mut new_raw_subchunks: Vec<([i32; 3], RawSubchunk)> = Vec::new();
    'subchunk_loop: for &subchunk_coords in subchunks {
        let [subchunk_x, subchunk_y, subchunk_z] = subchunk_coords;
        let Some(chunk) = &raw_chunks.get(&[subchunk_x, subchunk_z]) else {
            continue;
        };
        let chunk_section = &chunk.sections[usize::try_from(subchunk_y).unwrap()];
        if chunk_section.block_count == 0 {
            continue;
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
                    continue 'subchunk_loop;
                }
            }
        }
        let mut block_faces: [Vec<_>; 6] = Default::default();
        let mut tinted_block_faces: [Vec<_>; 6] = Default::default();
        let mut custom_block_instance_groups = AHashMap::new();
        // NOTE: RADIANCE CASCADES
        #[cfg(feature = "graphics_backend_vulkan")]
        let (
            mut vertex_positions,
            mut vertex_position_index_map,
            mut block_face_triangle_quads,
            mut block_face_quad_info,
            mut tinted_block_face_triangle_quads,
            mut tinted_block_face_quad_info,
            mut custom_block_face_triangle_quads,
            mut custom_block_face_quad_info,
        ) = (
            <Vec<[f32; 3]>>::new(),
            <AHashMap<[NotNan<f32>; 3], u32>>::new(),
            <Vec<[[u32; 3]; 2]>>::new(),
            <Vec<RayTracedQuadInfo>>::new(),
            <Vec<[[u32; 3]; 2]>>::new(),
            <Vec<RayTracedQuadInfo>>::new(),
            <Vec<[[u32; 3]; 2]>>::new(),
            <Vec<RayTracedQuadInfo>>::new(),
        );
        #[cfg(feature = "graphics_backend_vulkan")]
        fn add_block_quad(
            triangle_quads_list: &mut Vec<[[u32; 3]; 2]>,
            quad_info_list: &mut Vec<RayTracedQuadInfo>,
            vertex_positions: &mut Vec<[f32; 3]>,
            vertex_position_index_map: &mut AHashMap<[NotNan<f32>; 3], u32>,
            global_pos: [f32; 3],
            dir_i: usize,
            atlas_uvs: [u16; 4],
            uv_rotation: crate::resource::block::RightAngleRotation,
            tint_colour: Option<[u8; 4]>,
        ) {
            use crate::resource::block::RightAngleRotation;
            use nalgebra::Vector3;
            assert!(dir_i < 6);
            let global_pos_vec = Vector3::from(global_pos) + Vector3::repeat(0.5);
            let base_positions: [Vector3<f32>; 4] = [
                Vector3::new(0.5, 0.5, 0.5),
                Vector3::new(-0.5, 0.5, 0.5),
                Vector3::new(0.5, 0.5, -0.5),
                Vector3::new(-0.5, 0.5, -0.5),
            ];
            let face_matrix =
                crate::client::graphics::chunk_rc::block_face::face_matrices::rotations()[dir_i];
            let global_positions = base_positions
                .map(|pos| face_matrix.transform_vector(&pos))
                .map(|pos| pos + global_pos_vec)
                .map(<[f32; 3]>::from)
                .map(|pos| pos.map(|n| NotNan::new(n).unwrap()));
            let pos_indices = global_positions.map(|pos_not_nan| {
                *vertex_position_index_map
                    .entry(pos_not_nan)
                    .or_insert_with(|| {
                        let new_index: u32 = vertex_positions.len().try_into().unwrap();
                        vertex_positions.push(pos_not_nan.map(Into::into));
                        new_index
                    })
            });
            triangle_quads_list.push([
                [pos_indices[1], pos_indices[0], pos_indices[2]],
                [pos_indices[3], pos_indices[1], pos_indices[2]],
            ]);
            let base_uvs = [
                [atlas_uvs[2], atlas_uvs[3]],
                [atlas_uvs[0], atlas_uvs[3]],
                [atlas_uvs[2], atlas_uvs[1]],
                [atlas_uvs[0], atlas_uvs[1]],
            ];
            let uv_rotation_arr = match uv_rotation {
                RightAngleRotation::Zero => [0, 1, 2, 3],
                // RightAngleRotation::Ninety => [1, 3, 0, 2],
                RightAngleRotation::Ninety => [2, 0, 3, 1],
                RightAngleRotation::OneEighty => [3, 2, 1, 0],
                // RightAngleRotation::TwoSeventy => [2, 0, 3, 1],
                RightAngleRotation::TwoSeventy => [1, 3, 0, 2],
            };
            let vertex_uvs = [
                base_uvs[uv_rotation_arr[0]],
                base_uvs[uv_rotation_arr[1]],
                base_uvs[uv_rotation_arr[2]],
                base_uvs[uv_rotation_arr[3]],
            ];
            let mut quad_fields = RayTracedQuadPackedFields(0);
            if let Some(tint) = tint_colour {
                quad_fields.set_tint_colour(
                    tint[0] as u32 | ((tint[1] as u32) << 8) | ((tint[2] as u32) << 16),
                );
            } else {
                quad_fields.set_tint_colour(0xFFFFFF);
            }
            quad_info_list.push(RayTracedQuadInfo {
                uvs: vertex_uvs,
                packed_fields: quad_fields,
            });
        }
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
                    let blockstate_info = &graphics_resources.block_registry[global_palette_index];
                    let model = match &blockstate_info.model_data {
                        blockstate::ModelData::Single(model) => model,
                        blockstate::ModelData::RandomChoice(models) => 'model_blk: {
                            // Find weight for model by hashed position
                            let mut block_hasher = AHasher::default();
                            block_hasher.write_i32(global_x_i32);
                            block_hasher.write_i32(global_y_i32);
                            block_hasher.write_i32(global_z_i32);
                            let hash = block_hasher.finish();
                            let mut current_percentage = (hash % 65537) as f32 / 65536.0;
                            for model in models.iter() {
                                if current_percentage <= model.weight {
                                    break 'model_blk &model.model;
                                } else {
                                    current_percentage -= model.weight;
                                }
                            }
                            // Should be unreachable
                            let model = &models[models.len() - 1];
                            &model.model
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
                        let check_global_y =
                            (SUBCHUNK_AXIS_LEN_I32 * subchunk_y + y) + MIN_HEIGHT_I32;
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
                        let neighbour_blockstate_info =
                            &graphics_resources.block_registry[global_palette_index];
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
                    match model.as_ref() {
                        ModelType::None => continue,
                        ModelType::Block(info) => {
                            match block_opacity {
                                BlockOpacity::Opaque => {
                                    for i in 0..6 {
                                        if face_cull_map[i] {
                                            continue;
                                        }
                                        block_faces[i].push(
                                            graphics::chunk_rc::block_face::Instance::new(
                                                [x as u8, y as u8, z as u8],
                                                info.per_face_atlas_uvs[i],
                                                match info.per_face_uv_rotations[i] {
                                                    RightAngleRotation::Zero => 0,
                                                    RightAngleRotation::Ninety => 1,
                                                    RightAngleRotation::OneEighty => 2,
                                                    RightAngleRotation::TwoSeventy => 3,
                                                },
                                                face_light_map[i],
                                                blockstate_info
                                                    .extra_info
                                                    .light_info
                                                    .emission_level
                                                    > 0,
                                            ),
                                        );
                                        #[cfg(feature = "graphics_backend_vulkan")]
                                        add_block_quad(
                                            &mut block_face_triangle_quads,
                                            &mut block_face_quad_info,
                                            &mut vertex_positions,
                                            &mut vertex_position_index_map,
                                            [global_x, global_y, global_z],
                                            i,
                                            info.per_face_atlas_uvs[i],
                                            info.per_face_uv_rotations[i],
                                            None,
                                        );
                                    }
                                }
                                _ => {
                                    for i in 0..6 {
                                        if face_cull_map[i] {
                                            continue;
                                        }
                                        tinted_block_faces[i].push(
                                            graphics::chunk_rc::tinted_block_face::Instance::new(
                                                [x as u8, y as u8, z as u8],
                                                info.per_face_atlas_uvs[i],
                                                match info.per_face_uv_rotations[i] {
                                                    RightAngleRotation::Zero => 0,
                                                    RightAngleRotation::Ninety => 1,
                                                    RightAngleRotation::OneEighty => 2,
                                                    RightAngleRotation::TwoSeventy => 3,
                                                },
                                                face_light_map[i],
                                                // Block doesn't have any tint, so just use
                                                // transparent white as a null value.
                                                [0xFF, 0xFF, 0xFF, 0x00],
                                                blockstate_info
                                                    .extra_info
                                                    .light_info
                                                    .emission_level
                                                    > 0,
                                            ),
                                        );
                                        #[cfg(feature = "graphics_backend_vulkan")]
                                        add_block_quad(
                                            &mut tinted_block_face_triangle_quads,
                                            &mut tinted_block_face_quad_info,
                                            &mut vertex_positions,
                                            &mut vertex_position_index_map,
                                            [global_x, global_y, global_z],
                                            i,
                                            info.per_face_atlas_uvs[i],
                                            info.per_face_uv_rotations[i],
                                            None,
                                        );
                                    }
                                }
                            }
                        }
                        ModelType::TintedBlock(info) => {
                            for i in 0..6 {
                                if face_cull_map[i] {
                                    continue;
                                }
                                tinted_block_faces[i].push(
                                    graphics::chunk_rc::tinted_block_face::Instance::new(
                                        [x as u8, y as u8, z as u8],
                                        info.per_face_atlas_uvs[i],
                                        match info.per_face_uv_rotations[i] {
                                            RightAngleRotation::Zero => 0,
                                            RightAngleRotation::Ninety => 1,
                                            RightAngleRotation::OneEighty => 2,
                                            RightAngleRotation::TwoSeventy => 3,
                                        },
                                        face_light_map[i],
                                        tint_color,
                                        blockstate_info.extra_info.light_info.emission_level > 0,
                                    ),
                                );
                                #[cfg(feature = "graphics_backend_vulkan")]
                                add_block_quad(
                                    &mut tinted_block_face_triangle_quads,
                                    &mut tinted_block_face_quad_info,
                                    &mut vertex_positions,
                                    &mut vertex_position_index_map,
                                    [global_x, global_y, global_z],
                                    i,
                                    info.per_face_atlas_uvs[i],
                                    info.per_face_uv_rotations[i],
                                    Some(tint_color),
                                );
                            }
                        }
                        ModelType::OverlayedBlock(info) => {
                            for face in &info.faces {
                                if face_cull_map[face.face_i as usize] {
                                    continue;
                                }
                                if let Some(tint) = face.tint {
                                    assert!(tint == Tint::Biome, "TODO: Alternative tints");
                                    tinted_block_faces[face.face_i as usize].push(
                                        graphics::chunk_rc::tinted_block_face::Instance::new(
                                            [x as u8, y as u8, z as u8],
                                            face.atlas_uvs,
                                            match face.uv_rotation {
                                                RightAngleRotation::Zero => 0,
                                                RightAngleRotation::Ninety => 1,
                                                RightAngleRotation::OneEighty => 2,
                                                RightAngleRotation::TwoSeventy => 3,
                                            },
                                            face_light_map[face.face_i as usize],
                                            tint_color,
                                            blockstate_info.extra_info.light_info.emission_level
                                                > 0,
                                        ),
                                    );
                                    #[cfg(feature = "graphics_backend_vulkan")]
                                    add_block_quad(
                                        &mut tinted_block_face_triangle_quads,
                                        &mut tinted_block_face_quad_info,
                                        &mut vertex_positions,
                                        &mut vertex_position_index_map,
                                        [global_x, global_y, global_z],
                                        face.face_i as usize,
                                        face.atlas_uvs,
                                        face.uv_rotation,
                                        Some(tint_color),
                                    );
                                } else {
                                    block_faces[face.face_i as usize].push(
                                        graphics::chunk_rc::block_face::Instance::new(
                                            [x as u8, y as u8, z as u8],
                                            face.atlas_uvs,
                                            match face.uv_rotation {
                                                RightAngleRotation::Zero => 0,
                                                RightAngleRotation::Ninety => 1,
                                                RightAngleRotation::OneEighty => 2,
                                                RightAngleRotation::TwoSeventy => 3,
                                            },
                                            face_light_map[face.face_i as usize],
                                            blockstate_info.extra_info.light_info.emission_level
                                                > 0,
                                        ),
                                    );
                                    #[cfg(feature = "graphics_backend_vulkan")]
                                    add_block_quad(
                                        &mut block_face_triangle_quads,
                                        &mut block_face_quad_info,
                                        &mut vertex_positions,
                                        &mut vertex_position_index_map,
                                        [global_x, global_y, global_z],
                                        face.face_i as usize,
                                        face.atlas_uvs,
                                        face.uv_rotation,
                                        None,
                                    );
                                }
                            }
                        }
                        ModelType::Other(info) => {
                            // TODO:
                            // - Add a `face_opacity_map`
                            // - For each neighbour, if it's opaque, replace with centre block
                            //   light level
                            let light_section = chunk
                                .lighting
                                .get_section(MIN_HEIGHT_I32, subchunk_y + (MIN_HEIGHT_I32 / 16))
                                .unwrap();
                            let block_instances = custom_block_instance_groups
                                .entry(info)
                                .or_insert_with(Vec::new);
                            block_instances.push(graphics::chunk_rc::custom_block::Instance::new(
                                [global_x, global_y, global_z],
                                tint_color,
                                light_section.get(x, y, z),
                                face_light_map,
                                blockstate_info.extra_info.light_info.emission_level > 0,
                            ));
                            #[cfg(feature = "graphics_backend_vulkan")]
                            {
                                use nalgebra::Vector3;
                                let block_global_pos_vec =
                                    Vector3::new(global_x, global_y, global_z)
                                        + Vector3::repeat(0.5);
                                for local_index_i in (0..info.indices.len()).step_by(6) {
                                    // Refer to FACE_INDICES in
                                    // crate::resource::model::finalise_model.
                                    // Should give indices [0, 1, 2, 3] + base.
                                    let quad_model_vertices = [
                                        &info.vertices[info.indices[local_index_i + 1] as usize],
                                        &info.vertices[info.indices[local_index_i + 3] as usize],
                                        &info.vertices[info.indices[local_index_i + 4] as usize],
                                        &info.vertices[info.indices[local_index_i + 5] as usize],
                                    ];
                                    let quad_indices = quad_model_vertices.map(|v| {
                                        let global_pos = v.local_pos + block_global_pos_vec;
                                        let global_pos_f32s = <[f32; 3]>::from(global_pos);
                                        let global_pos_not_nan =
                                            global_pos_f32s.map(|n| NotNan::new(n).unwrap());
                                        *vertex_position_index_map
                                            .entry(global_pos_not_nan)
                                            .or_insert_with(|| {
                                                let new_index: u32 =
                                                    vertex_positions.len().try_into().unwrap();
                                                vertex_positions.push(global_pos_f32s);
                                                new_index
                                            })
                                    });
                                    custom_block_face_triangle_quads.push([
                                        [quad_indices[1], quad_indices[0], quad_indices[2]],
                                        [quad_indices[3], quad_indices[1], quad_indices[2]],
                                    ]);
                                    let quad_uvs = quad_model_vertices.map(|v| v.uvs);
                                    let mut quad_fields = RayTracedQuadPackedFields(0);
                                    if quad_model_vertices[0].tint.is_some() {
                                        quad_fields.set_tint_colour(
                                            tint_color[0] as u32
                                                | ((tint_color[1] as u32) << 8)
                                                | ((tint_color[2] as u32) << 16),
                                        );
                                    } else {
                                        quad_fields.set_tint_colour(0xFFFFFF);
                                    }
                                    custom_block_face_quad_info.push(RayTracedQuadInfo {
                                        uvs: quad_uvs,
                                        packed_fields: quad_fields,
                                    });
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        // Runs a variant of Minecraft's cave culling algorithm, specifically the connected
        // face generation.
        // Outlined here: https://tomcc.github.io/2014/08/31/visibility-1.html
        let connected_faces = 'connected_faces: {
            use graphics::chunk_rc::SubchunkConnectivity;
            use protocol_chunk::Palette;
            // If we can immediately tell all the subchunk blocks are opaque, skip this entire
            // process and just return that no subchunk faces are connected.
            match chunk_section.block_states.palette() {
                Palette::SingleValue(global_palette_index) => {
                    let blockstate_info = &graphics_resources.block_registry[*global_palette_index];
                    break 'connected_faces match blockstate_info.extra_info.opacity {
                        BlockOpacity::Opaque => SubchunkConnectivity::empty(),
                        _ => SubchunkConnectivity::full(),
                    };
                }
                Palette::Palette(indices) => {
                    let mut num_opaque = 0;
                    for global_palette_index in indices {
                        let blockstate_info =
                            &graphics_resources.block_registry[*global_palette_index];
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
                        let blockstate_info =
                            &graphics_resources.block_registry[global_palette_index];
                        if blockstate_info.extra_info.opacity != BlockOpacity::Opaque {
                            let bit_index = coords_to_bit_idx([x, y, z].map(|n| n as i8));
                            unchecked_blocks.insert(bit_index);
                        }
                    }
                }
            }
            // Flood fill from each non-opaque block, to split all the blocks into groups.
            let mut queue: AHashSet<[i8; 3]> = AHashSet::new();
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
        // Block faces
        let mut block_face_quads: [Option<_>; 6] = [None; 6];
        let block_face_instance_groups: [Vec<_>; 6] = block_faces;
        for i in 0..6 {
            if block_face_instance_groups[i].is_empty() {
                continue;
            }
            let base_quad =
                graphics::chunk_rc::block_face::Vertex::generate_base_quad(start_coords, i);
            block_face_quads[i] = Some(base_quad);
        }
        // Tinted block faces
        let mut tinted_block_face_quads: [Option<_>; 6] = [None; 6];
        let tinted_block_face_instance_groups: [Vec<_>; 6] = tinted_block_faces;
        for i in 0..6 {
            if tinted_block_face_instance_groups[i].is_empty() {
                continue;
            }
            let base_quad =
                graphics::chunk_rc::tinted_block_face::Vertex::generate_base_quad(start_coords, i);
            tinted_block_face_quads[i] = Some(base_quad);
        }
        // Custom block groups
        let custom_block_groups = custom_block_instance_groups
            .into_iter()
            .map(|(info, instances)| RawCustomBlockGroup {
                start_vertex: info.start_vertex,
                start_index_and_len: info.start_index_and_len,
                instances,
            })
            .collect();
        #[cfg(feature = "graphics_backend_vulkan")]
        let (quads_info, quads_info_offsets) = {
            let tinted_offset: u32 = block_face_quad_info.len().try_into().unwrap();
            block_face_quad_info.extend(tinted_block_face_quad_info);
            let custom_offset: u32 = block_face_quad_info.len().try_into().unwrap();
            block_face_quad_info.extend(custom_block_face_quad_info);
            (block_face_quad_info, [tinted_offset, custom_offset])
        };
        new_raw_subchunks.push((
            subchunk_coords,
            RawSubchunk {
                start_coords,
                block_face_quads,
                block_face_instance_groups,
                tinted_block_face_quads,
                tinted_block_face_instance_groups,
                custom_block_groups,
                connected_faces,
                #[cfg(feature = "graphics_backend_vulkan")]
                rt_info: crate::client::RayTracingInfo {
                    vertex_positions,
                    block_face_triangle_quads,
                    tinted_block_face_triangle_quads,
                    custom_block_face_triangle_quads,
                    quads_info,
                    quads_info_offsets,
                },
            },
        ));
    }
    play_state_update_tx
        .send(ClientPlayStateUpdate::PlaceSubchunks {
            update_id,
            new_raw_subchunks,
        })
        .unwrap();
}

pub fn recalculate_light(
    graphics_resources: &GraphicsResources,
    raw_chunks: &mut AHashMap<[i32; 2], Arc<RawChunk>>,
    subchunks_to_update: &mut AHashSet<[i32; 3]>,
    pos: [i32; 3],
    old_block_id: GlobalPaletteIndex,
    new_block_id: GlobalPaletteIndex,
) {
    let old_blockstate_info = &graphics_resources.block_registry[old_block_id];
    let old_extra_info = &old_blockstate_info.extra_info;
    let new_blockstate_info = &graphics_resources.block_registry[new_block_id];
    let new_extra_info = &new_blockstate_info.extra_info;
    if old_extra_info == new_extra_info {
        return;
    }
    let mut new_block_level;
    if old_extra_info.light_info.emission_level == new_extra_info.light_info.emission_level {
        match (old_extra_info.opacity, new_extra_info.opacity) {
            // If we've kept the same emission level, but either changed to or stayed opaque, set
            // to the emission level.
            (_, BlockOpacity::Opaque) => {
                new_block_level = new_extra_info.light_info.emission_level;
            }
            // If we've kept the same emission level, but changed to transparent, then set to the
            // max light level it could be.
            (BlockOpacity::Opaque, _) => {
                new_block_level = new_extra_info.light_info.emission_level;
                for (neighbour, _dir) in neighbours(pos) {
                    let (neighbour_block_level, _, _) =
                        get_block_light_level_and_info(graphics_resources, raw_chunks, neighbour);
                    let target_level = neighbour_block_level.saturating_sub(1);
                    new_block_level = u8::max(new_block_level, target_level);
                }
            }
            // Same as the case for both opaque.
            (_, _) => new_block_level = new_extra_info.light_info.emission_level,
        }
    } else {
        // If we've changed emission levels, then just do propagation with the new emission level.
        new_block_level = new_extra_info.light_info.emission_level;
    }
    let mut new_sky_level;
    match new_extra_info.light_info.sky_light_opacity {
        // If we're now opaque, set to zero.
        SkyLightOpacity::Opaque => new_sky_level = 0,
        // If we're now translucent, then set to the max light level it could be.
        SkyLightOpacity::Translucent => {
            new_sky_level = 0;
            for (neighbour, _dir) in neighbours(pos) {
                let (neighbour_sky_level, _) =
                    get_sky_light_level_and_opacity(graphics_resources, raw_chunks, neighbour);
                let target_level = neighbour_sky_level.saturating_sub(1);
                new_sky_level = u8::max(new_sky_level, target_level);
            }
        }
        // If we're now transparent, then set to the max light level it could be, propagating max
        // level sky light downwards unaffected.
        SkyLightOpacity::Transparent => {
            new_sky_level = 0;
            for (neighbour, dir) in neighbours(pos) {
                let (neighbour_sky_level, _) =
                    get_sky_light_level_and_opacity(graphics_resources, raw_chunks, neighbour);
                let target_level = if dir == AxisDirection::Up && neighbour_sky_level == 15 {
                    15
                } else {
                    neighbour_sky_level.saturating_sub(1)
                };
                new_sky_level = u8::max(new_sky_level, target_level);
            }
        }
    }
    update_light_and_propagate(
        graphics_resources,
        raw_chunks,
        subchunks_to_update,
        pos,
        [new_block_level, new_sky_level],
    );
}

fn update_light_and_propagate(
    graphics_resources: &GraphicsResources,
    raw_chunks: &mut AHashMap<[i32; 2], Arc<RawChunk>>,
    subchunks_to_update: &mut AHashSet<[i32; 3]>,
    pos: [i32; 3],
    new_light_levels: [u8; 2],
) {
    assert!(new_light_levels[0] < 16);
    assert!(new_light_levels[1] < 16);
    // Update block light levels
    'block_light: {
        // Compare current light level to new light level, to pick increase or decrease.
        // If increase:
        // - Set the block's light level
        // - `target_level = <block light level> - 1`
        // - Add each neighbour to queue that has a light level below target
        // - Repeat until queue is empty
        // If decrease:
        // - Set the block's light level
        // - `target_level = <block light level> - 1`
        // - Set each neighbour's light level to 0 if it's below or equal to target, and add to
        //   decrease queue
        // - Add each neighbour to increase queue that has a light level above target
        // - Repeat until decrease queue is empty
        // - Run increase steps, as detailed above
        let new_level = new_light_levels[0];
        let mut increase_queue: VecDeque<([i32; 3], u8, Option<AxisDirection>)> = VecDeque::new();
        let (old_level, _, _) = get_block_light_level_and_info(graphics_resources, raw_chunks, pos);
        set_light_level(
            raw_chunks,
            subchunks_to_update,
            pos,
            LightType::Block,
            new_level,
        );
        match u8::cmp(&new_level, &old_level) {
            std::cmp::Ordering::Less => {
                let mut decrease_queue: VecDeque<([i32; 3], u8, Option<AxisDirection>)> =
                    VecDeque::new();
                decrease_queue.push_back((pos, old_level, None));
                // Propagate decreases
                while let Some((pos, level, from_dir)) = decrease_queue.pop_front() {
                    for (neighbour, dir) in neighbours(pos) {
                        // Small optimisation, don't bother checking block we just came from
                        if Some(dir) == from_dir {
                            continue;
                        }
                        let (neighbour_light_level, neighbour_opacity, neighbour_emission_level) =
                            get_block_light_level_and_info(
                                graphics_resources,
                                raw_chunks,
                                neighbour,
                            );
                        if neighbour_light_level == 0 {
                            continue;
                        }
                        let target_level = match neighbour_opacity {
                            BlockOpacity::Opaque => 0,
                            _ => level.saturating_sub(1),
                        };
                        if neighbour_light_level <= target_level {
                            decrease_queue.push_back((
                                neighbour,
                                neighbour_light_level,
                                Some(dir.invert()),
                            ));
                            // If we find a dim source while decreasing from a bright source, make
                            // sure to repropagate its dim light.
                            if neighbour_emission_level > 0 {
                                set_light_level(
                                    raw_chunks,
                                    subchunks_to_update,
                                    neighbour,
                                    LightType::Block,
                                    neighbour_emission_level,
                                );
                                increase_queue.push_back((
                                    neighbour,
                                    neighbour_emission_level,
                                    None,
                                ));
                            } else {
                                set_light_level(
                                    raw_chunks,
                                    subchunks_to_update,
                                    neighbour,
                                    LightType::Block,
                                    0,
                                );
                            }
                        } else {
                            increase_queue.push_back((neighbour, neighbour_light_level, None));
                        }
                    }
                }
                // If we've switched from a bright source to a dimmer source, make sure to
                // repropagate its new, dimmer light.
                if new_level > 0 {
                    increase_queue.push_back((pos, new_level, None));
                }
            }
            std::cmp::Ordering::Equal => break 'block_light,
            std::cmp::Ordering::Greater => increase_queue.push_back((pos, new_level, None)),
        }
        // Propagate increases
        while let Some((pos, new_level, from_dir)) = increase_queue.pop_front() {
            for (neighbour, dir) in neighbours(pos) {
                if Some(dir) == from_dir {
                    continue;
                }
                let (neighbour_light_level, neighbour_opacity, _) =
                    get_block_light_level_and_info(graphics_resources, raw_chunks, neighbour);
                let target_level = match neighbour_opacity {
                    BlockOpacity::Opaque => 0,
                    _ => new_level.saturating_sub(1),
                };
                if neighbour_light_level < target_level {
                    set_light_level(
                        raw_chunks,
                        subchunks_to_update,
                        neighbour,
                        LightType::Block,
                        target_level,
                    );
                    increase_queue.push_back((neighbour, target_level, Some(dir.invert())));
                }
            }
        }
    }
    // Update sky light levels
    'sky_light: {
        // Same process as for block lighting, but full (15) light passes down through transparent
        // blocks without decreasing.
        let new_level = new_light_levels[1];
        let mut increase_queue: VecDeque<([i32; 3], u8, Option<AxisDirection>)> = VecDeque::new();
        let (old_level, _) = get_sky_light_level_and_opacity(graphics_resources, raw_chunks, pos);
        set_light_level(
            raw_chunks,
            subchunks_to_update,
            pos,
            LightType::Sky,
            new_level,
        );
        match u8::cmp(&new_level, &old_level) {
            std::cmp::Ordering::Less => {
                let mut decrease_queue: VecDeque<([i32; 3], u8, Option<AxisDirection>)> =
                    VecDeque::new();
                decrease_queue.push_back((pos, old_level, None));
                // Propagate decreases
                while let Some((pos, level, from_dir)) = decrease_queue.pop_front() {
                    for (neighbour, dir) in neighbours(pos) {
                        // Small optimisation, don't bother checking block we just came from
                        if Some(dir) == from_dir {
                            continue;
                        }
                        let (neighbour_light_level, neighbour_sky_opacity) =
                            get_sky_light_level_and_opacity(
                                graphics_resources,
                                raw_chunks,
                                neighbour,
                            );
                        if neighbour_light_level == 0 {
                            continue;
                        }
                        use AxisDirection::*;
                        let target_level = match neighbour_sky_opacity {
                            SkyLightOpacity::Opaque => 0,
                            SkyLightOpacity::Transparent if dir == Down && level == 15 => 15,
                            _ => level.saturating_sub(1),
                        };
                        if neighbour_light_level <= target_level {
                            set_light_level(
                                raw_chunks,
                                subchunks_to_update,
                                neighbour,
                                LightType::Sky,
                                0,
                            );
                            decrease_queue.push_back((
                                neighbour,
                                neighbour_light_level,
                                Some(dir.invert()),
                            ));
                        } else {
                            increase_queue.push_back((neighbour, neighbour_light_level, None));
                        }
                    }
                }
            }
            std::cmp::Ordering::Equal => break 'sky_light,
            std::cmp::Ordering::Greater => increase_queue.push_back((pos, new_level, None)),
        }
        // Propagate increases
        while let Some((pos, new_level, from_dir)) = increase_queue.pop_front() {
            for (neighbour, dir) in neighbours(pos) {
                if Some(dir) == from_dir {
                    continue;
                }
                let (neighbour_light_level, neighbour_sky_opacity) =
                    get_sky_light_level_and_opacity(graphics_resources, raw_chunks, neighbour);
                use AxisDirection::*;
                let target_level = match neighbour_sky_opacity {
                    SkyLightOpacity::Opaque => 0,
                    SkyLightOpacity::Transparent if dir == Down && new_level == 15 => 15,
                    _ => new_level - 1,
                };
                if neighbour_light_level < target_level {
                    set_light_level(
                        raw_chunks,
                        subchunks_to_update,
                        neighbour,
                        LightType::Sky,
                        target_level,
                    );
                    increase_queue.push_back((neighbour, target_level, Some(dir.invert())));
                }
            }
        }
    }
}

#[inline]
fn get_section_info_and_inner_pos(
    raw_chunks: &mut AHashMap<[i32; 2], Arc<RawChunk>>,
    global_pos: [i32; 3],
    channel: LightType,
) -> Option<(
    &mut ChunkSection,
    ChunkSectionLightChannelInfoMut,
    [usize; 3],
)> {
    let chunk_x = global_pos[0].div_euclid(SUBCHUNK_AXIS_LEN_I32);
    let chunk_z = global_pos[2].div_euclid(SUBCHUNK_AXIS_LEN_I32);
    let section_i: usize = (global_pos[1] - MIN_HEIGHT_I32)
        .div_euclid(SUBCHUNK_AXIS_LEN_I32)
        .try_into()
        .unwrap();
    let chunk = raw_chunks.get_mut(&[chunk_x, chunk_z])?;
    let chunk_mut = Arc::make_mut(chunk);
    let chunk_section = &mut chunk_mut.sections[section_i];
    let light_section = chunk_mut.lighting.get_section_channel_mut(
        MIN_HEIGHT_I32,
        global_pos[1].div_euclid(SUBCHUNK_AXIS_LEN_I32),
        channel,
    )?;
    let x = global_pos[0].rem_euclid(SUBCHUNK_AXIS_LEN_I32);
    let x_usize: usize = x.try_into().unwrap();
    let y = global_pos[1].rem_euclid(SUBCHUNK_AXIS_LEN_I32);
    let y_usize: usize = y.try_into().unwrap();
    let z = global_pos[2].rem_euclid(SUBCHUNK_AXIS_LEN_I32);
    let z_usize: usize = z.try_into().unwrap();
    Some((chunk_section, light_section, [x_usize, y_usize, z_usize]))
}

#[inline]
/// Returns the block light, opacity, and emission level.
fn get_block_light_level_and_info(
    graphics_resources: &GraphicsResources,
    raw_chunks: &mut AHashMap<[i32; 2], Arc<RawChunk>>,
    global_pos: [i32; 3],
) -> (u8, BlockOpacity, u8) {
    match get_section_info_and_inner_pos(raw_chunks, global_pos, LightType::Block) {
        None => (0, BlockOpacity::Opaque, 0),
        Some((chunk_section, light_section, [x, y, z])) => {
            let light_level = light_section.get(x, y, z);
            let global_palette_index = chunk_section.block_states.get(x, y, z);
            let blockstate_info = &graphics_resources.block_registry[global_palette_index];
            let extra_info = &blockstate_info.extra_info;
            (
                light_level,
                extra_info.opacity,
                extra_info.light_info.emission_level,
            )
        }
    }
}

#[inline]
fn get_sky_light_level_and_opacity(
    graphics_resources: &GraphicsResources,
    raw_chunks: &mut AHashMap<[i32; 2], Arc<RawChunk>>,
    global_pos: [i32; 3],
) -> (u8, SkyLightOpacity) {
    match get_section_info_and_inner_pos(raw_chunks, global_pos, LightType::Sky) {
        None => (0, SkyLightOpacity::Opaque),
        Some((chunk_section, light_section, [x, y, z])) => {
            let light_level = light_section.get(x, y, z);
            let global_palette_index = chunk_section.block_states.get(x, y, z);
            let blockstate_info = &graphics_resources.block_registry[global_palette_index];
            let extra_info = &blockstate_info.extra_info;
            (light_level, extra_info.light_info.sky_light_opacity)
        }
    }
}

#[inline]
fn set_light_level(
    raw_chunks: &mut AHashMap<[i32; 2], Arc<RawChunk>>,
    subchunks_to_update: &mut AHashSet<[i32; 3]>,
    global_pos: [i32; 3],
    channel: LightType,
    new_level: u8,
) {
    let Some((_chunk_section, mut light_section, [x, y, z])) =
        get_section_info_and_inner_pos(raw_chunks, global_pos, channel)
    else {
        return;
    };
    let chunk_x = global_pos[0].div_euclid(SUBCHUNK_AXIS_LEN_I32);
    let chunk_z = global_pos[2].div_euclid(SUBCHUNK_AXIS_LEN_I32);
    let section_i = (global_pos[1] - MIN_HEIGHT_I32).div_euclid(SUBCHUNK_AXIS_LEN_I32);
    let subchunk_y = section_i;
    subchunks_to_update.insert([chunk_x, subchunk_y, chunk_z]);
    light_section.set(x, y, z, new_level)
}

/// Is allowed to return neighbours one block above max height and one below min height.
fn neighbours(pos: [i32; 3]) -> SmallVec<[([i32; 3], AxisDirection); 6]> {
    let unfiltered_neighbours = [
        ([pos[0] - 1, pos[1], pos[2]], AxisDirection::West),
        ([pos[0] + 1, pos[1], pos[2]], AxisDirection::East),
        ([pos[0], pos[1] - 1, pos[2]], AxisDirection::Down),
        ([pos[0], pos[1] + 1, pos[2]], AxisDirection::Up),
        ([pos[0], pos[1], pos[2] - 1], AxisDirection::North),
        ([pos[0], pos[1], pos[2] + 1], AxisDirection::South),
    ];
    let mut out = SmallVec::new();
    for ([x, y, z], dir) in unfiltered_neighbours {
        if ((MIN_HEIGHT_I32 - 1)..=MAX_HEIGHT_I32).contains(&y) {
            out.push(([x, y, z], dir));
        }
    }
    out
}
