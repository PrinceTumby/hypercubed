use crate::client::graphics::{self, GraphicsState};
use crate::client::input::PlayControlState;
use crate::client::{
    world, ClientPlayState, ClientPlayStateUpdate, RawChunk, MIN_HEIGHT_I32, SUBCHUNK_AXIS_LEN_I32,
};
use crate::identifier;
use crate::physics;
use crate::protocol::chunk as protocol_chunk;
use crate::protocol::play::{
    self as protocol_play, serverbound as serverbound_packets, Clientbound as ClientboundPacket,
    GameEventType, GameMode,
};
use crate::protocol::prelude::*;
use crate::resource::block::GlobalPaletteIndex;
use ahash::{AHashMap, AHashSet};
use indexmap::IndexMap;
use nalgebra::Vector3;
use std::sync::Arc;
use threadpool::ThreadPool;

#[allow(clippy::too_many_arguments)]
pub fn process_game_events(
    thread_pool: &ThreadPool,
    play_state: &mut ClientPlayState,
    graphics_state: &mut GraphicsState,
    debug_state: &mut graphics::DebugState,
    input_state: &mut PlayControlState,
    server_connection: &PlayConnection,
    clientbound_tx: &std::sync::mpsc::Sender<ClientboundPacket>,
    clientbound_rx: &std::sync::mpsc::Receiver<ClientboundPacket>,
    play_state_update_tx: &std::sync::mpsc::Sender<ClientPlayStateUpdate>,
    play_state_update_rx: &std::sync::mpsc::Receiver<ClientPlayStateUpdate>,
    current_update_id: &mut usize,
    current_time_s: f64,
    last_tick_time_s: &mut f64,
    next_tick_time_s: &mut f64,
    delta_time: f32,
) {
    let span = tracing::trace_span!("process_game_events");
    let _enter = span.enter();
    let visible_chunks = &play_state.visible_chunks;
    let mut raw_chunks;
    let mut subchunks_to_dispatch: AHashMap<[i32; 3], usize> = AHashMap::new();
    loop {
        let packet = match clientbound_rx.try_recv() {
            Ok(packet) => packet,
            Err(std::sync::mpsc::TryRecvError::Empty) => break,
            Err(other_err) => panic!("{other_err:?}"),
        };
        let span = tracing::trace_span!("dispatch_packet");
        let _enter = span.enter();
        match packet {
            // Basic
            ClientboundPacket::ErrorDisconnect { reason } => {
                println!("Disconnected: {reason:?}")
            }
            ClientboundPacket::BundleDelimiter => unreachable!(),
            ClientboundPacket::LoginPlay {
                raw_entity_id,
                game_mode,
                ..
            } => {
                play_state.player.entity_id = EntityId(raw_entity_id);
                play_state.player.game_mode = game_mode;
                println!("LoginPlay: {{");
                println!("    player_entity_id: {raw_entity_id},");
                println!("    game_mode: {game_mode:?},");
                println!("}}");
            }
            ClientboundPacket::GameEvent { event, value } => {
                if event == GameEventType::ChangeGameMode {
                    let new_game_mode = match value as u8 {
                        0 => GameMode::Survival,
                        1 => GameMode::Creative,
                        2 => GameMode::Adventure,
                        3 => GameMode::Spectator,
                        _ => panic!("Unknown game mode value {value}"),
                    };
                    play_state.player.game_mode = new_game_mode;
                    println!("Changed game mode to {new_game_mode:?}");
                }
            }
            ClientboundPacket::KeepAlive { id } => {
                server_connection
                    .send_packet(serverbound_packets::KeepAliveResponse { id })
                    .unwrap();
            }
            // Configuration
            ClientboundPacket::UpdateRecipes(_recipes) => {
                println!("Update recipes: [<skipped>]");
            }
            ClientboundPacket::UpdateTags(_tags) => {
                println!("Update tags: [<skipped>]");
            }
            ClientboundPacket::UpdateRecipeBook(_) => {
                println!("Update recipes: {{<skipped>}}");
            }
            ClientboundPacket::ServerData(data) => {
                println!("Server MOTD: {:?}", data.motd);
            }
            // Gameplay
            ClientboundPacket::SystemChatMessage {
                content,
                at_action_bar: _,
            } => {
                println!("System Chat Message: {content:?}");
            }
            ClientboundPacket::ChunkBatchStart => {}
            ClientboundPacket::ChunkBatchEnd { num_chunks: _ } => {
                // TODO: Calculate time taken since batch started, send back value
                // to use half of available bandwidth
                server_connection
                    .send_packet(serverbound_packets::ChunkBatchReceived {
                        desired_chunks_per_tick: 4.0,
                    })
                    .unwrap();
            }
            ClientboundPacket::ChunkDataAndUpdateLight(data) => {
                raw_chunks = Arc::make_mut(&mut play_state.raw_chunks);
                let [chunk_x, chunk_z] = data.chunk_xz;
                let (rest, chunk_sections) = nom::multi::count(
                    protocol_chunk::ChunkSection::deserialize,
                    24,
                )(InputSpan::new(&data.chunk_data))
                .unwrap();
                assert_eq!(rest.len(), 0);
                // eprintln!("Sky light mask:          {:b}", &data.light_info.sky_light_mask);
                // eprintln!("Empty sky light mask:    {:b}", &data.light_info.empty_sky_light_mask);
                // eprintln!("Block light mask:        {:b}", &data.light_info.block_light_mask);
                // eprintln!("Empty block light mask:  {:b}", &data.light_info.empty_block_light_mask);
                let lighting = protocol_chunk::ChunkLightInfo::from_raw(data.light_info, 24);
                raw_chunks.insert(
                    [chunk_x, chunk_z],
                    Arc::new(RawChunk {
                        sections: chunk_sections.into(),
                        lighting,
                    }),
                );
                {
                    let update_id = *current_update_id;
                    *current_update_id = current_update_id.wrapping_add(1);
                    for subchunk_y in 0..24 {
                        let subchunk_coords = [chunk_x, subchunk_y, chunk_z];
                        subchunks_to_dispatch.insert(subchunk_coords, update_id);
                        play_state
                            .pending_subchunk_update_ids
                            .insert(subchunk_coords, update_id);
                    }
                }
                let neighbouring_chunks = [
                    [chunk_x - 1, chunk_z],
                    [chunk_x + 1, chunk_z],
                    [chunk_x, chunk_z - 1],
                    [chunk_x, chunk_z + 1],
                ];
                for neighbour_chunk_coords in neighbouring_chunks {
                    if raw_chunks.contains_key(&neighbour_chunk_coords)
                        && !visible_chunks.contains(&neighbour_chunk_coords)
                    {
                        let update_id = *current_update_id;
                        *current_update_id = current_update_id.wrapping_add(1);
                        let [x, z] = neighbour_chunk_coords;
                        for y in 0..24 {
                            subchunks_to_dispatch.insert([x, y, z], update_id);
                            play_state
                                .pending_subchunk_update_ids
                                .insert([x, y, z], update_id);
                        }
                    }
                }
            }
            ClientboundPacket::UpdateLight {
                chunk_xz,
                light_info,
            } => {
                raw_chunks = Arc::make_mut(&mut play_state.raw_chunks);
                // Remove VarInt wrapper
                let chunk_xz = chunk_xz.map(|n| n.0);
                let Some(chunk) = raw_chunks.get_mut(&chunk_xz) else {
                    continue;
                };
                let chunk_mut = Arc::make_mut(chunk);
                chunk_mut.lighting.update_from_raw(light_info);
            }
            ClientboundPacket::BlockUpdate(update) => {
                raw_chunks = Arc::make_mut(&mut play_state.raw_chunks);
                let pos = update.position;
                let chunk_x = pos.x.div_euclid(SUBCHUNK_AXIS_LEN_I32);
                let chunk_z = pos.z.div_euclid(SUBCHUNK_AXIS_LEN_I32);
                let section_i: usize = (pos.y - MIN_HEIGHT_I32)
                    .div_euclid(SUBCHUNK_AXIS_LEN_I32)
                    .try_into()
                    .unwrap();
                let Some(chunk) = raw_chunks.get_mut(&[chunk_x, chunk_z]) else {
                    continue;
                };
                let chunk_mut = Arc::make_mut(chunk);
                let chunk_section = &mut chunk_mut.sections[section_i];
                let x = pos.x.rem_euclid(SUBCHUNK_AXIS_LEN_I32);
                let x_usize: usize = x.try_into().unwrap();
                let y = pos.y.rem_euclid(SUBCHUNK_AXIS_LEN_I32);
                let y_usize: usize = y.try_into().unwrap();
                let z = pos.z.rem_euclid(SUBCHUNK_AXIS_LEN_I32);
                let z_usize: usize = z.try_into().unwrap();
                // Update block section and lighting, increment or decrement block
                // count
                let mut subchunks_to_relight = AHashSet::new();
                {
                    let new_block_id: GlobalPaletteIndex = update.block_id.0.try_into().unwrap();
                    let old_block_id =
                        chunk_section
                            .block_states
                            .replace(x_usize, y_usize, z_usize, new_block_id);
                    let old_block_air = graphics_state
                        .resources
                        .block_registry
                        .is_blockstate_air_like(old_block_id);
                    let new_block_air = graphics_state
                        .resources
                        .block_registry
                        .is_blockstate_air_like(new_block_id);
                    match (old_block_air, new_block_air) {
                        (true, false) => chunk_section.block_count += 1,
                        (false, true) => chunk_section.block_count -= 1,
                        (true, true) => continue,
                        _ => {}
                    }
                    // Update lighting
                    world::recalculate_light(
                        &graphics_state.resources,
                        raw_chunks,
                        &mut subchunks_to_relight,
                        [pos.x, pos.y, pos.z],
                        old_block_id,
                        new_block_id,
                    );
                }
                let subchunk_y = section_i as i32;
                let update_id = *current_update_id;
                *current_update_id = current_update_id.wrapping_add(1);
                subchunks_to_dispatch.insert([chunk_x, subchunk_y, chunk_z], update_id);
                play_state
                    .pending_subchunk_update_ids
                    .insert([chunk_x, subchunk_y, chunk_z], update_id);
                for subchunk_coords in subchunks_to_relight {
                    subchunks_to_dispatch.insert(subchunk_coords, update_id);
                    play_state
                        .pending_subchunk_update_ids
                        .insert(subchunk_coords, update_id);
                }
                // Update neighbours
                let in_chunk_coords = [x, y, z];
                for axis_i in 0..3 {
                    let axis = in_chunk_coords[axis_i];
                    let mut subchunk_coords = [chunk_x, subchunk_y, chunk_z];
                    if axis == 0 {
                        subchunk_coords[axis_i] -= 1;
                        subchunks_to_dispatch.insert(subchunk_coords, update_id);
                        play_state
                            .pending_subchunk_update_ids
                            .insert(subchunk_coords, update_id);
                    } else if axis == 15 {
                        subchunk_coords[axis_i] += 1;
                        subchunks_to_dispatch.insert(subchunk_coords, update_id);
                        play_state
                            .pending_subchunk_update_ids
                            .insert(subchunk_coords, update_id);
                    }
                }
            }
            ClientboundPacket::UpdateSectionBlocks(update) => {
                raw_chunks = Arc::make_mut(&mut play_state.raw_chunks);
                let [chunk_x, subchunk_y, chunk_z] = update.subchunk_coords;
                let section_i: usize = (subchunk_y
                    - MIN_HEIGHT_I32.div_euclid(SUBCHUNK_AXIS_LEN_I32))
                .try_into()
                .unwrap();
                let Some(chunk) = raw_chunks.get_mut(&[chunk_x, chunk_z]) else {
                    continue;
                };
                let chunk_mut = Arc::make_mut(chunk);
                let chunk_section = &mut chunk_mut.sections[section_i];
                let update_id = *current_update_id;
                *current_update_id = current_update_id.wrapping_add(1);
                let subchunk_y = section_i as i32;
                subchunks_to_dispatch.insert([chunk_x, subchunk_y, chunk_z], update_id);
                play_state
                    .pending_subchunk_update_ids
                    .insert([chunk_x, subchunk_y, chunk_z], update_id);
                let mut old_block_ids = Vec::new();
                for &([x, y, z], new_block_id) in &update.blocks {
                    // Update block section, increment or decrement block count
                    {
                        let old_block_id = chunk_section.block_states.replace(
                            x as usize,
                            y as usize,
                            z as usize,
                            new_block_id,
                        );
                        old_block_ids.push(old_block_id);
                        let is_old_block_air = graphics_state
                            .resources
                            .block_registry
                            .is_blockstate_air_like(old_block_id);
                        let is_new_block_air = graphics_state
                            .resources
                            .block_registry
                            .is_blockstate_air_like(new_block_id);
                        match (is_old_block_air, is_new_block_air) {
                            (true, false) => chunk_section.block_count += 1,
                            (false, true) => chunk_section.block_count -= 1,
                            (true, true) => continue,
                            _ => {}
                        }
                    }
                    // Update neighbours
                    let in_chunk_coords = [x, y, z];
                    for axis_i in 0..3 {
                        let axis = in_chunk_coords[axis_i];
                        let mut subchunk_coords = [chunk_x, subchunk_y, chunk_z];
                        if axis == 0 {
                            subchunk_coords[axis_i] -= 1;
                            subchunks_to_dispatch.insert(subchunk_coords, update_id);
                            play_state
                                .pending_subchunk_update_ids
                                .insert(subchunk_coords, update_id);
                        } else if axis == 15 {
                            subchunk_coords[axis_i] += 1;
                            subchunks_to_dispatch.insert(subchunk_coords, update_id);
                            play_state
                                .pending_subchunk_update_ids
                                .insert(subchunk_coords, update_id);
                        }
                    }
                }
                // Update lighting
                let new_block_ids_iter = update.blocks.into_iter();
                let old_block_ids_iter = old_block_ids.into_iter();
                let iter = Iterator::zip(old_block_ids_iter, new_block_ids_iter);
                let mut subchunks_to_relight = AHashSet::new();
                for (old_block_id, ([x, y, z], new_block_id)) in iter {
                    let global_x = chunk_x * SUBCHUNK_AXIS_LEN_I32 + x as i32;
                    let global_y =
                        section_i as i32 * SUBCHUNK_AXIS_LEN_I32 + y as i32 + MIN_HEIGHT_I32;
                    let global_z = chunk_z * SUBCHUNK_AXIS_LEN_I32 + z as i32;
                    world::recalculate_light(
                        &graphics_state.resources,
                        raw_chunks,
                        &mut subchunks_to_relight,
                        [global_x, global_y, global_z],
                        old_block_id,
                        new_block_id,
                    );
                }
                for subchunk_coords in subchunks_to_relight {
                    subchunks_to_dispatch.insert(subchunk_coords, update_id);
                    play_state
                        .pending_subchunk_update_ids
                        .insert(subchunk_coords, update_id);
                }
            }
            ClientboundPacket::UnloadChunk { chunk_x, chunk_z } => {
                raw_chunks = Arc::make_mut(&mut play_state.raw_chunks);
                let chunk_coords = [chunk_x, chunk_z];
                raw_chunks.remove(&chunk_coords);
                play_state_update_tx
                    .send(ClientPlayStateUpdate::RemoveChunk(chunk_coords))
                    .unwrap();
                let neighbouring_chunks = [
                    [chunk_x - 1, chunk_z],
                    [chunk_x + 1, chunk_z],
                    [chunk_x, chunk_z - 1],
                    [chunk_x, chunk_z + 1],
                ];
                for neighbour_chunk_coords in neighbouring_chunks {
                    if visible_chunks.contains(&neighbour_chunk_coords) {
                        play_state_update_tx
                            .send(ClientPlayStateUpdate::RemoveChunk(neighbour_chunk_coords))
                            .unwrap();
                    }
                }
            }
            ClientboundPacket::SynchronizePlayerPosition(pos_info) => {
                use crate::protocol::play::{PositionChange, RotationChange};
                let player = &mut play_state.player;
                let camera = &mut graphics_state.camera;
                player.pos.x = match pos_info.x {
                    PositionChange::Absolute(new_x) => new_x as f32,
                    PositionChange::Relative(x_diff) => player.pos.x + x_diff as f32,
                };
                player.pos.y = match pos_info.y {
                    PositionChange::Absolute(new_y) => new_y as f32,
                    PositionChange::Relative(y_diff) => player.pos.y + y_diff as f32,
                };
                player.pos.z = match pos_info.z {
                    PositionChange::Absolute(new_z) => new_z as f32,
                    PositionChange::Relative(z_diff) => player.pos.z + z_diff as f32,
                };
                let (raw_yaw, raw_pitch) = player.get_mc_rot();
                let new_yaw = match pos_info.yaw {
                    RotationChange::Absolute(new_yaw) => new_yaw,
                    RotationChange::Relative(yaw_diff) => raw_yaw + yaw_diff,
                };
                let new_pitch = match pos_info.pitch {
                    RotationChange::Absolute(new_pitch) => new_pitch,
                    RotationChange::Relative(pitch_diff) => raw_pitch + pitch_diff,
                };
                player.set_mc_rot(new_yaw, new_pitch);
                // Teleport camera back to player position, force free cam off.
                debug_state.free_cam = false;
                camera.pos = player.pos + Vector3::new(0.0, 1.62, 0.0);
                camera.yaw = player.yaw;
                camera.pitch = player.pitch;
                // Update previous tick position so that teleportation is instant,
                // instead of interpolation making it look like we're moving fast
                // over the span of a tick.
                play_state.player_last_tick = player.clone();
                // Let server know we've completed the teleport.
                server_connection
                    .send_packet(serverbound_packets::ConfirmTeleportation {
                        id: pos_info.teleport_id,
                    })
                    .unwrap();
            }
            ClientboundPacket::Explosion {
                base_coords,
                affected_block_offsets,
                ..
            } => {
                // Reconvert explosion block updates into a series of
                // `UpdateSectionBlocks` updates.
                let air_global_palette_index = graphics_state
                    .resources
                    .block_registry
                    .get_entry_from_identifier(&identifier!("minecraft:air"))
                    .unwrap()
                    .default_blockstate;
                let [base_x, base_y, base_z] = base_coords.map(|n| n as i32);
                let mut subchunk_updates: AHashMap<[i32; 3], Vec<[u8; 3]>> =
                    AHashMap::with_capacity(1);
                for [x, y, z] in affected_block_offsets {
                    let global_x = base_x + x as i32;
                    let global_y = base_y + y as i32;
                    let global_z = base_z + z as i32;
                    let chunk_x = global_x.div_euclid(SUBCHUNK_AXIS_LEN_I32);
                    let chunk_z = global_z.div_euclid(SUBCHUNK_AXIS_LEN_I32);
                    let section_i = (global_y - MIN_HEIGHT_I32).div_euclid(SUBCHUNK_AXIS_LEN_I32);
                    let subchunk_y = section_i;
                    let local_x = global_x.rem_euclid(SUBCHUNK_AXIS_LEN_I32);
                    let local_y = global_y.rem_euclid(SUBCHUNK_AXIS_LEN_I32);
                    let local_z = global_z.rem_euclid(SUBCHUNK_AXIS_LEN_I32);
                    subchunk_updates
                        .entry([chunk_x, subchunk_y, chunk_z])
                        .or_default()
                        .push([local_x, local_y, local_z].map(|n| n.try_into().unwrap()));
                }
                for (subchunk_coords, blocks) in subchunk_updates {
                    clientbound_tx
                        .send(ClientboundPacket::UpdateSectionBlocks(
                            protocol_play::UpdateSectionBlocks {
                                subchunk_coords,
                                blocks: blocks
                                    .into_iter()
                                    .map(|coords| (coords, air_global_palette_index))
                                    .collect(),
                            },
                        ))
                        .unwrap();
                }
                // TODO: Add explosion velocity to player
            }
            // other => println!("{other:?}"),
            _ => {}
        }
    }
    if thread_pool.panic_count() > 0 {
        panic!("Thread pool panic");
    }
    // Dispatch subchunk processing
    {
        let span = tracing::trace_span!("dispatch_subchunk_processing");
        let _enter = span.enter();
        // We're using an IndexMap here to dispatch the updates in order of update
        // ID. Shouldn't have any effect on correctness, but might make updates
        // appear in a more intuitive order.
        let mut subchunk_update_groups: IndexMap<usize, Vec<[i32; 3]>> = IndexMap::new();
        for (subchunk_coords, update_id) in subchunks_to_dispatch {
            let update_group = subchunk_update_groups.entry(update_id).or_default();
            update_group.push(subchunk_coords);
        }
        subchunk_update_groups.sort_unstable_keys();
        for (update_id, subchunks) in subchunk_update_groups {
            let graphics_resources = graphics_state.resources.clone();
            let raw_chunks = play_state.raw_chunks.clone();
            let play_state_update_tx = play_state_update_tx.clone();
            thread_pool.execute(move || {
                world::process_subchunks(
                    &graphics_resources,
                    &raw_chunks,
                    play_state_update_tx,
                    &subchunks,
                    update_id,
                )
            });
        }
    }
    // Receive play state updates
    let mut subchunks_processed_this_frame = 0;
    loop {
        if subchunks_processed_this_frame >= 12 {
            break;
        }
        let update = match play_state_update_rx.try_recv() {
            Ok(update) => update,
            Err(std::sync::mpsc::TryRecvError::Empty) => break,
            Err(other_err) => {
                panic!("Error while trying to receive play state update: {other_err:?}")
            }
        };
        let span = tracing::trace_span!("dispatch_play_state_update");
        let _enter = span.enter();
        match update {
            ClientPlayStateUpdate::RemoveChunk(chunk_coords) => {
                let [chunk_x, chunk_z] = chunk_coords;
                // Remove old subchunks
                if play_state.visible_chunks.contains(&chunk_coords) {
                    let span = tracing::trace_span!("remove_subchunks", ?chunk_coords);
                    let _enter = span.enter();
                    for subchunk_y in 0..24 {
                        use std::collections::hash_map::Entry;
                        let subchunk_coords = [chunk_x, subchunk_y, chunk_z];
                        let span = tracing::trace_span!("remove_subchunk", ?subchunk_coords);
                        let _enter = span.enter();
                        if let Entry::Occupied(entry) = play_state.subchunks.entry(subchunk_coords)
                        {
                            entry.remove();
                            play_state
                                .pending_subchunk_update_ids
                                .remove(&subchunk_coords);
                            graphics_state
                                .buffer_managers
                                .block_face_vertex
                                .free_subchunk_areas(subchunk_coords);
                            graphics_state
                                .buffer_managers
                                .block_face_instance
                                .free_subchunk_areas(subchunk_coords);
                            graphics_state
                                .buffer_managers
                                .tinted_block_face_vertex
                                .free_subchunk_areas(subchunk_coords);
                            graphics_state
                                .buffer_managers
                                .tinted_block_face_instance
                                .free_subchunk_areas(subchunk_coords);
                            graphics_state
                                .buffer_managers
                                .custom_block_instance
                                .free_subchunk_areas(subchunk_coords);
                            subchunks_processed_this_frame += 1;
                        }
                    }
                }
                // Mark chunk as invisible
                play_state.visible_chunks.remove(&chunk_coords);
            }
            ClientPlayStateUpdate::PlaceSubchunks {
                update_id,
                new_raw_subchunks,
            } => {
                let span = tracing::trace_span!("add_new_subchunks");
                let _enter = span.enter();
                let buffer_managers = &mut graphics_state.buffer_managers;
                for (subchunk_coords, raw_subchunk) in new_raw_subchunks {
                    use std::collections::hash_map::Entry;
                    let [subchunk_x, _, subchunk_z] = subchunk_coords;
                    match play_state
                        .pending_subchunk_update_ids
                        .entry(subchunk_coords)
                    {
                        Entry::Occupied(update_id_entry) => {
                            // If this is the most recent pending update, mark
                            // the update as completed so older updates that
                            // may have taken longer to process don't replace
                            // this one.
                            if *update_id_entry.get() == update_id {
                                update_id_entry.remove();
                            }
                        }
                        // Skip subchunk update if we've already done a more
                        // recent one.
                        Entry::Vacant(_) => continue,
                    }
                    // Remove old subchunk
                    if play_state
                        .visible_chunks
                        .contains(&[subchunk_x, subchunk_z])
                    {
                        let span = tracing::trace_span!("remove_old_subchunk", ?subchunk_coords);
                        let _enter = span.enter();
                        let span = tracing::trace_span!("remove_old_subchunk", ?subchunk_coords);
                        let _enter = span.enter();
                        if let Entry::Occupied(entry) = play_state.subchunks.entry(subchunk_coords)
                        {
                            entry.remove();
                            buffer_managers
                                .block_face_vertex
                                .free_subchunk_areas(subchunk_coords);
                            buffer_managers
                                .block_face_instance
                                .free_subchunk_areas(subchunk_coords);
                            buffer_managers
                                .tinted_block_face_vertex
                                .free_subchunk_areas(subchunk_coords);
                            buffer_managers
                                .tinted_block_face_instance
                                .free_subchunk_areas(subchunk_coords);
                            buffer_managers
                                .custom_block_instance
                                .free_subchunk_areas(subchunk_coords);
                        }
                    }
                    // Base block faces
                    let mut block_face_start_vertices: [u32; 6] = [u32::MAX; 6];
                    let mut block_face_instance_groups: [(u32, u32); 6] = Default::default();
                    for i in 0..6 {
                        let Some(base_quad) = raw_subchunk.block_face_quads[i] else {
                            continue;
                        };
                        let quad_start_vertex = buffer_managers.block_face_vertex.alloc_area(
                            &graphics_state.resources.queue,
                            subchunk_coords,
                            base_quad,
                        );
                        let instance_group = &raw_subchunk.block_face_instance_groups[i];
                        let instance_group_start = buffer_managers.block_face_instance.alloc_area(
                            &graphics_state.resources.queue,
                            subchunk_coords,
                            instance_group,
                        );
                        let instance_group_len: u32 = instance_group.len().try_into().unwrap();
                        block_face_start_vertices[i] = quad_start_vertex;
                        block_face_instance_groups[i] = (instance_group_start, instance_group_len);
                    }
                    // Tinted block faces
                    let mut tinted_block_face_start_vertices: [u32; 6] = [u32::MAX; 6];
                    let mut tinted_block_face_instance_groups: [(u32, u32); 6] = Default::default();
                    for i in 0..6 {
                        let Some(base_quad) = raw_subchunk.tinted_block_face_quads[i] else {
                            continue;
                        };
                        let quad_start_vertex =
                            buffer_managers.tinted_block_face_vertex.alloc_area(
                                &graphics_state.resources.queue,
                                subchunk_coords,
                                base_quad,
                            );
                        let instance_group = &raw_subchunk.tinted_block_face_instance_groups[i];
                        let instance_group_start =
                            buffer_managers.tinted_block_face_instance.alloc_area(
                                &graphics_state.resources.queue,
                                subchunk_coords,
                                instance_group,
                            );
                        let instance_group_len: u32 = instance_group.len().try_into().unwrap();
                        tinted_block_face_start_vertices[i] = quad_start_vertex;
                        tinted_block_face_instance_groups[i] =
                            (instance_group_start, instance_group_len);
                    }
                    let custom_block_groups = raw_subchunk
                        .custom_block_groups
                        .into_iter()
                        .map(|group| {
                            let start_instance = buffer_managers.custom_block_instance.alloc_area(
                                &graphics_state.resources.queue,
                                subchunk_coords,
                                &group.instances,
                            );
                            let num_instances: u32 = group.instances.len().try_into().unwrap();
                            graphics::chunk_rc::CustomBlockGroup {
                                start_vertex: group.start_vertex,
                                start_index_and_len: group.start_index_and_len,
                                start_instance_and_len: [start_instance, num_instances],
                            }
                        })
                        .collect();
                    play_state.subchunks.insert(
                        subchunk_coords,
                        graphics::chunk_rc::Subchunk {
                            start_coords: raw_subchunk.start_coords,
                            block_face_start_vertices,
                            block_face_instance_groups,
                            tinted_block_face_start_vertices,
                            tinted_block_face_instance_groups,
                            custom_block_groups,
                            connected_faces: raw_subchunk.connected_faces,
                        },
                    );
                    subchunks_processed_this_frame += 1;
                    play_state.visible_chunks.insert([subchunk_x, subchunk_z]);
                }
            }
        }
    }
    tracing::trace_span!("chunk_queue_submit").in_scope(|| {
        graphics_state.resources.queue.submit([]);
    });
    // Run tick updates
    {
        let span = tracing::trace_span!("tick_updates");
        let _enter = span.enter();
        let tick_this_frame = if current_time_s >= *next_tick_time_s {
            *last_tick_time_s = current_time_s;
            *next_tick_time_s += 1.0 / 20.0;
            true
        } else {
            false
        };
        let player = &mut play_state.player;
        let player_last_tick = &mut play_state.player_last_tick;
        let camera = &mut graphics_state.camera;
        if !debug_state.free_cam {
            player.yaw = camera.yaw;
            player.pitch = camera.pitch;
        }
        if tick_this_frame {
            *player_last_tick = player.clone();
            match player.game_mode {
                GameMode::Spectator => {}
                _ => {
                    let mut player_input = if debug_state.free_cam {
                        physics::PlayerInput {
                            forward: false,
                            backward: false,
                            left: false,
                            right: false,
                            jump: false,
                            sneak: false,
                            sprint: false,
                        }
                    } else {
                        input_state.sprint = input_state.sprint || input_state.trying_to_sprint;
                        input_state.sprint =
                            input_state.sprint && input_state.forward && !input_state.sneak;
                        physics::PlayerInput {
                            forward: input_state.forward,
                            backward: input_state.backward,
                            left: input_state.left,
                            right: input_state.right,
                            jump: input_state.jump,
                            sneak: input_state.sneak,
                            sprint: input_state.sprint,
                        }
                    };
                    physics::simulate_player(
                        &graphics_state.resources.block_registry,
                        &play_state.raw_chunks,
                        player,
                        &mut player_input,
                    );
                    if !debug_state.free_cam {
                        input_state.sprint = player_input.sprint;
                    }
                }
            }
        }
        match player.game_mode {
            GameMode::Spectator if !debug_state.free_cam => {
                input_state.update_fly_camera_pos(camera, delta_time);
                player.pos = camera.pos - Vector3::new(0.0, 1.62, 0.0);
            }
            _ if debug_state.free_cam => {
                input_state.update_fly_camera_pos(camera, delta_time);
            }
            _ => {
                let (interpolated_camera_pos, interpolated_camera_fov) = {
                    fn mix<T, U>(last_tick: T, next_tick: T, tick_percentage: f32) -> T
                    where
                        T: std::ops::Add<U, Output = T> + std::ops::Sub<T, Output = U> + Clone,
                        U: std::ops::Mul<f32, Output = U>,
                    {
                        let diff = next_tick - last_tick.clone();
                        last_tick + (diff * tick_percentage)
                    }
                    let tick_duration = *next_tick_time_s - *last_tick_time_s;
                    let time_since_last_tick = current_time_s - *last_tick_time_s;
                    let tick_percentage = time_since_last_tick / tick_duration;
                    let tick_percentage_f32 = tick_percentage as f32;
                    (
                        // Camera pos
                        mix(
                            player_last_tick.pos + Vector3::new(0.0, 1.62, 0.0),
                            player.pos + Vector3::new(0.0, 1.62, 0.0),
                            tick_percentage_f32,
                        ),
                        // Camera FOV
                        mix(
                            match player_last_tick.physics_state.sprinting {
                                false => GraphicsState::DEFAULT_FOV,
                                true => GraphicsState::DEFAULT_FOV + 10.0,
                            },
                            match player.physics_state.sprinting {
                                false => GraphicsState::DEFAULT_FOV,
                                true => GraphicsState::DEFAULT_FOV + 10.0,
                            },
                            tick_percentage_f32,
                        ),
                    )
                };
                camera.pos = interpolated_camera_pos;
                camera
                    .proj_matrix
                    .set_fovy(interpolated_camera_fov.to_radians());
            }
        }
        {
            let span = tracing::trace_span!("send_move_packet");
            let _enter = span.enter();
            let player = &play_state.player;
            let (mc_yaw, mc_pitch) = player.get_mc_rot();
            if tick_this_frame {
                // TODO: Send `PlayerCommand` and `PlayerInput` packets
                server_connection
                    .send_packet(serverbound_packets::SetPlayerPositionAndRotation {
                        // x: player.pos.x as f64,
                        // feet_y: player.pos.y as f64,
                        // z: player.pos.z as f64,
                        x: 15.0,
                        feet_y: 162.0,
                        z: -16.0,
                        mc_yaw,
                        mc_pitch,
                        on_ground: false,
                    })
                    .unwrap();
            }
            server_connection.flush().unwrap();
        }
    }
}
