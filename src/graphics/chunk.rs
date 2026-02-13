use crate::basic_types::AxisDirection;
use crate::{MAX_HEIGHT_I32, MIN_HEIGHT_I32, SUBCHUNK_AXIS_LEN, SUBCHUNK_AXIS_LEN_I32};
use ahash::AHasher;
use core::hash::Hasher;
use fixedbitset::FixedBitSet;
use portable_std::{Arc, FastHashMap};
use resources::block::blockstate::BlockOpacity;
use resources::identifier;

pub trait HasSubchunkData {
    fn get_data(&self) -> SubchunkData;
}

#[derive(Clone, Copy, Debug)]
pub struct SubchunkData {
    pub start_coords: [i32; 3],
    pub connectivity: SubchunkConnectivity,
}

// Bits (least to most significant) store if each of these pairs of faces are connected:
// 0: Down-Up
// 1: Down-North
// 2: Down-South
// 3: Down-West
// 4: Down-East
// 5: Up-North
// 6: Up-South
// 7: Up-West
// 8: Up-East
// 9: North-South
// 10: North-West
// 11: North-East
// 12: South-West
// 13: South-East
// 14: West-East
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubchunkConnectivity(u16);

impl SubchunkConnectivity {
    pub fn empty() -> Self {
        Self(0)
    }

    pub fn full() -> Self {
        Self(0x7FFF)
    }

    #[inline(always)]
    pub fn add_connection(&mut self, face_1: &AxisDirection, face_2: &AxisDirection) {
        use AxisDirection::*;
        match (face_1, face_2) {
            (&Down, &Down) | (&Up, &Up) => {}
            (&North, &North) | (&South, &South) => {}
            (&West, &West) | (&East, &East) => {}
            (&Down, &Up) | (&Up, &Down) => self.0 |= 0x1,
            (&Down, &North) | (&North, &Down) => self.0 |= 0x2,
            (&Down, &South) | (&South, &Down) => self.0 |= 0x4,
            (&Down, &West) | (&West, &Down) => self.0 |= 0x8,
            (&Down, &East) | (&East, &Down) => self.0 |= 0x10,
            (&Up, &North) | (&North, &Up) => self.0 |= 0x20,
            (&Up, &South) | (&South, &Up) => self.0 |= 0x40,
            (&Up, &West) | (&West, &Up) => self.0 |= 0x80,
            (&Up, &East) | (&East, &Up) => self.0 |= 0x100,
            (&North, &South) | (&South, &North) => self.0 |= 0x200,
            (&North, &West) | (&West, &North) => self.0 |= 0x400,
            (&North, &East) | (&East, &North) => self.0 |= 0x800,
            (&South, &West) | (&West, &South) => self.0 |= 0x1000,
            (&South, &East) | (&East, &South) => self.0 |= 0x2000,
            (&West, &East) | (&East, &West) => self.0 |= 0x4000,
        }
    }

    #[inline(always)]
    pub fn connects(&self, face_1: &AxisDirection, face_2: &AxisDirection) -> bool {
        use AxisDirection::*;
        match (face_1, face_2) {
            (&Down, &Down) | (&Up, &Up) => true,
            (&North, &North) | (&South, &South) => true,
            (&West, &West) | (&East, &East) => true,
            (&Down, &Up) | (&Up, &Down) => self.0 & 0x1 != 0,
            (&Down, &North) | (&North, &Down) => self.0 & 0x2 != 0,
            (&Down, &South) | (&South, &Down) => self.0 & 0x4 != 0,
            (&Down, &West) | (&West, &Down) => self.0 & 0x8 != 0,
            (&Down, &East) | (&East, &Down) => self.0 & 0x10 != 0,
            (&Up, &North) | (&North, &Up) => self.0 & 0x20 != 0,
            (&Up, &South) | (&South, &Up) => self.0 & 0x40 != 0,
            (&Up, &West) | (&West, &Up) => self.0 & 0x80 != 0,
            (&Up, &East) | (&East, &Up) => self.0 & 0x100 != 0,
            (&North, &South) | (&South, &North) => self.0 & 0x200 != 0,
            (&North, &West) | (&West, &North) => self.0 & 0x400 != 0,
            (&North, &East) | (&East, &North) => self.0 & 0x800 != 0,
            (&South, &West) | (&West, &South) => self.0 & 0x1000 != 0,
            (&South, &East) | (&East, &South) => self.0 & 0x2000 != 0,
            (&West, &East) | (&East, &West) => self.0 & 0x4000 != 0,
        }
    }

    #[inline(always)]
    pub fn get_pairs(&self) -> [([AxisDirection; 2], bool); 15] {
        let fields = [
            ([AxisDirection::Down, AxisDirection::Up], 0x1),
            ([AxisDirection::Down, AxisDirection::North], 0x2),
            ([AxisDirection::Down, AxisDirection::South], 0x4),
            ([AxisDirection::Down, AxisDirection::West], 0x8),
            ([AxisDirection::Down, AxisDirection::East], 0x10),
            ([AxisDirection::Up, AxisDirection::North], 0x20),
            ([AxisDirection::Up, AxisDirection::South], 0x40),
            ([AxisDirection::Up, AxisDirection::West], 0x80),
            ([AxisDirection::Up, AxisDirection::East], 0x100),
            ([AxisDirection::North, AxisDirection::South], 0x200),
            ([AxisDirection::North, AxisDirection::West], 0x400),
            ([AxisDirection::North, AxisDirection::East], 0x800),
            ([AxisDirection::South, AxisDirection::West], 0x1000),
            ([AxisDirection::South, AxisDirection::East], 0x2000),
            ([AxisDirection::West, AxisDirection::East], 0x4000),
        ];
        fields.map(|(dirs, mask)| (dirs, self.0 & mask != 0))
    }
}

impl core::fmt::Debug for SubchunkConnectivity {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        let mut debug_set = f.debug_set();
        let fields = [
            ("down_up", 0x1),
            ("down_north", 0x2),
            ("down_south", 0x4),
            ("down_west", 0x8),
            ("down_east", 0x10),
            ("up_north", 0x20),
            ("up_south", 0x40),
            ("up_west", 0x80),
            ("up_east", 0x100),
            ("north_south", 0x200),
            ("north_west", 0x400),
            ("north_east", 0x800),
            ("south_west", 0x1000),
            ("south_east", 0x2000),
            ("west_east", 0x4000),
        ];
        for (field_name, field_mask) in fields {
            if self.0 & field_mask != 0 {
                debug_set.entry(&field_name);
            }
        }
        debug_set.finish()
    }
}

pub struct ModelProcessingArgs<'a> {
    pub model_registry: &'a resources::block::model::ModelRegistry,
    pub chunk: &'a crate::RawChunk,
    pub block_opacity: resources::block::blockstate::BlockOpacity,
    pub face_cull_map: [bool; 6],
    pub face_light_map: [[u8; 2]; 6],
    pub tint_color: [u8; 4],
    pub subchunk_xyz: [i32; 3],
    pub global_xyz: [f32; 3],
    pub xyz: [usize; 3],
    pub model_idx: resources::block::model::ModelIndex,
}

/// If this returns `None`, then the subchunk is not visible and should be skipped.
#[tracing::instrument(skip_all)]
#[inline]
pub fn process_subchunk_models(
    block_registry: &resources::block::Registry,
    model_registry: &resources::block::model::ModelRegistry,
    raw_chunks: &FastHashMap<[i32; 2], Arc<crate::RawChunk>>,
    subchunk_coords: [i32; 3],
    mut process_model: impl FnMut(ModelProcessingArgs),
) -> Option<SubchunkConnectivity> {
    let spruce_leaves_registry_index = block_registry
        .get_index_from_identifier(&identifier!("minecraft:spruce_leaves"))
        .unwrap();
    let [subchunk_x, subchunk_y, subchunk_z] = subchunk_coords;
    let Some(chunk) = &raw_chunks.get(&[subchunk_x, subchunk_z]) else {
        return None;
    };
    let chunk_section = &chunk.sections[usize::try_from(subchunk_y).unwrap()];
    if chunk_section.block_count == 0 {
        return None;
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
                return None;
            }
        }
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
                let blockstate_info = &block_registry[global_palette_index];
                let model_idx = match &blockstate_info.model_data {
                    resources::block::blockstate::ModelData::Single(model_idx) => *model_idx,
                    resources::block::blockstate::ModelData::RandomChoice(models) => 'model_blk: {
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
                process_model(ModelProcessingArgs {
                    model_registry,
                    chunk,
                    block_opacity,
                    face_cull_map,
                    face_light_map,
                    tint_color,
                    subchunk_xyz: [subchunk_x, subchunk_y, subchunk_z],
                    global_xyz: [global_x, global_y, global_z],
                    xyz: [x, y, z],
                    model_idx,
                });
            }
        }
    }
    // Runs a variant of Minecraft's cave culling algorithm, specifically the connected
    // face generation.
    // Outlined here: <https://tomcc.github.io/2014/08/31/visibility-1.html>
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
        #[inline(always)]
        fn coords_to_bit_idx(coords: [i8; 3]) -> usize {
            let [x, y, z] = coords.map(|n| n as usize);
            y * SUBCHUNK_AXIS_LEN.pow(2) + z * SUBCHUNK_AXIS_LEN + x
        }
        #[inline(always)]
        fn bit_idx_to_coords(bit_idx: usize) -> [i8; 3] {
            [
                (bit_idx & 0xF) as i8,
                ((bit_idx >> 8) & 0xF) as i8,
                ((bit_idx >> 4) & 0xF) as i8,
            ]
        }
        // Add all non-opaque blocks.
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
        let mut queued_blocks = FixedBitSet::with_capacity(SUBCHUNK_AXIS_LEN.pow(3));
        while !queued_blocks.is_clear() || !unchecked_blocks.is_clear() {
            let [x, y, z] = queued_blocks
                .minimum()
                .map(|bit_idx| {
                    queued_blocks.remove(bit_idx);
                    bit_idx_to_coords(bit_idx)
                })
                .unwrap_or_else(|| {
                    // No more blocks in queue, make a new group and grab a new block
                    // that hasn't been checked yet.
                    let coord = { bit_idx_to_coords(unchecked_blocks.minimum().unwrap()) };
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
                // If fill escapes subchunk, add escaping face to group.
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
                    queued_blocks.insert(coords_to_bit_idx(new_coord));
                }
            }
        }
        group_faces.push(current_group_faces);
        // Add connected faces for each group to subchunk connectivity.
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
    Some(connectivity)
}
