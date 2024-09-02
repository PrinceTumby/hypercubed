pub mod graphics;
pub mod input;
pub mod world;

use crate::identifier;
use crate::protocol::chunk as protocol_chunk;
use crate::protocol::play::{
    self as protocol_play, serverbound as serverbound_packets, Clientbound as ClientboundPacket,
};
use crate::protocol::prelude::*;
use crate::resource::block::GlobalPaletteIndex;
use ahash::{AHashMap, AHashSet};
use graphics::GraphicsState;
use indexmap::IndexMap;
use input::PlayControlState;
use std::sync::{mpsc, Arc};
use std::time::Instant;
use threadpool::ThreadPool;
use winit::event::{Event, StartCause, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

pub struct ClientPlayState {
    pub raw_chunks: Arc<AHashMap<[i32; 2], Arc<RawChunk>>>,
    // TODO: Currently the Y coordinate is a chunk section index, rather than the subchunk Y
    //       coordinate. Consider changing to actually be the Y coordinate.
    pub subchunks: AHashMap<[i32; 3], graphics::chunk::Subchunk>,
    pub visible_chunks: AHashSet<[i32; 2]>,
    pub pending_subchunk_update_ids: AHashMap<[i32; 3], usize>,
    pub player_entity_id: EntityId,
}

#[derive(Clone)]
pub struct RawChunk {
    pub sections: Box<[protocol_chunk::ChunkSection]>,
    pub lighting: protocol_chunk::ChunkLightInfo,
}

#[derive(Debug)]
pub enum ClientPlayStateUpdate {
    RemoveChunk([i32; 2]),
    PlaceSubchunks {
        update_id: usize,
        new_raw_subchunks: Vec<([i32; 3], RawSubchunk)>,
    },
}

#[derive(Debug)]
pub struct RawSubchunk {
    pub start_coords: [i32; 3],
    pub block_face_quads: [Option<[graphics::chunk::block_face::Vertex; 4]>; 6],
    pub block_face_instance_groups: [Vec<graphics::chunk::block_face::Instance>; 6],
    pub tinted_block_face_quads: [Option<[graphics::chunk::tinted_block_face::Vertex; 4]>; 6],
    pub tinted_block_face_instance_groups: [Vec<graphics::chunk::tinted_block_face::Instance>; 6],
    pub custom_block_groups: Vec<RawCustomBlockGroup>,
    pub connected_faces: graphics::chunk::SubchunkConnectivity,
}

#[derive(Debug)]
pub struct RawCustomBlockGroup {
    pub start_vertex: u32,
    pub start_index_and_len: (u32, u32),
    pub instances: Vec<graphics::chunk::custom_block::Instance>,
}

pub const SUBCHUNK_AXIS_LEN: usize = 16;
pub const SUBCHUNK_AXIS_LEN_I32: i32 = SUBCHUNK_AXIS_LEN as i32;
pub const MIN_HEIGHT_I32: i32 = -64;
pub const MAX_HEIGHT_I32: i32 = 319;

pub(crate) async fn window_run(
    server_connection: Arc<PlayConnection>,
    clientbound_rx: std::sync::mpsc::Receiver<ClientboundPacket>,
    clientbound_tx: std::sync::mpsc::Sender<ClientboundPacket>,
) -> anyhow::Result<()> {
    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new().build(&event_loop)?;
    window.set_title("Rust Minecraft Client");
    let window_id = window.id();
    let mut scale_factor = window.scale_factor();
    let mut graphics_state =
        GraphicsState::new(window, crate::resource::block::register_vanilla_blocks).await?;
    let mut input_state = PlayControlState::default();
    let egui_ctx = egui::Context::default();
    let mut play_state = ClientPlayState {
        raw_chunks: Arc::new(AHashMap::new()),
        subchunks: AHashMap::new(),
        visible_chunks: AHashSet::new(),
        pending_subchunk_update_ids: AHashMap::new(),
        player_entity_id: EntityId::placeholder(),
    };
    let (play_state_update_tx, play_state_update_rx) = mpsc::channel::<ClientPlayStateUpdate>();
    let mut last_mouse_pos = egui::Pos2::new(0.0, 0.0);
    let mut events: Vec<egui::Event> = Vec::new();
    let mut debug_state = graphics::DebugState {
        cull_planes_active: 6,
        rendering_view_frustum: false,
        cull_camera_moving_with_player: true,
        cull_camera: graphics_state.camera,
        cave_cull_check_unflipped: true,
        cave_cull_check_not_backwards: false,
        cave_cull_check_frustum: true,
        cave_cull_check_connectivity: true,
        cave_cull_render_connectivity: false,
        cave_cull_render_traversal_graph: false,
        cave_cull_debug_render_dist: 24.0,
        max_render_chunks: 3000,
    };
    let mut debug_output = graphics::DebugOutput {
        subchunks_culled: 0,
        subchunk_traversal_graph: Vec::new(),
    };
    let thread_pool = ThreadPool::new(
        std::thread::available_parallelism()
            .map(|num_threads_non_zero| num_threads_non_zero.get())
            .unwrap_or(2),
    );
    let mut current_update_id: usize = 0;
    let mut previous_frame_times = std::collections::VecDeque::new();
    let mut last_frame_time = Instant::now();
    let mut current_time_s: f64 = 0.0;
    event_loop.run(move |event, window_target| {
        window_target.set_control_flow(ControlFlow::Poll);
        match event {
            Event::NewEvents(StartCause::Poll) => {
                let subchunks = &play_state.subchunks;
                let visible_chunks = &play_state.visible_chunks;
                let new_time = Instant::now();
                let delta_time_f64 =
                    (new_time - std::mem::replace(&mut last_frame_time, new_time)).as_secs_f64();
                previous_frame_times.push_back(delta_time_f64);
                if previous_frame_times.len() > 100 {
                    previous_frame_times.pop_front();
                }
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
                        Window::new("Debug Info").resizable(false).show(ctx, |ui| {
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
                            ui.label(format!(
                                "Subchunks Culled: {}",
                                debug_output.subchunks_culled
                            ));
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
                            let old_cull_camera_moving_with_player =
                                debug_state.cull_camera_moving_with_player;
                            ui.checkbox(
                                &mut debug_state.cull_camera_moving_with_player,
                                "Move cull camera with player camera",
                            );
                            if debug_state.cull_camera_moving_with_player
                                && !old_cull_camera_moving_with_player
                            {
                                // Reset camera position instead of sending a massive (invalid)
                                // movement request to the server.
                                graphics_state.camera.pos = debug_state.cull_camera.pos;
                                graphics_state.camera.yaw = debug_state.cull_camera.yaw;
                                graphics_state.camera.pitch = debug_state.cull_camera.pitch;
                                graphics_state.camera.roll = debug_state.cull_camera.roll;
                            }
                            ui.collapsing("Cave Culling", |ui| {
                                ui.checkbox(
                                    &mut debug_state.cave_cull_check_unflipped,
                                    "Flip check",
                                );
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
                                        0.0..=64.0,
                                    )
                                    .clamp_to_range(false)
                                    .text("Render distance"),
                                );
                            });
                            ui.collapsing("Block Info", |ui| {
                                let raw_chunks = &play_state.raw_chunks;
                                let pos = graphics_state.camera.pos.coords;
                                let chunk_x = (pos.x.floor() as i32).div_euclid(16);
                                let chunk_z = (pos.z.floor() as i32).div_euclid(16);
                                let section_i = ((pos.y.floor() + 64.0).div_euclid(16.0)) as usize;
                                let x = (pos.x.floor() as i32).rem_euclid(16) as usize;
                                let y = (pos.y.floor() as i32).rem_euclid(16) as usize;
                                let z = (pos.z.floor() as i32).rem_euclid(16) as usize;
                                if let Some(chunk) = raw_chunks.get(&[chunk_x, chunk_z]) {
                                    if pos.y < 0.0 || section_i >= chunk.sections.len() {
                                        ui.label("Global Palette ID: N/A");
                                        ui.label("Identifier: N/A");
                                        ui.label("Blockstate data: N/A");
                                    } else {
                                        let chunk_section = &chunk.sections[section_i];
                                        let global_palette_index =
                                            chunk_section.block_states.get(x, y, z);
                                        let blockstate = &graphics_state.resources.block_registry
                                            [global_palette_index];
                                        let identifier = graphics_state
                                            .resources
                                            .block_registry
                                            .get_identifier_from_index(blockstate.block_index)
                                            .unwrap();
                                        ui.label(format!(
                                            "Global Palette ID: {}",
                                            global_palette_index.as_raw()
                                        ));
                                        ui.label(format!("Identifier: {identifier:?}"));
                                        ui.label(format!("Blockstate data: {blockstate:?}"));
                                    }
                                } else {
                                    ui.label("Global Palette ID: N/A");
                                    ui.label("Blockstate data: N/A");
                                }
                            });
                            ui.collapsing("Chunk Info", |ui| {
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
                            ui.collapsing("Frametimes", |ui| {
                                use egui_plot::{Line, Plot, PlotPoints};
                                Plot::new("frame_time_plot")
                                    .allow_zoom(false)
                                    .allow_drag(false)
                                    .allow_scroll(false)
                                    .set_margin_fraction(egui::Vec2 { x: 0.0, y: 0.0 })
                                    .view_aspect(2.5)
                                    .include_y(0.0)
                                    .include_y(50.0)
                                    .show(ui, |plot_ui| {
                                        let points: PlotPoints = previous_frame_times
                                            .iter()
                                            .enumerate()
                                            .map(|(i, frame_time_s)| {
                                                let x = i as f64;
                                                let frame_time_ms = frame_time_s * 1000.0;
                                                [x, frame_time_ms]
                                            })
                                            .collect();
                                        let line = Line::new(points);
                                        plot_ui.line(line);
                                    });
                            });
                        });
                        if debug_state.cull_camera_moving_with_player {
                            debug_state.cull_camera = graphics_state.camera;
                        }
                        if debug_state.rendering_view_frustum {
                            use nalgebra::Point3;
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
                                let subchunk_centre = subchunk.start_coords.map(|n| (n + 8) as f32);
                                let subchunk_centre = Point3::new(
                                    subchunk_centre[0],
                                    subchunk_centre[1],
                                    subchunk_centre[2],
                                );
                                // Render bounding box
                                {
                                    let start = Point3::new(
                                        subchunk.start_coords[0] as f32,
                                        subchunk.start_coords[1] as f32,
                                        subchunk.start_coords[2] as f32,
                                    );
                                    let end =
                                        Point3::new(start.x + 16.0, start.y + 16.0, start.z + 16.0);
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
                                        let max_dist = debug_state.cave_cull_debug_render_dist;
                                        let end_1_dist =
                                            (graphics_camera.pos - line[0]).magnitude();
                                        let end_2_dist =
                                            (graphics_camera.pos - line[1]).magnitude();
                                        if end_1_dist > max_dist || end_2_dist > max_dist {
                                            continue;
                                        }
                                        let Some(line) =
                                            debug_clip_and_project_line(line, graphics_camera)
                                        else {
                                            continue;
                                        };
                                        let centre_dist =
                                            (graphics_camera.pos - subchunk_centre).magnitude();
                                        let alpha =
                                            (1.0 - (centre_dist / max_dist.max(0.01))).max(0.0);
                                        painter.add(Shape::line_segment(
                                            line.map(|p| {
                                                Pos2::new(
                                                    (p.x + 1.0) / 2.0 * width_f32,
                                                    (-p.y + 1.0) / 2.0 * height_f32,
                                                )
                                            }),
                                            (
                                                5.0 * alpha,
                                                Color32::from_rgba_unmultiplied(
                                                    0xFF, 0x00, 0xFF, 0xFF,
                                                )
                                                .gamma_multiply(alpha),
                                            ),
                                        ));
                                    }
                                }
                                for (i, ([dir_1, dir_2], pair_connected)) in
                                    pairs.into_iter().enumerate()
                                {
                                    if !pair_connected {
                                        continue;
                                    }
                                    let colour = colours[i];
                                    let offset_pos = offset_positions[i];
                                    let pair_centre = subchunk_centre + offset_pos;
                                    let max_dist = debug_state.cave_cull_debug_render_dist;
                                    let end_1 = pair_centre + dir_1.as_vector() * 8.0;
                                    let end_2 = pair_centre + dir_2.as_vector() * 8.0;
                                    let end_1_dist = (graphics_camera.pos - end_1).magnitude();
                                    let end_2_dist = (graphics_camera.pos - end_2).magnitude();
                                    if end_1_dist > max_dist || end_2_dist > max_dist {
                                        continue;
                                    }
                                    let average_dist = (end_1_dist + end_2_dist) / 2.0;
                                    let alpha =
                                        (1.0 - (average_dist / max_dist.max(0.01))).max(0.0);
                                    if let Some(line_1) = debug_clip_and_project_line(
                                        [pair_centre, end_1],
                                        graphics_camera,
                                    ) {
                                        painter.add(Shape::line_segment(
                                            line_1.map(|p| {
                                                Pos2::new(
                                                    (p.x + 1.0) / 2.0 * width_f32,
                                                    (-p.y + 1.0) / 2.0 * height_f32,
                                                )
                                            }),
                                            (5.0 * alpha, colour.gamma_multiply(alpha)),
                                        ));
                                    }
                                    if let Some(line_2) = debug_clip_and_project_line(
                                        [pair_centre, end_2],
                                        graphics_camera,
                                    ) {
                                        painter.add(Shape::line_segment(
                                            line_2.map(|p| {
                                                Pos2::new(
                                                    (p.x + 1.0) / 2.0 * width_f32,
                                                    (-p.y + 1.0) / 2.0 * height_f32,
                                                )
                                            }),
                                            (5.0 * alpha, colour.gamma_multiply(alpha)),
                                        ));
                                    }
                                }
                            }
                        }
                        if debug_state.cave_cull_render_traversal_graph {
                            use nalgebra::Point3;
                            let graphics_camera = &graphics_state.camera;
                            for (from_chunk, to_chunk) in &debug_output.subchunk_traversal_graph {
                                let chunks = [from_chunk, to_chunk];
                                let chunk_centres = chunks.map(|chunk_coords| {
                                    Point3::new(
                                        (chunk_coords[0] * 16 + 8) as f32,
                                        (chunk_coords[1] * 16 - 64 + 8) as f32,
                                        (chunk_coords[2] * 16 + 8) as f32,
                                    )
                                });
                                let max_dist = debug_state.cave_cull_debug_render_dist;
                                let from_centre_dist =
                                    (graphics_camera.pos - chunk_centres[0]).magnitude();
                                let to_centre_dist =
                                    (graphics_camera.pos - chunk_centres[1]).magnitude();
                                if from_centre_dist > max_dist || to_centre_dist > max_dist {
                                    continue;
                                }
                                let average_dist = (from_centre_dist + to_centre_dist) / 2.0;
                                let alpha = (1.0 - (average_dist / max_dist.max(0.01))).max(0.0);
                                if let Some(line) =
                                    debug_clip_and_project_line(chunk_centres, graphics_camera)
                                {
                                    painter.add(Shape::line_segment(
                                        line.map(|p| {
                                            Pos2::new(
                                                (p.x + 1.0) / 2.0 * width_f32,
                                                (-p.y + 1.0) / 2.0 * height_f32,
                                            )
                                        }),
                                        (5.0 * alpha, Color32::YELLOW.gamma_multiply(alpha)),
                                    ));
                                }
                            }
                        }
                    })
                };
                // Main rendering
                match graphics_state.render(
                    subchunks,
                    visible_chunks,
                    &egui_ctx,
                    egui_full_output,
                    &debug_state,
                ) {
                    Ok(new_debug_output) => debug_output = new_debug_output,
                    Err(wgpu::SurfaceError::Timeout) => {}
                    // Reconfigure the surface if lost
                    Err(wgpu::SurfaceError::Lost) | Err(wgpu::SurfaceError::Outdated) => {
                        let size = graphics_state.size;
                        graphics_state.resize(size)
                    }
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory) => window_target.exit(),
                }
                // Process game events
                {
                    let span = tracing::trace_span!("process_game_events");
                    let _enter = span.enter();
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
                            ClientboundPacket::LoginPlay { raw_entity_id, .. } => {
                                play_state.player_entity_id = EntityId(raw_entity_id);
                                println!("Login play: {{ player_entity_id: {raw_entity_id} }}");
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
                                )(
                                    InputSpan::new(&data.chunk_data)
                                )
                                .unwrap();
                                assert_eq!(rest.len(), 0);
                                // eprintln!("Sky light mask:          {:b}", &data.light_info.sky_light_mask);
                                // eprintln!("Empty sky light mask:    {:b}", &data.light_info.empty_sky_light_mask);
                                // eprintln!("Block light mask:        {:b}", &data.light_info.block_light_mask);
                                // eprintln!("Empty block light mask:  {:b}", &data.light_info.empty_block_light_mask);
                                let lighting =
                                    protocol_chunk::ChunkLightInfo::from_raw(data.light_info, 24);
                                raw_chunks.insert(
                                    [chunk_x, chunk_z],
                                    Arc::new(RawChunk {
                                        sections: chunk_sections.into(),
                                        lighting,
                                    }),
                                );
                                {
                                    let update_id = current_update_id;
                                    current_update_id = current_update_id.wrapping_add(1);
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
                                        let update_id = current_update_id;
                                        current_update_id = current_update_id.wrapping_add(1);
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
                                    let new_block_id: GlobalPaletteIndex =
                                        update.block_id.0.try_into().unwrap();
                                    let old_block_id = chunk_section.block_states.replace(
                                        x_usize,
                                        y_usize,
                                        z_usize,
                                        new_block_id,
                                    );
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
                                let update_id = current_update_id;
                                current_update_id = current_update_id.wrapping_add(1);
                                subchunks_to_dispatch
                                    .insert([chunk_x, subchunk_y, chunk_z], update_id);
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
                                let update_id = current_update_id;
                                current_update_id = current_update_id.wrapping_add(1);
                                let subchunk_y = section_i as i32;
                                subchunks_to_dispatch
                                    .insert([chunk_x, subchunk_y, chunk_z], update_id);
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
                                            subchunks_to_dispatch
                                                .insert(subchunk_coords, update_id);
                                            play_state
                                                .pending_subchunk_update_ids
                                                .insert(subchunk_coords, update_id);
                                        } else if axis == 15 {
                                            subchunk_coords[axis_i] += 1;
                                            subchunks_to_dispatch
                                                .insert(subchunk_coords, update_id);
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
                                    let global_y = section_i as i32 * SUBCHUNK_AXIS_LEN_I32
                                        + y as i32
                                        + MIN_HEIGHT_I32;
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
                                            .send(ClientPlayStateUpdate::RemoveChunk(
                                                neighbour_chunk_coords,
                                            ))
                                            .unwrap();
                                    }
                                }
                            }
                            ClientboundPacket::SynchronizePlayerPosition(pos_info) => {
                                use crate::protocol::play::{PositionChange, RotationChange};
                                debug_state.cull_camera_moving_with_player = true;
                                let camera = &mut graphics_state.camera;
                                let camera_pos = camera.pos.coords;
                                camera.pos.coords.x = match pos_info.x {
                                    PositionChange::Absolute(new_x) => new_x as f32,
                                    PositionChange::Relative(x_diff) => {
                                        camera_pos.x + x_diff as f32
                                    }
                                };
                                camera.pos.coords.y = match pos_info.y {
                                    PositionChange::Absolute(new_y) => new_y as f32 + 1.62,
                                    PositionChange::Relative(y_diff) => {
                                        camera_pos.y + y_diff as f32
                                    }
                                };
                                camera.pos.coords.z = match pos_info.z {
                                    PositionChange::Absolute(new_z) => new_z as f32,
                                    PositionChange::Relative(z_diff) => {
                                        camera_pos.z + z_diff as f32
                                    }
                                };
                                let (cam_yaw, cam_pitch) = camera.get_mc_rot();
                                let new_yaw = match pos_info.yaw {
                                    RotationChange::Absolute(new_yaw) => new_yaw,
                                    RotationChange::Relative(yaw_diff) => cam_yaw + yaw_diff,
                                };
                                let new_pitch = match pos_info.pitch {
                                    RotationChange::Absolute(new_pitch) => new_pitch,
                                    RotationChange::Relative(pitch_diff) => cam_pitch + pitch_diff,
                                };
                                camera.set_mc_rot(new_yaw, new_pitch);
                                // Make sure we've committed the changes to cull camera before we
                                // send a movement packet using its position.
                                debug_state.cull_camera = graphics_state.camera;
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
                                    let section_i = (global_y - MIN_HEIGHT_I32)
                                        .div_euclid(SUBCHUNK_AXIS_LEN_I32);
                                    let subchunk_y = section_i;
                                    let local_x = global_x.rem_euclid(SUBCHUNK_AXIS_LEN_I32);
                                    let local_y = global_y.rem_euclid(SUBCHUNK_AXIS_LEN_I32);
                                    let local_z = global_z.rem_euclid(SUBCHUNK_AXIS_LEN_I32);
                                    subchunk_updates
                                        .entry([chunk_x, subchunk_y, chunk_z])
                                        .or_default()
                                        .push(
                                            [local_x, local_y, local_z]
                                                .map(|n| n.try_into().unwrap()),
                                        );
                                }
                                for (subchunk_coords, blocks) in subchunk_updates {
                                    clientbound_tx
                                        .send(ClientboundPacket::UpdateSectionBlocks(
                                            protocol_play::UpdateSectionBlocks {
                                                subchunk_coords,
                                                blocks: blocks
                                                    .into_iter()
                                                    .map(|coords| {
                                                        (coords, air_global_palette_index)
                                                    })
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
                        let mut subchunk_update_groups: IndexMap<usize, Vec<[i32; 3]>> =
                            IndexMap::new();
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
                            Err(other_err) => panic!(
                                "Error while trying to receive play state update: {other_err:?}"
                            ),
                        };
                        let span = tracing::trace_span!("dispatch_play_state_update");
                        let _enter = span.enter();
                        match update {
                            ClientPlayStateUpdate::RemoveChunk(chunk_coords) => {
                                let [chunk_x, chunk_z] = chunk_coords;
                                // Remove old subchunks
                                if play_state.visible_chunks.contains(&chunk_coords) {
                                    let span =
                                        tracing::trace_span!("remove_subchunks", ?chunk_coords);
                                    let _enter = span.enter();
                                    for subchunk_y in 0..24 {
                                        use std::collections::hash_map::Entry;
                                        let subchunk_coords = [chunk_x, subchunk_y, chunk_z];
                                        let span = tracing::trace_span!(
                                            "remove_subchunk",
                                            ?subchunk_coords
                                        );
                                        let _enter = span.enter();
                                        if let Entry::Occupied(entry) =
                                            play_state.subchunks.entry(subchunk_coords)
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
                                        let span = tracing::trace_span!(
                                            "remove_old_subchunk",
                                            ?subchunk_coords
                                        );
                                        let _enter = span.enter();
                                        let span = tracing::trace_span!(
                                            "remove_old_subchunk",
                                            ?subchunk_coords
                                        );
                                        let _enter = span.enter();
                                        if let Entry::Occupied(entry) =
                                            play_state.subchunks.entry(subchunk_coords)
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
                                    let mut block_face_instance_groups: [(u32, u32); 6] =
                                        Default::default();
                                    for i in 0..6 {
                                        let Some(base_quad) = raw_subchunk.block_face_quads[i]
                                        else {
                                            continue;
                                        };
                                        let quad_start_vertex =
                                            buffer_managers.block_face_vertex.alloc_area(
                                                &graphics_state.resources.queue,
                                                subchunk_coords,
                                                base_quad,
                                            );
                                        let instance_group =
                                            &raw_subchunk.block_face_instance_groups[i];
                                        let instance_group_start =
                                            buffer_managers.block_face_instance.alloc_area(
                                                &graphics_state.resources.queue,
                                                subchunk_coords,
                                                instance_group,
                                            );
                                        let instance_group_len: u32 =
                                            instance_group.len().try_into().unwrap();
                                        block_face_start_vertices[i] = quad_start_vertex;
                                        block_face_instance_groups[i] =
                                            (instance_group_start, instance_group_len);
                                    }
                                    // Tinted block faces
                                    let mut tinted_block_face_start_vertices: [u32; 6] =
                                        [u32::MAX; 6];
                                    let mut tinted_block_face_instance_groups: [(u32, u32); 6] =
                                        Default::default();
                                    for i in 0..6 {
                                        let Some(base_quad) =
                                            raw_subchunk.tinted_block_face_quads[i]
                                        else {
                                            continue;
                                        };
                                        let quad_start_vertex =
                                            buffer_managers.tinted_block_face_vertex.alloc_area(
                                                &graphics_state.resources.queue,
                                                subchunk_coords,
                                                base_quad,
                                            );
                                        let instance_group =
                                            &raw_subchunk.tinted_block_face_instance_groups[i];
                                        let instance_group_start =
                                            buffer_managers.tinted_block_face_instance.alloc_area(
                                                &graphics_state.resources.queue,
                                                subchunk_coords,
                                                instance_group,
                                            );
                                        let instance_group_len: u32 =
                                            instance_group.len().try_into().unwrap();
                                        tinted_block_face_start_vertices[i] = quad_start_vertex;
                                        tinted_block_face_instance_groups[i] =
                                            (instance_group_start, instance_group_len);
                                    }
                                    let custom_block_groups = raw_subchunk
                                        .custom_block_groups
                                        .into_iter()
                                        .map(|group| {
                                            let start_instance =
                                                buffer_managers.custom_block_instance.alloc_area(
                                                    &graphics_state.resources.queue,
                                                    subchunk_coords,
                                                    &group.instances,
                                                );
                                            let num_instances: u32 =
                                                group.instances.len().try_into().unwrap();
                                            graphics::chunk::CustomBlockGroup {
                                                start_vertex: group.start_vertex,
                                                start_index_and_len: group.start_index_and_len,
                                                start_instance_and_len: (
                                                    start_instance,
                                                    num_instances,
                                                ),
                                            }
                                        })
                                        .collect();
                                    play_state.subchunks.insert(
                                        subchunk_coords,
                                        graphics::chunk::Subchunk {
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
                    {
                        let span = tracing::trace_span!("send_move_packet");
                        let _enter = span.enter();
                        let camera = &debug_state.cull_camera;
                        let camera_pos = camera.pos.coords;
                        let (mc_yaw, mc_pitch) = camera.get_mc_rot();
                        server_connection
                            .send_packet(serverbound_packets::SetPlayerPositionAndRotation {
                                x: camera_pos.x as f64,
                                feet_y: camera_pos.y as f64 - 1.62,
                                z: camera_pos.z as f64,
                                mc_yaw,
                                mc_pitch,
                                on_ground: false,
                            })
                            .unwrap();
                        server_connection.flush().unwrap();
                    }
                }
                tracing_tracy::client::frame_mark();
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
use nalgebra::Point3;

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
