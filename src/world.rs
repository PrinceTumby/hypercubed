use portable_std::{Arc, FastHashMap, FastHashSet, VecDeque};
use hypercubed_core::types::AxisDirection;
use resources::block::GlobalPaletteIndex;
use resources::block::blockstate::{BlockOpacity, SkyLightOpacity};
use smallvec::SmallVec;

use crate::portable_prelude::*;
use crate::protocol::chunk::{ChunkSection, ChunkSectionLightChannelInfoMut, LightType};
use crate::{MAX_HEIGHT_I32, MIN_HEIGHT_I32, RawChunk, SUBCHUNK_AXIS_LEN_I32};

pub fn recalculate_light(
    block_registry: &resources::block::Registry,
    raw_chunks: &mut FastHashMap<[i32; 2], Arc<RawChunk>>,
    subchunks_to_update: &mut FastHashSet<[i32; 3]>,
    pos: [i32; 3],
    old_block_id: GlobalPaletteIndex,
    new_block_id: GlobalPaletteIndex,
) {
    let old_blockstate_info = &block_registry[old_block_id];
    let old_extra_info = &old_blockstate_info.extra_info;
    let new_blockstate_info = &block_registry[new_block_id];
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
                        get_block_light_level_and_info(block_registry, raw_chunks, neighbour);
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
                    get_sky_light_level_and_opacity(block_registry, raw_chunks, neighbour);
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
                    get_sky_light_level_and_opacity(block_registry, raw_chunks, neighbour);
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
        block_registry,
        raw_chunks,
        subchunks_to_update,
        pos,
        [new_block_level, new_sky_level],
    );
}

fn update_light_and_propagate(
    block_registry: &resources::block::Registry,
    raw_chunks: &mut FastHashMap<[i32; 2], Arc<RawChunk>>,
    subchunks_to_update: &mut FastHashSet<[i32; 3]>,
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
        let (old_level, _, _) = get_block_light_level_and_info(block_registry, raw_chunks, pos);
        set_light_level(
            raw_chunks,
            subchunks_to_update,
            pos,
            LightType::Block,
            new_level,
        );
        match u8::cmp(&new_level, &old_level) {
            core::cmp::Ordering::Less => {
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
                            get_block_light_level_and_info(block_registry, raw_chunks, neighbour);
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
            core::cmp::Ordering::Equal => break 'block_light,
            core::cmp::Ordering::Greater => increase_queue.push_back((pos, new_level, None)),
        }
        // Propagate increases
        while let Some((pos, new_level, from_dir)) = increase_queue.pop_front() {
            for (neighbour, dir) in neighbours(pos) {
                if Some(dir) == from_dir {
                    continue;
                }
                let (neighbour_light_level, neighbour_opacity, _) =
                    get_block_light_level_and_info(block_registry, raw_chunks, neighbour);
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
        let (old_level, _) = get_sky_light_level_and_opacity(block_registry, raw_chunks, pos);
        set_light_level(
            raw_chunks,
            subchunks_to_update,
            pos,
            LightType::Sky,
            new_level,
        );
        match u8::cmp(&new_level, &old_level) {
            core::cmp::Ordering::Less => {
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
                            get_sky_light_level_and_opacity(block_registry, raw_chunks, neighbour);
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
            core::cmp::Ordering::Equal => break 'sky_light,
            core::cmp::Ordering::Greater => increase_queue.push_back((pos, new_level, None)),
        }
        // Propagate increases
        while let Some((pos, new_level, from_dir)) = increase_queue.pop_front() {
            for (neighbour, dir) in neighbours(pos) {
                if Some(dir) == from_dir {
                    continue;
                }
                let (neighbour_light_level, neighbour_sky_opacity) =
                    get_sky_light_level_and_opacity(block_registry, raw_chunks, neighbour);
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
fn get_section_info_and_inner_pos<'a>(
    raw_chunks: &'a mut FastHashMap<[i32; 2], Arc<RawChunk>>,
    global_pos: [i32; 3],
    channel: LightType,
) -> Option<(
    &'a mut ChunkSection,
    ChunkSectionLightChannelInfoMut<'a>,
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
    block_registry: &resources::block::Registry,
    raw_chunks: &mut FastHashMap<[i32; 2], Arc<RawChunk>>,
    global_pos: [i32; 3],
) -> (u8, BlockOpacity, u8) {
    match get_section_info_and_inner_pos(raw_chunks, global_pos, LightType::Block) {
        None => (0, BlockOpacity::Opaque, 0),
        Some((chunk_section, light_section, [x, y, z])) => {
            let light_level = light_section.get(x, y, z);
            let global_palette_index = chunk_section.block_states.get(x, y, z);
            let blockstate_info = &block_registry[global_palette_index];
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
    block_registry: &resources::block::Registry,
    raw_chunks: &mut FastHashMap<[i32; 2], Arc<RawChunk>>,
    global_pos: [i32; 3],
) -> (u8, SkyLightOpacity) {
    match get_section_info_and_inner_pos(raw_chunks, global_pos, LightType::Sky) {
        None => (0, SkyLightOpacity::Opaque),
        Some((chunk_section, light_section, [x, y, z])) => {
            let light_level = light_section.get(x, y, z);
            let global_palette_index = chunk_section.block_states.get(x, y, z);
            let blockstate_info = &block_registry[global_palette_index];
            let extra_info = &blockstate_info.extra_info;
            (light_level, extra_info.light_info.sky_light_opacity)
        }
    }
}

#[inline]
fn set_light_level(
    raw_chunks: &mut FastHashMap<[i32; 2], Arc<RawChunk>>,
    subchunks_to_update: &mut FastHashSet<[i32; 3]>,
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
