pub mod graphics;
pub mod input;

use ahash::{AHashMap, AHashSet};
use graphics::GraphicsState;
use input::PlayControlState;
use std::time::Instant;
use winit::event::{Event, StartCause, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

use super::resource;
use crate::identifier;
use resource::block::blockstate;
use resource::block::model::ModelVertex;
use resource::block::model::{ModelType, Tint};
use resource::block::RightAngleRotation;

// fn rotate_uvs([u1, v1, u2, v2]: [u16; 4], rotation: &RightAngleRotation) -> [u16; 4] {
//     match rotation {
//         &RightAngleRotation::Zero => [u1, v1, u2, v2],
//         &RightAngleRotation::Ninety => [v2, u1, u2, v1],
//         &RightAngleRotation::OneEighty => [u2, v2, u1, v1],
//         &RightAngleRotation::TwoSeventy => [u1, v2, u2, v1],
//     }
// }

// TODO Give this a better name
pub(crate) async fn window_run(
    chunks: AHashMap<(i32, i32), Vec<super::ChunkSection>>,
) -> anyhow::Result<()> {
    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new().build(&event_loop)?;
    window.set_title("Rust Minecraft Client");
    let window_id = window.id();
    let mut scale_factor = window.scale_factor();
    let mut graphics_state =
        GraphicsState::new(window, super::resource::block::register_vanilla_blocks).await?;
    let mut input_state = PlayControlState::default();
    let egui_ctx = egui::Context::default();
    // Generate block faces for each subchunk
    #[derive(Clone, Debug)]
    struct AddedCustomBlock {
        pub start_vertex: u32,
        pub start_index: u32,
        pub num_indices: u32,
        pub instance_group: Vec<graphics::chunk::custom_block::Instance>,
    }
    let mut subchunks: AHashMap<[i32; 3], graphics::chunk::Subchunk> = AHashMap::new();
    let mut loaded_chunks: AHashSet<[i32; 2]> = AHashSet::new();
    let spruce_leaves_registry_index = graphics_state
        .block_registry
        .get_index_from_identifier(&identifier!("minecraft:spruce_leaves"))
        .unwrap();
    let chunk_gen_start_time = std::time::Instant::now();
    'chunk_loop: for (&(chunk_x, chunk_z), chunk_sections) in chunks.iter() {
        loaded_chunks.insert([chunk_x, chunk_z]);
        'subchunk_loop: for (section_i, chunk_section) in chunk_sections.iter().enumerate() {
            if chunk_section.block_count == 0 {
                continue;
            }
            // Skip chunks with missing neighbours, so that for every chunk we actually render, it
            // has all its neighbours to decide whether border faces should be rendered.
            // I believe Minecraft does the same.
            {
                let surrounding_chunk_coords = [
                    (chunk_x - 1, chunk_z),
                    (chunk_x + 1, chunk_z),
                    (chunk_x, chunk_z - 1),
                    (chunk_x, chunk_z + 1),
                ];
                for chunk in surrounding_chunk_coords {
                    if !chunks.contains_key(&chunk) {
                        continue 'chunk_loop;
                    }
                }
            }
            let mut block_faces = Vec::new();
            let mut tinted_block_faces = Vec::new();
            let mut custom_block_vertices = Vec::new();
            let mut custom_block_indices = Vec::new();
            let mut added_custom_blocks = AHashMap::new();
            const AXIS_LEN: usize = 16;
            const AXIS_LEN_I32: i32 = AXIS_LEN as i32;
            const MIN_HEIGHT_I32: i32 = -64;
            const MAX_HEIGHT_I32: i32 = 319;
            for y in 0..AXIS_LEN {
                let global_y = ((AXIS_LEN * section_i + y) as i32 + MIN_HEIGHT_I32) as f32;
                for z in 0..AXIS_LEN {
                    let global_z_i32 = (AXIS_LEN_I32 * chunk_z) + z as i32;
                    let global_z = global_z_i32 as f32;
                    for x in 0..AXIS_LEN {
                        let global_x_i32 = (AXIS_LEN_I32 * chunk_x) + x as i32;
                        let global_x = global_x_i32 as f32;
                        let global_palette_index = chunk_section.block_states.get(x, y, z);
                        let blockstate_info = &graphics_state.block_registry.global_palette
                            [usize::from(global_palette_index)];
                        let (model, x_rot, y_rot) = match &blockstate_info.model_data {
                            blockstate::ModelData::Single(model) => {
                                (&model.model, model.x_rotation, model.y_rotation)
                            }
                            // TODO: Support randomised models:
                            // - Use ahash to hash the position, generate a pseudo-random number
                            blockstate::ModelData::RandomChoice(models) => {
                                let model = &models[0];
                                (&model.model, model.x_rotation, model.y_rotation)
                            }
                        };
                        let x_rot_mat = x_rot.matrix_index();
                        let y_rot_mat = y_rot.matrix_index();
                        let direction_map = [
                            (x as i32, y as i32 + 1, z as i32),
                            (x as i32, y as i32 - 1, z as i32),
                            (x as i32, y as i32, z as i32 - 1),
                            (x as i32, y as i32, z as i32 + 1),
                            (x as i32 + 1, y as i32, z as i32),
                            (x as i32 - 1, y as i32, z as i32),
                        ];
                        let mut face_cull_map = [false; 6];
                        for (i, (x, y, z)) in direction_map.into_iter().enumerate() {
                            let check_global_y = ((AXIS_LEN * section_i) as i32 + y) - 64;
                            if check_global_y < MIN_HEIGHT_I32 || check_global_y > MAX_HEIGHT_I32 {
                                continue;
                            }
                            let check_sections = match [x, z].iter().any(|n| !(0..=15).contains(n))
                            {
                                false => &chunk_sections,
                                true => match (x, z) {
                                    (-1, _) => &chunks[&(chunk_x - 1, chunk_z)],
                                    (16, _) => &chunks[&(chunk_x + 1, chunk_z)],
                                    (_, -1) => &chunks[&(chunk_x, chunk_z - 1)],
                                    (_, 16) => &chunks[&(chunk_x, chunk_z + 1)],
                                    _ => unreachable!(),
                                },
                            };
                            let indexing_section = &check_sections[usize::try_from(
                                ((AXIS_LEN * section_i) as i32 + y) / AXIS_LEN as i32,
                            )
                            .unwrap()];
                            let (x, y, z) = (
                                ((x + AXIS_LEN_I32) % AXIS_LEN_I32) as usize,
                                y as usize,
                                ((z + AXIS_LEN_I32) % AXIS_LEN_I32) as usize,
                            );
                            let global_palette_index =
                                indexing_section.block_states.get(x, y % 16, z);
                            let blockstate_info = &graphics_state.block_registry.global_palette
                                [usize::from(global_palette_index)];
                            let block_info = &graphics_state
                                .block_registry[blockstate_info.block_index];
                            face_cull_map[i] = block_info.properties.opaque;
                        }
                        // Rotate face cull map by block rotation
                        {
                            face_cull_map = match x_rot {
                                RightAngleRotation::Zero => face_cull_map,
                                RightAngleRotation::Ninety => [
                                    face_cull_map[2],
                                    face_cull_map[3],
                                    face_cull_map[1],
                                    face_cull_map[5],
                                    face_cull_map[4],
                                    face_cull_map[5],
                                ],
                                RightAngleRotation::OneEighty => [
                                    face_cull_map[1],
                                    face_cull_map[0],
                                    face_cull_map[3],
                                    face_cull_map[2],
                                    face_cull_map[4],
                                    face_cull_map[5],
                                ],
                                RightAngleRotation::TwoSeventy => [
                                    face_cull_map[3],
                                    face_cull_map[2],
                                    face_cull_map[5],
                                    face_cull_map[1],
                                    face_cull_map[4],
                                    face_cull_map[5],
                                ],
                            };
                            face_cull_map = match y_rot {
                                RightAngleRotation::Zero => face_cull_map,
                                RightAngleRotation::Ninety => [
                                    face_cull_map[0],
                                    face_cull_map[1],
                                    face_cull_map[4],
                                    face_cull_map[5],
                                    face_cull_map[3],
                                    face_cull_map[2],
                                ],
                                RightAngleRotation::OneEighty => [
                                    face_cull_map[0],
                                    face_cull_map[1],
                                    face_cull_map[3],
                                    face_cull_map[2],
                                    face_cull_map[5],
                                    face_cull_map[4],
                                ],
                                RightAngleRotation::TwoSeventy => [
                                    face_cull_map[0],
                                    face_cull_map[1],
                                    face_cull_map[5],
                                    face_cull_map[4],
                                    face_cull_map[2],
                                    face_cull_map[3],
                                ],
                            };
                        }
                        // TODO: Greedy meshing, using same principles as culling:
                        // - Culling works by checking blocks in directions
                        // - If the block in the direction is the same block (and neither have
                        //   rotation), then we can merge them together
                        // Spruce Leaves are hardcoded, so override tint colour here
                        let tint_color = match blockstate_info.block_index {
                            ident if ident == spruce_leaves_registry_index => {
                                [0x61, 0x99, 0x61, 0xFF]
                            }
                            _ => [0x91, 0xBD, 0x59, 0xFF],
                        };
                        match model.as_ref() {
                            ModelType::None => continue,
                            ModelType::Block(info) => {
                                for i in 0..6 {
                                    if face_cull_map[i] {
                                        continue;
                                    }
                                    block_faces.push(graphics::chunk::block_face::Instance {
                                        pos: [global_x, global_y, global_z],
                                        uvs: info.per_face_atlas_uvs[i],
                                        matrix_indices: [i as u8, x_rot_mat, y_rot_mat, 0],
                                    });
                                }
                            }
                            ModelType::TintedBlock(info) => {
                                for i in 0..6 {
                                    if face_cull_map[i] {
                                        continue;
                                    }
                                    tinted_block_faces.push(
                                        graphics::chunk::tinted_block_face::Instance {
                                            pos: [global_x, global_y, global_z],
                                            uvs: info.per_face_atlas_uvs[i],
                                            matrix_indices: [i as u8, x_rot_mat, y_rot_mat, 0],
                                            tint_color,
                                        },
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
                                        tinted_block_faces.push(
                                            graphics::chunk::tinted_block_face::Instance {
                                                pos: [global_x, global_y, global_z],
                                                uvs: face.atlas_uvs,
                                                matrix_indices: [
                                                    face.face_i,
                                                    x_rot_mat,
                                                    y_rot_mat,
                                                    0,
                                                ],
                                                tint_color,
                                            },
                                        );
                                    } else {
                                        block_faces.push(graphics::chunk::block_face::Instance {
                                            pos: [global_x, global_y, global_z],
                                            uvs: face.atlas_uvs,
                                            matrix_indices: [face.face_i, x_rot_mat, y_rot_mat, 0],
                                        });
                                    }
                                }
                            }
                            ModelType::Other(info) => {
                                use graphics::chunk::custom_block;
                                let added_block =
                                    added_custom_blocks.entry(info).or_insert_with(|| {
                                        let start_vertex: u32 =
                                            custom_block_vertices.len().try_into().unwrap();
                                        let start_index: u32 =
                                            custom_block_indices.len().try_into().unwrap();
                                        let num_indices: u32 =
                                            info.indices.len().try_into().unwrap();
                                        custom_block_vertices.extend(info.vertices.iter().map(
                                            |v| custom_block::Vertex {
                                                pos: *v.local_pos.coords.as_ref(),
                                                uvs: v.uvs,
                                                normal: *v.normal.as_ref(),
                                                tint_percentage: match v.tint {
                                                    None => 0.0,
                                                    Some(Tint::Biome) => 1.0,
                                                },
                                            },
                                        ));
                                        custom_block_indices.extend(info.indices.iter().copied());
                                        AddedCustomBlock {
                                            start_vertex,
                                            start_index,
                                            num_indices,
                                            instance_group: Vec::new(),
                                        }
                                    });
                                // assert_eq!(blockstate_info.identifier, added_block.identifier);
                                added_block.instance_group.push(custom_block::Instance {
                                    pos: [global_x, global_y, global_z],
                                    matrix_indices: [x_rot_mat, y_rot_mat, 0, 0],
                                    tint_color,
                                });
                            }
                            _ => {}
                        }
                    }
                }
            }
            // Generate custom block instance and draw command buffers
            let custom_block_info = if added_custom_blocks.len() > 0 {
                let mut custom_block_instances = Vec::new();
                let mut custom_block_draw_args = Vec::new();
                for custom_block in added_custom_blocks.into_values() {
                    let start_instance: u32 = custom_block_instances.len().try_into().unwrap();
                    let num_instances: u32 = custom_block.instance_group.len().try_into().unwrap();
                    custom_block_instances.extend(custom_block.instance_group);
                    custom_block_draw_args.push(
                        graphics::chunk::custom_block::DrawIndexedIndirectArgs {
                            num_indices: custom_block.num_indices,
                            num_instances,
                            start_index: custom_block.start_index,
                            start_vertex: custom_block.start_vertex,
                            start_instance,
                        },
                    );
                }
                Some(graphics::chunk::SubchunkCustomBlockInfo {
                    vertices: graphics::chunk::custom_block::VertexList::new(
                        &graphics_state.resources.device,
                        &custom_block_vertices,
                    ),
                    indices: graphics::chunk::custom_block::IndexList::new(
                        &graphics_state.resources.device,
                        &custom_block_indices,
                    ),
                    instances: graphics::chunk::custom_block::InstanceList::new(
                        &graphics_state.resources.device,
                        &custom_block_instances,
                    ),
                    draw_args: graphics::chunk::custom_block::DrawArgsList::new(
                        &graphics_state.resources.device,
                        &custom_block_draw_args,
                    ),
                })
            } else {
                None
            };
            // Runs a variant of Minecraft's cave culling algorithm connected face generation.
            // Documented here: https://tomcc.github.io/2014/08/31/visibility-1.html
            let connected_faces = 'connected_faces: {
                //dbg!(chunk_x, chunk_z, section_i);
                // TODO: Fix this, seems to be pretty wrong
                use super::Palette;
                use crate::basic_types::AxisDirection;
                use graphics::chunk::SubchunkConnectivity;
                use std::collections::VecDeque;
                // If we can immediately tell all the subchunk blocks are opaque, skip this entire
                // process and just return that no subchunk faces are connected.
                match chunk_section.block_states.palette() {
                    Palette::SingleValue(index) => {
                        let blockstate_info = &graphics_state.block_registry.global_palette
                            [usize::from(*index)];
                        let block_info = &graphics_state
                            .block_registry[blockstate_info.block_index];
                        break 'connected_faces match block_info.properties.opaque {
                            true => SubchunkConnectivity::empty(),
                            false => SubchunkConnectivity::full(),
                        };
                    }
                    Palette::Palette(indices) => {
                        let mut all_opaque = true;
                        let mut all_non_opaque = true;
                        let mut num_opaque = 0;
                        for index in indices {
                            let blockstate_info = &graphics_state.block_registry.global_palette
                                [usize::from(*index)];
                            let block_info = &graphics_state
                                .block_registry[blockstate_info.block_index];
                            if block_info.properties.opaque {
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
                let mut unchecked_blocks: AHashSet<[i8; 3]> = AHashSet::new();
                let mut group_faces: Vec<FaceSet> = Vec::new();
                // Add all non-opaque blocks
                for x in 0..AXIS_LEN {
                    for y in 0..AXIS_LEN {
                        for z in 0..AXIS_LEN {
                            let global_palette_index = chunk_section.block_states.get(x, y, z);
                            let blockstate_info = &graphics_state.block_registry.global_palette
                                [usize::from(global_palette_index)];
                            let block_info = &graphics_state
                                .block_registry[blockstate_info.block_index];
                            if !block_info.properties.opaque {
                                let coords = [x as i8, y as i8, z as i8];
                                unchecked_blocks.insert(coords);
                            }
                        }
                    }
                }
                // Flood fill from each non-opaque block, to split all the blocks into groups.
                let mut queue: AHashSet<[i8; 3]> = AHashSet::new();
                while !queue.is_empty() || !unchecked_blocks.is_empty() {
                    let [x, y, z] = queue.iter()
                        .copied()
                        .next()
                        .map(|coord| {
                            queue.remove(&coord);
                            coord
                        })
                        .unwrap_or_else(|| {
                            // No more blocks in queue, make a new group and grab a new block
                            // that hasn't been checked yet.
                            let coord = *unchecked_blocks.iter().next().unwrap();
                            group_faces.push(current_group_faces);
                            current_group += 1;
                            current_group_faces = FaceSet::empty();
                            coord
                        });
                    unchecked_blocks.remove(&[x, y, z]);
                    let surrounding_block_coords = [
                        [x - 1, y, z],
                        [x + 1, y, z],
                        [x, y - 1, z],
                        [x, y + 1, z],
                        [x, y, z - 1],
                        [x, y, z + 1],
                    ];
                    for new_coord in surrounding_block_coords {
                        let [new_x, new_y, new_z] = new_coord;
                        // If fill escapes subchunk, add escaping face to group
                        if new_x < 0 {
                            current_group_faces.add_dir(AxisDirection::West);
                        } else if new_x >= AXIS_LEN as i8 {
                            current_group_faces.add_dir(AxisDirection::East);
                        } else if new_y < 0 {
                            current_group_faces.add_dir(AxisDirection::Down);
                        } else if new_y >= AXIS_LEN as i8 {
                            current_group_faces.add_dir(AxisDirection::Up);
                        } else if new_z < 0 {
                            current_group_faces.add_dir(AxisDirection::North);
                        } else if new_z >= AXIS_LEN as i8 {
                            current_group_faces.add_dir(AxisDirection::South);
                        } else if unchecked_blocks.contains(&new_coord) {
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
                    //for i in 0..directions.len() - 1 {
                    //    let (face_1, face_1_in_set) = directions[i];
                    //    if !face_1_in_set {
                    //        continue;
                    //    }
                    //    for j in i..directions.len() {
                    //        let (face_2, face_2_in_set) = directions[j];
                    //        if !face_2_in_set {
                    //            continue;
                    //        }
                    //        subchunk_connectivity.add_connection(&face_1, &face_2);
                    //    }
                    //}
                }
                subchunk_connectivity
            };
            subchunks.insert([chunk_x, section_i as i32, chunk_z], graphics::chunk::Subchunk {
                start_coords: [
                    AXIS_LEN_I32 * chunk_x,
                    AXIS_LEN_I32 * section_i as i32 + MIN_HEIGHT_I32,
                    AXIS_LEN_I32 * chunk_z,
                ],
                block_faces: graphics::chunk::block_face::InstanceList::new(
                    &graphics_state.resources.device,
                    &block_faces,
                ),
                tinted_block_faces: graphics::chunk::tinted_block_face::InstanceList::new(
                    &graphics_state.resources.device,
                    &tinted_block_faces,
                ),
                custom_block_info,
                connected_faces,
            });
        }
    }
    println!(
        "Processed {} chunks in {}s",
        loaded_chunks.len(),
        (std::time::Instant::now() - chunk_gen_start_time).as_secs_f64()
    );
    if true {
        std::process::exit(0);
    }
    let mut last_mouse_pos = egui::Pos2::new(0.0, 0.0);
    let mut events: Vec<egui::Event> = Vec::new();
    let mut subchunks_skipped = 0;
    let mut subchunk_traversal_graph: Vec<([i32; 3], [i32; 3])> = Vec::new();
    let mut debug_state = graphics::DebugState {
        cull_planes_active: 6,
        rendering_view_frustum: false,
        cull_camera_moving_with_player: true,
        cull_camera: graphics_state.camera,
        cave_cull_check_not_backwards: false,
        cave_cull_check_frustum: true,
        cave_cull_check_connectivity: true,
        cave_cull_render_connectivity: false,
        cave_cull_render_traversal_graph: false,
        cave_cull_debug_render_dist: 24.0,
        max_render_chunks: 3000,
    };
    let mut last_frame_time = Instant::now();
    let mut current_time_s: f64 = 0.0;
    event_loop.run(move |event, window_target| {
        window_target.set_control_flow(ControlFlow::Poll);
        match event {
            Event::NewEvents(StartCause::Poll) => {
                let new_time = Instant::now();
                let delta_time_f64 =
                    (new_time - std::mem::replace(&mut last_frame_time, new_time)).as_secs_f64();
                current_time_s += delta_time_f64;
                let delta_time = delta_time_f64 as f32;
                input_state.update_camera(&mut graphics_state.camera, delta_time);
                // GUI
                let egui_full_output = {
                    let raw_input = egui::RawInput {
                        viewport_id: egui::viewport::ViewportId::ROOT,
                        viewports: {
                            let mut map = egui::viewport::ViewportIdMap::default();
                            map.insert(
                                egui::viewport::ViewportId::ROOT,
                                egui::ViewportInfo {
                                    native_pixels_per_point: Some(scale_factor as f32),
                                    ..Default::default()
                                },
                            );
                            map
                        },
                        screen_rect: Some(egui::Rect {
                            min: egui::Pos2::ZERO,
                            max: egui::Pos2::new(
                                graphics_state.size.width as f32 / scale_factor as f32,
                                graphics_state.size.height as f32 / scale_factor as f32,
                            ),
                        }),
                        time: Some(current_time_s),
                        predicted_dt: delta_time,
                        events: std::mem::take(&mut events),
                        ..Default::default()
                    };
                    egui_ctx.run(raw_input, |ctx| {
                        use egui::*;
                        let width_f32 = graphics_state.size.width as f32 / scale_factor as f32;
                        let height_f32 = graphics_state.size.height as f32 / scale_factor as f32;
                        let painter =
                            Painter::new(ctx.clone(), LayerId::background(), Rect::EVERYTHING);
                        Window::new("Debug Info").resizable(false).show(&ctx, |ui| {
                            ui.label(format!("FPS: {:.2}", 1.0 / delta_time_f64));
                            // VSync
                            {
                                let graphics_options = &mut graphics_state.graphics_options;
                                let old_vsync = graphics_options.vsync;
                                ui.checkbox(&mut graphics_options.vsync, "VSync");
                                if graphics_options.vsync != old_vsync {
                                    graphics_state.config.present_mode =
                                        graphics_options.get_present_mode();
                                    graphics_state.resources.surface.configure(
                                        &graphics_state.resources.device,
                                        &graphics_state.config,
                                    );
                                }
                            }
                            ui.label(format!("Position: {:.2?}", graphics_state.camera.pos));
                            ui.label(format!("Subchunks Skipped: {subchunks_skipped}"));
                            ui.add(
                                Slider::new(&mut debug_state.cull_planes_active, 0..=6)
                                    .text("Planes active"),
                            );
                            ui.add(
                                Slider::new(&mut debug_state.max_render_chunks, 0..=3000)
                                    .drag_value_speed(1.0)
                                    .clamp_to_range(false)
                                    .text("Max render chunks"),
                            );
                            ui.checkbox(
                                &mut debug_state.rendering_view_frustum,
                                "Render view frustum",
                            );
                            ui.checkbox(
                                &mut debug_state.cull_camera_moving_with_player,
                                "Move cull camera with player camera",
                            );
                            ui.collapsing("Cave Culling", |ui| {
                                ui.checkbox(
                                    &mut debug_state.cave_cull_check_not_backwards,
                                    "Backwards check",
                                );
                                ui.checkbox(
                                    &mut debug_state.cave_cull_check_frustum,
                                    "Frustum check",
                                );
                                ui.checkbox(
                                    &mut debug_state.cave_cull_check_connectivity,
                                    "Connectivity check",
                                );
                                ui.checkbox(
                                    &mut debug_state.cave_cull_render_connectivity,
                                    "Render subchunk connectivity lines",
                                );
                                ui.checkbox(
                                    &mut debug_state.cave_cull_render_traversal_graph,
                                    "Render subchunk traversal graph",
                                );
                                ui.add(
                                    Slider::new(
                                        &mut debug_state.cave_cull_debug_render_dist,
                                        0.0..=64.0
                                    )
                                    .clamp_to_range(false)
                                    .text("Render distance")
                                );
                            });
                            ui.collapsing("Block Info", |ui| {
                                let pos = graphics_state.camera.pos.coords;
                                let chunk_x = (pos.x.floor() as i32).div_euclid(16);
                                let chunk_z = (pos.z.floor() as i32).div_euclid(16);
                                let section_i = ((pos.y.floor() + 64.0).div_euclid(16.0)) as usize;
                                let x = (pos.x.floor() as i32).rem_euclid(16) as usize;
                                let y = (pos.y.floor() as i32).rem_euclid(16) as usize;
                                let z = (pos.z.floor() as i32).rem_euclid(16) as usize;
                                if let Some(chunk) = chunks.get(&(chunk_x, chunk_z)) {
                                    if pos.y < 0.0 || section_i >= chunk.len() {
                                        ui.label("Global Palette ID: N/A");
                                        ui.label("Blockstate data: N/A");
                                    } else {
                                        let chunk_section = &chunk[section_i];
                                        let global_palette_index =
                                            chunk_section.block_states.get(x, y, z);
                                        let blockstate = &graphics_state
                                            .block_registry
                                            .global_palette[usize::from(global_palette_index)];
                                        ui.label(format!(
                                            "Global Palette ID: {}",
                                            global_palette_index.as_raw()
                                        ));
                                        ui.label(format!("Blockstate data: {blockstate:?}"));
                                    }
                                } else {
                                    ui.label("Global Palette ID: N/A");
                                    ui.label("Blockstate data: N/A");
                                }
                            });
                            ui.collapsing("Chunk Info", |ui| {
                                const MIN_HEIGHT_I32: i32 = -64;
                                let pos = graphics_state.camera.pos;
                                let x = (pos.x.floor() as i32).div_euclid(16);
                                let y = (pos.y.floor() as i32 - MIN_HEIGHT_I32).div_euclid(16);
                                let z = (pos.z.floor() as i32).div_euclid(16);
                                ui.label(format!("{x}, {y}, {z}"));
                                if let Some(subchunk) = subchunks.get(&[x, y, z]) {
                                    ui.label(format!(
                                        "Subchunk connectivity info: {:?}",
                                        subchunk.connected_faces,
                                    ));
                                } else {
                                    ui.label("Subchunk connectivity info: N/A");
                                }
                            });
                        });
                        if debug_state.cull_camera_moving_with_player {
                            debug_state.cull_camera = graphics_state.camera;
                        }
                        if debug_state.rendering_view_frustum {
                            use nalgebra::{Point3, Vector3};
                            let cull_camera = debug_state.cull_camera;
                            let graphics_camera = &graphics_state.camera;
                            let inv_cull_view_mat =
                                cull_camera.generate_view_matrix().try_inverse().unwrap();
                            let plane_point_groups: [[Point3<f32>; 4]; 6] = [
                                [
                                    Point3::new(-1.0, 1.0, 0.9999),
                                    Point3::new(-1.0, 1.0, -1.0),
                                    Point3::new(-1.0, -1.0, -1.0),
                                    Point3::new(-1.0, -1.0, 0.9999),
                                ],
                                [
                                    Point3::new(1.0, 1.0, 0.9999),
                                    Point3::new(1.0, 1.0, -1.0),
                                    Point3::new(1.0, -1.0, -1.0),
                                    Point3::new(1.0, -1.0, 0.9999),
                                ],
                                [
                                    Point3::new(-1.0, -1.0, 0.9999),
                                    Point3::new(-1.0, -1.0, -1.0),
                                    Point3::new(1.0, -1.0, -1.0),
                                    Point3::new(1.0, -1.0, 0.9999),
                                ],
                                [
                                    Point3::new(-1.0, 1.0, 0.9999),
                                    Point3::new(-1.0, 1.0, -1.0),
                                    Point3::new(1.0, 1.0, -1.0),
                                    Point3::new(1.0, 1.0, 0.9999),
                                ],
                                [
                                    Point3::new(-1.0, 1.0, -1.0),
                                    Point3::new(1.0, 1.0, -1.0),
                                    Point3::new(1.0, -1.0, -1.0),
                                    Point3::new(-1.0, -1.0, -1.0),
                                ],
                                [
                                    Point3::new(-1.0, 1.0, 0.9999),
                                    Point3::new(1.0, 1.0, 0.9999),
                                    Point3::new(1.0, -1.0, 0.9999),
                                    Point3::new(-1.0, -1.0, 0.9999),
                                ],
                            ]
                            .map(|group| group.map(|p| inv_cull_view_mat.transform_point(&p)));
                            let test_colors = [
                                Color32::from_rgba_unmultiplied(0xFF, 0x00, 0x00, 127),
                                Color32::from_rgba_unmultiplied(0x00, 0xFF, 0x00, 127),
                                Color32::from_rgba_unmultiplied(0x00, 0x00, 0xFF, 127),
                                Color32::from_rgba_unmultiplied(0xFF, 0xFF, 0x00, 127),
                                Color32::from_rgba_unmultiplied(0x00, 0xFF, 0xFF, 127),
                                Color32::from_rgba_unmultiplied(0xFF, 0x00, 0xFF, 127),
                            ];
                            for (points, color) in plane_point_groups.into_iter().zip(test_colors) {
                                painter.add(Shape::convex_polygon(
                                    debug_clip_and_project_points(&points, graphics_camera)
                                        .iter()
                                        .copied()
                                        .map(|p| {
                                            Pos2::new(
                                                (p.x + 1.0) / 2.0 * width_f32,
                                                (-p.y + 1.0) / 2.0 * height_f32,
                                            )
                                        })
                                        .collect(),
                                    color,
                                    Stroke::NONE,
                                ));
                            }
                        }
                        if debug_state.cave_cull_render_connectivity {
                            use nalgebra::{Point3, Vector3};
                            let cull_camera = debug_state.cull_camera;
                            let graphics_camera = &graphics_state.camera;
                            let colours = [
                                Color32::GRAY,
                                Color32::LIGHT_GRAY,
                                Color32::WHITE,
                                Color32::BROWN,
                                Color32::DARK_RED,
                                Color32::RED,
                                Color32::LIGHT_RED,
                                Color32::YELLOW,
                                Color32::LIGHT_YELLOW,
                                Color32::KHAKI,
                                Color32::GREEN,
                                Color32::LIGHT_GREEN,
                                Color32::BLUE,
                                Color32::LIGHT_BLUE,
                                Color32::GOLD,
                                //Color32::from_rgba_unmultiplied(0xFF, 0x00, 0x00, 0xFF),
                                //Color32::from_rgba_unmultiplied(0x00, 0xFF, 0x00, 0xFF),
                                //Color32::from_rgba_unmultiplied(0x00, 0x00, 0xFF, 0xFF),
                                //Color32::from_rgba_unmultiplied(0xFF, 0xFF, 0x00, 0xFF),
                                //Color32::from_rgba_unmultiplied(0x00, 0xFF, 0xFF, 0xFF),
                            ];
                            let offset_positions = [
                                // Down
                                Vector3::new(0.0, 0.0, 0.0),
                                Vector3::new(-0.1, -0.1, -0.1),
                                Vector3::new(0.1, 0.1, 0.1),
                                Vector3::new(-0.2, -0.2, -0.2),
                                Vector3::new(0.2, 0.2, 0.2),
                                // Up
                                Vector3::new(0.1, 0.1, 0.1),
                                Vector3::new(-0.1, -0.1, -0.1),
                                Vector3::new(0.2, 0.2, 0.2),
                                Vector3::new(-0.2, -0.2, -0.2),
                                // North
                                Vector3::new(0.0, 0.0, 0.0),
                                Vector3::new(-0.3, -0.3, -0.3),
                                Vector3::new(0.3, 0.3, 0.3),
                                // South
                                Vector3::new(0.3, 0.3, 0.3),
                                Vector3::new(-0.3, -0.3, -0.3),
                                // West
                                Vector3::new(0.0, 0.0, 0.0),
                            ];
                            for subchunk in subchunks.values() {
                                let pairs = subchunk.connected_faces.get_pairs();
                                let subchunk_centre =
                                    subchunk.start_coords.map(|n| (n + 8) as f32);
                                let subchunk_centre = Point3::new(
                                    subchunk_centre[0],
                                    subchunk_centre[1],
                                    subchunk_centre[2],
                                );
                                if (graphics_camera.pos - subchunk_centre).magnitude() > debug_state.cave_cull_debug_render_dist {
                                    continue;
                                }
                                // Render bounding box
                                {
                                    let start = Point3::new(
                                        subchunk.start_coords[0] as f32,
                                        subchunk.start_coords[1] as f32,
                                        subchunk.start_coords[2] as f32,
                                    );
                                    let end = Point3::new(
                                        start.x + 16.0,
                                        start.y + 16.0,
                                        start.z + 16.0,
                                    );
                                    let corners = [
                                        Point3::new(start.x, start.y, start.z),
                                        Point3::new(end.x, start.y, start.z),
                                        Point3::new(end.x, start.y, end.z),
                                        Point3::new(start.x, start.y, end.z),
                                        Point3::new(start.x, end.y, start.z),
                                        Point3::new(end.x, end.y, start.z),
                                        Point3::new(end.x, end.y, end.z),
                                        Point3::new(start.x, end.y, end.z),
                                    ];
                                    let lines = [
                                        // Bottom
                                        [corners[0], corners[1]],
                                        [corners[1], corners[2]],
                                        [corners[2], corners[3]],
                                        [corners[3], corners[0]],
                                        // Connectors
                                        [corners[0], corners[4]],
                                        [corners[1], corners[5]],
                                        [corners[2], corners[6]],
                                        [corners[3], corners[7]],
                                        // Top
                                        [corners[4], corners[5]],
                                        [corners[5], corners[6]],
                                        [corners[6], corners[7]],
                                        [corners[7], corners[4]],
                                    ];
                                    for line in lines {
                                        let Some(line) = debug_clip_and_project_line(line, graphics_camera) else {
                                            continue;
                                        };
                                        painter.add(Shape::line_segment(
                                            line.map(|p| Pos2::new(
                                                (p.x + 1.0) / 2.0 * width_f32,
                                                (-p.y + 1.0) / 2.0 * height_f32,
                                            )),
                                            (3.0, Color32::from_rgba_unmultiplied(0xFF, 0x00, 0xFF, 0xFF)),
                                        ));
                                    }
                                }
                                for (i, ([dir_1, dir_2], pair_connected)) in pairs.into_iter().enumerate() {
                                    if !pair_connected {
                                        continue;
                                    }
                                    let colour = colours[i];
                                    let offset_pos = offset_positions[i];
                                    let pair_centre = subchunk_centre + offset_pos;
                                    let end_1 = pair_centre + dir_1.as_vector() * 8.0;
                                    let end_2 = pair_centre + dir_2.as_vector() * 8.0;
                                    if let Some(line_1) = debug_clip_and_project_line([pair_centre, end_1], graphics_camera) {
                                        painter.add(Shape::line_segment(
                                            line_1.map(|p| Pos2::new(
                                                (p.x + 1.0) / 2.0 * width_f32,
                                                (-p.y + 1.0) / 2.0 * height_f32,
                                            )),
                                            (1.5, colour),
                                        ));
                                    }
                                    if let Some(line_2) = debug_clip_and_project_line([pair_centre, end_2], graphics_camera) {
                                        painter.add(Shape::line_segment(
                                            line_2.map(|p| Pos2::new(
                                                (p.x + 1.0) / 2.0 * width_f32,
                                                (-p.y + 1.0) / 2.0 * height_f32,
                                            )),
                                            (1.5, colour),
                                        ));
                                    }
                                }
                            }
                        }
                        if debug_state.cave_cull_render_traversal_graph {
                            use nalgebra::{Point3, Vector3};
                            let cull_camera = debug_state.cull_camera;
                            let graphics_camera = &graphics_state.camera;
                            for (from_chunk, to_chunk) in &subchunk_traversal_graph {
                                let chunks = [from_chunk, to_chunk];
                                let chunk_centres = chunks.map(|chunk_coords| Point3::new(
                                    (chunk_coords[0] * 16 + 8) as f32,
                                    (chunk_coords[1] * 16 - 64 + 8) as f32,
                                    (chunk_coords[2] * 16 + 8) as f32,
                                ));
                                let max_dist = debug_state.cave_cull_debug_render_dist;
                                let from_centre_dist = (graphics_camera.pos - chunk_centres[0]).magnitude();
                                let to_centre_dist = (graphics_camera.pos - chunk_centres[1]).magnitude();
                                if from_centre_dist > max_dist || to_centre_dist > max_dist {
                                    continue;
                                }
                                let average_dist = (from_centre_dist + to_centre_dist) / 2.0;
                                let alpha = (1.0 - (average_dist / max_dist.max(0.01))).max(0.0);
                                if let Some(line) = debug_clip_and_project_line(chunk_centres, graphics_camera) {
                                    painter.add(Shape::line_segment(
                                        line.map(|p| Pos2::new(
                                            (p.x + 1.0) / 2.0 * width_f32,
                                            (-p.y + 1.0) / 2.0 * height_f32,
                                        )),
                                        (5.0 * alpha, Color32::YELLOW.gamma_multiply(alpha)),
                                    ));
                                }
                            }
                        }
                    })
                };
                // Main rendering
                match graphics_state.render(
                    &subchunks,
                    &loaded_chunks,
                    &egui_ctx,
                    egui_full_output,
                    &debug_state,
                ) {
                    Ok(graphics::DebugOutput {
                        subchunks_culled: new_subchunks_skipped,
                        subchunk_traversal_graph: new_subchunk_traversal_graph,
                    }) => {
                        subchunks_skipped = new_subchunks_skipped;
                        subchunk_traversal_graph = new_subchunk_traversal_graph;
                    },
                    Err(wgpu::SurfaceError::Timeout) => {}
                    // Reconfigure the surface if lost
                    Err(wgpu::SurfaceError::Lost) | Err(wgpu::SurfaceError::Outdated) => {
                        let size = graphics_state.size;
                        graphics_state.resize(size)
                    }
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory) => window_target.exit(),
                }
            }
            Event::WindowEvent {
                window_id: event_window_id,
                ref event,
            } if event_window_id == window_id => match event {
                WindowEvent::CloseRequested | WindowEvent::Destroyed => window_target.exit(),
                WindowEvent::Resized(physical_size) => graphics_state.resize(*physical_size),
                WindowEvent::ScaleFactorChanged {
                    scale_factor: new_scale_factor,
                    inner_size_writer: _,
                } => scale_factor = *new_scale_factor,
                WindowEvent::KeyboardInput {
                    device_id: _,
                    event,
                    is_synthetic,
                } if !is_synthetic => input_state.update_from_input(event),
                WindowEvent::CursorMoved {
                    device_id: _,
                    position,
                } => {
                    let egui_pos = egui::Pos2 {
                        x: (position.x / scale_factor) as f32,
                        y: (position.y / scale_factor) as f32,
                    };
                    last_mouse_pos = egui_pos;
                    events.push(egui::Event::PointerMoved(egui_pos));
                }
                WindowEvent::CursorLeft { device_id: _ } => events.push(egui::Event::PointerGone),
                WindowEvent::MouseInput {
                    device_id: _,
                    state,
                    button,
                } => events.push(egui::Event::PointerButton {
                    pos: last_mouse_pos,
                    button: match button {
                        winit::event::MouseButton::Left => egui::PointerButton::Primary,
                        winit::event::MouseButton::Right => egui::PointerButton::Secondary,
                        winit::event::MouseButton::Middle => egui::PointerButton::Middle,
                        winit::event::MouseButton::Back => egui::PointerButton::Extra1,
                        winit::event::MouseButton::Forward => egui::PointerButton::Extra2,
                        _ => return,
                    },
                    pressed: state.is_pressed(),
                    modifiers: egui::Modifiers::NONE,
                }),
                _ => {}
            },
            _ => {}
        }
    })?;
    Ok(())
}

use graphics::Camera;
use nalgebra::{Matrix4, Point3};

fn debug_clip_and_project_points(points: &[Point3<f32>], camera: &Camera) -> Vec<Point3<f32>> {
    fn clip_intersection_x(p1: Point3<f32>, p2: Point3<f32>, edge: f32) -> Point3<f32> {
        let diff = p2.x - p1.x;
        let grads = (p2.yz() - p1.yz()) / diff;
        let new = p1.yz() - (grads * (p1.x - edge));
        Point3::new(edge, new.x, new.y)
    }
    fn clip_intersection_y(p1: Point3<f32>, p2: Point3<f32>, edge: f32) -> Point3<f32> {
        let diff = p2.y - p1.y;
        let grad_x = (p2.x - p1.x) / diff;
        let grad_z = (p2.z - p1.z) / diff;
        let edge_dist = p1.y - edge;
        Point3::new(
            p1.x - (grad_x * edge_dist),
            edge,
            p1.z - (grad_z * edge_dist),
        )
    }
    fn clip_intersection_z(p1: Point3<f32>, p2: Point3<f32>, edge: f32) -> Point3<f32> {
        let diff = p2.z - p1.z;
        let grad_x = (p2.x - p1.x) / diff;
        let grad_y = (p2.y - p1.y) / diff;
        let edge_dist = p1.z - edge;
        Point3::new(
            p1.x - (grad_x * edge_dist),
            p1.y - (grad_y * edge_dist),
            edge,
        )
    }
    let mut vec1: Vec<Point3<f32>> = points
        .iter()
        .copied()
        .map(|p| {
            let translated = nalgebra::Isometry3::new(camera.pos.coords, nalgebra::zero())
                .inverse()
                .transform_point(&p);
            camera.get_rot().inverse().transform_point(&translated)
        })
        .collect();
    let mut vec2: Vec<Point3<f32>> = Vec::new();
    macro_rules! clip_edge {
        ($op1:tt, $op2:tt, $axis:ident, $func:expr, $edge:expr) => {{
            for (i, point) in vec1.iter().copied().enumerate() {
                let prev_point = vec1[(i as isize - 1).rem_euclid(vec1.len() as isize) as usize];
                if point.$axis $op1 $edge {
                    if prev_point.$axis $op2 $edge {
                        vec2.push($func(prev_point, point, $edge));
                    }
                    vec2.push(point);
                } else if prev_point.$axis $op1 $edge {
                    vec2.push($func(prev_point, point, $edge));
                }
            }
            std::mem::swap(&mut vec1, &mut vec2);
            vec2.clear();
        }};
    }
    macro_rules! clip_edge_pair {
        ($axis:ident, $func:expr, $low_edge:expr, $high_edge:expr) => {
            clip_edge!(<, >=, $axis, $func, $high_edge);
            clip_edge!(>, <=, $axis, $func, $low_edge);
        };
    }
    // Clipping after projection seems to completely mess things up, so we have to do this instead
    clip_edge_pair!(
        z,
        clip_intersection_z,
        -GraphicsState::DEFAULT_ZFAR,
        -GraphicsState::DEFAULT_ZNEAR
    );
    for point in &mut vec1 {
        *point = camera.proj_matrix.transform_point(point);
    }
    clip_edge_pair!(x, clip_intersection_x, -1.0, 1.0);
    clip_edge_pair!(y, clip_intersection_y, -1.0, 1.0);
    vec1
}

fn debug_clip_and_project_line(
    line: [Point3<f32>; 2],
    camera: &Camera,
) -> Option<[Point3<f32>; 2]> {
    fn clip_intersection_x(p1: Point3<f32>, p2: Point3<f32>, edge: f32) -> Point3<f32> {
        let diff = p2.x - p1.x;
        let grads = (p2.yz() - p1.yz()) / diff;
        let new = p1.yz() - (grads * (p1.x - edge));
        Point3::new(edge, new.x, new.y)
    }
    fn clip_intersection_y(p1: Point3<f32>, p2: Point3<f32>, edge: f32) -> Point3<f32> {
        let diff = p2.y - p1.y;
        let grad_x = (p2.x - p1.x) / diff;
        let grad_z = (p2.z - p1.z) / diff;
        let edge_dist = p1.y - edge;
        Point3::new(
            p1.x - (grad_x * edge_dist),
            edge,
            p1.z - (grad_z * edge_dist),
        )
    }
    fn clip_intersection_z(p1: Point3<f32>, p2: Point3<f32>, edge: f32) -> Point3<f32> {
        let diff = p2.z - p1.z;
        let grad_x = (p2.x - p1.x) / diff;
        let grad_y = (p2.y - p1.y) / diff;
        let edge_dist = p1.z - edge;
        Point3::new(
            p1.x - (grad_x * edge_dist),
            p1.y - (grad_y * edge_dist),
            edge,
        )
    }
    let mut line = line.map(|p| {
        let translated = nalgebra::Isometry3::new(camera.pos.coords, nalgebra::zero())
            .inverse()
            .transform_point(&p);
        camera.get_rot().inverse().transform_point(&translated)
    });
    macro_rules! clip_edge {
        ($op:tt, $axis:ident, $func:expr, $edge:expr) => {{
            line = match (line[0].$axis $op $edge, line[1].$axis $op $edge) {
                (false, false) => return None,
                (false, true) => [$func(line[0], line[1], $edge), line[1]],
                (true, false) => [line[0], $func(line[1], line[0], $edge)],
                (true, true) => line,
            };
        }};
    }
    macro_rules! clip_edge_pair {
        ($axis:ident, $func:expr, $low_edge:expr, $high_edge:expr) => {
            clip_edge!(<, $axis, $func, $high_edge);
            clip_edge!(>, $axis, $func, $low_edge);
        };
    }
    // Clipping after projection seems to completely mess things up, so we have to do this instead
    clip_edge_pair!(
        z,
        clip_intersection_z,
        -GraphicsState::DEFAULT_ZFAR,
        -GraphicsState::DEFAULT_ZNEAR
    );
    line = line.map(|p| camera.proj_matrix.transform_point(&p));
    clip_edge_pair!(x, clip_intersection_x, -1.0, 1.0);
    clip_edge_pair!(y, clip_intersection_y, -1.0, 1.0);
    Some(line)
}
