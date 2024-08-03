pub mod graphics;
pub mod input;

use ahash::{AHashMap, AHashSet, AHasher};
use fixedbitset::FixedBitSet;
use graphics::{GraphicsResources, GraphicsState};
use input::PlayControlState;
use std::hash::Hasher;
use std::sync::{mpsc, Arc};
use std::time::Instant;
use threadpool::ThreadPool;
use winit::event::{Event, StartCause, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

use super::resource;
use crate::identifier;
use resource::block::blockstate;
use resource::block::model::{ModelType, Tint};

use crate::protocol::v765::play::{
    serverbound as serverbound_packets, Clientbound as ClientboundPacket,
};
use crate::protocol::v765::prelude::PlayConnection;
use crate::protocol::Deserialize;

struct ClientPlayState {
    pub raw_chunks: Arc<AHashMap<[i32; 2], Arc<[crate::ChunkSection]>>>,
    // TODO: Currently the Y coordinate is a chunk section index, rather than the subchunk Y
    //       coordinate. Consider changing to actually be the Y coordinate.
    pub subchunks: AHashMap<[i32; 3], graphics::chunk::Subchunk>,
    pub visible_chunks: AHashSet<[i32; 2]>,
}

#[derive(Debug)]
enum ClientPlayStateUpdate {
    PlaceChunk {
        chunk_coords: [i32; 2],
        new_raw_subchunks: Vec<(i32, RawSubchunk)>,
    },
}

#[derive(Debug)]
struct RawSubchunk {
    pub start_coords: [i32; 3],
    pub block_face_quads: [Option<[graphics::chunk::block_face::Vertex; 4]>; 6],
    pub block_face_instance_groups: [Vec<graphics::chunk::block_face::Instance>; 6],
    pub tinted_block_face_quads: [Option<[graphics::chunk::tinted_block_face::Vertex; 4]>; 6],
    pub tinted_block_face_instance_groups: [Vec<graphics::chunk::tinted_block_face::Instance>; 6],
    pub custom_block_groups: Vec<RawCustomBlockGroup>,
    pub connected_faces: graphics::chunk::SubchunkConnectivity,
}

#[derive(Debug)]
struct RawCustomBlockGroup {
    pub start_vertex: u32,
    pub start_index_and_len: (u32, u32),
    pub instances: Vec<graphics::chunk::custom_block::Instance>,
}

const SUBCHUNK_AXIS_LEN: usize = 16;
const SUBCHUNK_AXIS_LEN_I32: i32 = SUBCHUNK_AXIS_LEN as i32;
const MIN_HEIGHT_I32: i32 = -64;
const MAX_HEIGHT_I32: i32 = 319;

pub(crate) async fn window_run(
    server_connection: Arc<PlayConnection>,
    clientbound_rx: std::sync::mpsc::Receiver<ClientboundPacket>,
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
    let mut play_state = ClientPlayState {
        raw_chunks: Arc::new(AHashMap::new()),
        subchunks: AHashMap::new(),
        visible_chunks: AHashSet::new(),
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
                            ui.checkbox(
                                &mut debug_state.cull_camera_moving_with_player,
                                "Move cull camera with player camera",
                            );
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
                                    if pos.y < 0.0 || section_i >= chunk.len() {
                                        ui.label("Global Palette ID: N/A");
                                        ui.label("Identifier: N/A");
                                        ui.label("Blockstate data: N/A");
                                    } else {
                                        let chunk_section = &chunk[section_i];
                                        let global_palette_index =
                                            chunk_section.block_states.get(x, y, z);
                                        let blockstate =
                                            &graphics_state.resources.block_registry.global_palette
                                                [usize::from(global_palette_index)];
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
                    &subchunks,
                    &visible_chunks,
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
                    let mut chunks_to_dispatch: AHashSet<[i32; 2]> = AHashSet::new();
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
                            ClientboundPacket::LoginPlay(_packet) => {
                                println!("Login play: {{<skipped>}}")
                            }
                            ClientboundPacket::KeepAlive { id } => {
                                server_connection
                                    .send_packet(serverbound_packets::KeepAliveResponse { id })
                                    .unwrap();
                            }
                            // Configuration
                            ClientboundPacket::UpdateRecipes(_recipes) => {
                                println!("Update recipes: [<skipped>]")
                            }
                            ClientboundPacket::UpdateTags(_tags) => {
                                println!("Update tags: [<skipped>]")
                            }
                            ClientboundPacket::DeclareCommands(_) => {
                                println!("Declare commands: [<todo>]")
                            }
                            ClientboundPacket::UpdateRecipeBook(_) => {
                                println!("Update recipes: {{<skipped>}}")
                            }
                            ClientboundPacket::ServerData(data) => {
                                println!("Server MOTD: {:?}", data.motd)
                            }
                            // Gameplay
                            ClientboundPacket::ChunkBatchStart => println!("Chunk batch started"),
                            ClientboundPacket::ChunkBatchEnd { num_chunks } => {
                                println!("Chunk batch ended, received {num_chunks} chunks");
                                server_connection
                                    .send_packet(serverbound_packets::ChunkBatchReceived {
                                        desired_chunks_per_tick: 4.0,
                                    })
                                    .unwrap();
                            }
                            ClientboundPacket::ChunkDataAndUpdateLight(data) => {
                                raw_chunks = Arc::make_mut(&mut play_state.raw_chunks);
                                println!("Update chunk data and lighting: {{<skipped>}}");
                                let (rest, chunk_sections) =
                                    nom::multi::count(crate::ChunkSection::deserialize, 24)(
                                        &data.chunk_data,
                                    )
                                    .map_err(|err| err.to_owned())
                                    .unwrap();
                                assert_eq!(rest.len(), 0);
                                let chunk_coords = [data.chunk_x, data.chunk_z];
                                raw_chunks.insert(chunk_coords, chunk_sections.into());
                                chunks_to_dispatch.insert(chunk_coords);
                                let neighbouring_chunks = [
                                    [data.chunk_x - 1, data.chunk_z],
                                    [data.chunk_x + 1, data.chunk_z],
                                    [data.chunk_x, data.chunk_z - 1],
                                    [data.chunk_x, data.chunk_z + 1],
                                ];
                                for neighbour_chunk_coords in neighbouring_chunks {
                                    if raw_chunks.contains_key(&neighbour_chunk_coords) {
                                        if !visible_chunks.contains(&neighbour_chunk_coords) {
                                            chunks_to_dispatch.insert(neighbour_chunk_coords);
                                        }
                                    }
                                }
                            }
                            ClientboundPacket::UpdateLight(_) => {
                                println!("Update chunk lighting: {{<todo>}}")
                            }
                            ClientboundPacket::UnloadChunk { chunk_x, chunk_z } => {
                                raw_chunks = Arc::make_mut(&mut play_state.raw_chunks);
                                let chunk_coords = [chunk_x, chunk_z];
                                raw_chunks.remove(&chunk_coords);
                                chunks_to_dispatch.insert(chunk_coords);
                                let neighbouring_chunks = [
                                    [chunk_x - 1, chunk_z],
                                    [chunk_x + 1, chunk_z],
                                    [chunk_x, chunk_z - 1],
                                    [chunk_x, chunk_z + 1],
                                ];
                                for neighbour_chunk_coords in neighbouring_chunks {
                                    if visible_chunks.contains(&neighbour_chunk_coords) {
                                        chunks_to_dispatch.insert(neighbour_chunk_coords);
                                    }
                                }
                            }
                            ClientboundPacket::SynchronizePlayerPosition(pos_info) => {
                                use crate::protocol::v765::play::{PositionChange, RotationChange};
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
                                server_connection
                                    .send_packet(serverbound_packets::ConfirmTeleportation {
                                        id: pos_info.teleport_id,
                                    })
                                    .unwrap();
                            }
                            _ => {} // other => println!("{other:?}"),
                        }
                    }
                    if thread_pool.panic_count() > 0 {
                        panic!("Thread pool panic");
                    }
                    // Dispatch chunk processing
                    {
                        let span = tracing::trace_span!("dispatch_chunk_processing");
                        let _enter = span.enter();
                        for chunk_coords in chunks_to_dispatch {
                            let graphics_resources = graphics_state.resources.clone();
                            let raw_chunks = play_state.raw_chunks.clone();
                            let play_state_update_tx = play_state_update_tx.clone();
                            thread_pool.execute(move || {
                                process_chunk(
                                    &graphics_resources,
                                    &raw_chunks,
                                    play_state_update_tx,
                                    chunk_coords,
                                )
                            });
                        }
                    }
                    // Receive play state updates
                    let mut chunks_processed_this_frame = 0;
                    loop {
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
                            ClientPlayStateUpdate::PlaceChunk {
                                chunk_coords,
                                new_raw_subchunks,
                            } => {
                                let [chunk_x, chunk_z] = chunk_coords;
                                // Remove old subchunks
                                if play_state.visible_chunks.contains(&chunk_coords) {
                                    let span =
                                        tracing::trace_span!("remove_old_chunks", ?chunk_coords);
                                    let _enter = span.enter();
                                    for subchunk_y in 0..24 {
                                        use std::collections::hash_map::Entry;
                                        let subchunk_coords = [chunk_x, subchunk_y, chunk_z];
                                        let span = tracing::trace_span!(
                                            "remove_old_subchunk",
                                            ?subchunk_coords
                                        );
                                        let _enter = span.enter();
                                        if let Entry::Occupied(entry) =
                                            play_state.subchunks.entry(subchunk_coords)
                                        {
                                            entry.remove();
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
                                        }
                                    }
                                }
                                // Mark chunk visibility
                                if new_raw_subchunks.is_empty() {
                                    play_state.visible_chunks.remove(&chunk_coords);
                                    chunks_processed_this_frame += 1;
                                    if chunks_processed_this_frame >= 1 {
                                        break;
                                    } else {
                                        continue;
                                    }
                                } else {
                                    play_state.visible_chunks.insert(chunk_coords);
                                }
                                // Add new subchunks
                                let span = tracing::trace_span!("add_new_chunks");
                                let _enter = span.enter();
                                let buffer_managers = &mut graphics_state.buffer_managers;
                                for (subchunk_y, raw_subchunk) in new_raw_subchunks {
                                    let subchunk_coords = [chunk_x, subchunk_y, chunk_z];
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
                                }
                                chunks_processed_this_frame += 1;
                                if chunks_processed_this_frame >= 1 {
                                    break;
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

fn process_chunk(
    graphics_resources: &GraphicsResources,
    raw_chunks: &AHashMap<[i32; 2], Arc<[crate::ChunkSection]>>,
    play_state_update_tx: mpsc::Sender<ClientPlayStateUpdate>,
    chunk_coords: [i32; 2],
) {
    let [chunk_x, chunk_z] = chunk_coords;
    let Some(chunk_sections) = raw_chunks.get(&chunk_coords) else {
        play_state_update_tx
            .send(ClientPlayStateUpdate::PlaceChunk {
                chunk_coords,
                new_raw_subchunks: Vec::new(),
            })
            .unwrap();
        return;
    };
    // Skip chunks with missing neighbours, so that for every chunk we actually render, it
    // has all its neighbours to decide whether border faces should be rendered.
    // I believe Minecraft does the same.
    {
        let surrounding_chunk_coords = [
            [chunk_x - 1, chunk_z],
            [chunk_x + 1, chunk_z],
            [chunk_x, chunk_z - 1],
            [chunk_x, chunk_z + 1],
        ];
        for chunk in surrounding_chunk_coords {
            if !raw_chunks.contains_key(&chunk) {
                play_state_update_tx
                    .send(ClientPlayStateUpdate::PlaceChunk {
                        chunk_coords,
                        new_raw_subchunks: Vec::new(),
                    })
                    .unwrap();
                return;
            }
        }
    }
    let spruce_leaves_registry_index = graphics_resources
        .block_registry
        .get_index_from_identifier(&identifier!("minecraft:spruce_leaves"))
        .unwrap();
    let mut block_hasher = AHasher::default();
    let mut new_raw_subchunks: Vec<(i32, RawSubchunk)> = Vec::new();
    for (section_i, chunk_section) in chunk_sections.iter().enumerate() {
        if chunk_section.block_count == 0 {
            continue;
        }
        let mut block_faces: [Vec<_>; 6] = Default::default();
        let mut tinted_block_faces: [Vec<_>; 6] = Default::default();
        let mut custom_block_instance_groups = AHashMap::new();
        for y in 0..SUBCHUNK_AXIS_LEN {
            let global_y_i32 = (SUBCHUNK_AXIS_LEN * section_i + y) as i32 + MIN_HEIGHT_I32;
            let global_y = global_y_i32 as f32;
            for z in 0..SUBCHUNK_AXIS_LEN {
                let global_z_i32 = (SUBCHUNK_AXIS_LEN_I32 * chunk_z) + z as i32;
                let global_z = global_z_i32 as f32;
                for x in 0..SUBCHUNK_AXIS_LEN {
                    let global_x_i32 = (SUBCHUNK_AXIS_LEN_I32 * chunk_x) + x as i32;
                    let global_x = global_x_i32 as f32;
                    let global_palette_index = chunk_section.block_states.get(x, y, z);
                    let blockstate_info = &graphics_resources.block_registry.global_palette
                        [usize::from(global_palette_index)];
                    let model = match &blockstate_info.model_data {
                        blockstate::ModelData::Single(model) => model,
                        blockstate::ModelData::RandomChoice(models) => 'model_blk: {
                            // Find weight for model by hash
                            block_hasher.write_i32(global_x_i32);
                            block_hasher.write_i32(global_y_i32);
                            block_hasher.write_i32(global_z_i32);
                            let hash = block_hasher.finish();
                            let mut current_percentage = (hash % 257) as f32 / 256.0;
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
                        let check_global_y = ((SUBCHUNK_AXIS_LEN * section_i) as i32 + y) - 64;
                        if check_global_y < MIN_HEIGHT_I32 || check_global_y > MAX_HEIGHT_I32 {
                            continue;
                        }
                        let check_sections = match [x, z].iter().any(|n| !(0..=15).contains(n)) {
                            false => &chunk_sections,
                            true => match (x, z) {
                                (-1, _) => &raw_chunks[&[chunk_x - 1, chunk_z]],
                                (16, _) => &raw_chunks[&[chunk_x + 1, chunk_z]],
                                (_, -1) => &raw_chunks[&[chunk_x, chunk_z - 1]],
                                (_, 16) => &raw_chunks[&[chunk_x, chunk_z + 1]],
                                _ => unreachable!(),
                            },
                        };
                        let indexing_section = &check_sections[usize::try_from(
                            ((SUBCHUNK_AXIS_LEN * section_i) as i32 + y) / SUBCHUNK_AXIS_LEN as i32,
                        )
                        .unwrap()];
                        let (x, y, z) = (
                            ((x + SUBCHUNK_AXIS_LEN_I32) % SUBCHUNK_AXIS_LEN_I32) as usize,
                            y as usize,
                            ((z + SUBCHUNK_AXIS_LEN_I32) % SUBCHUNK_AXIS_LEN_I32) as usize,
                        );
                        let global_palette_index = indexing_section.block_states.get(x, y % 16, z);
                        let blockstate_info = &graphics_resources.block_registry.global_palette
                            [usize::from(global_palette_index)];
                        let block_info =
                            &graphics_resources.block_registry[blockstate_info.block_index];
                        face_cull_map[i] = block_info.properties.opaque;
                    }
                    // Spruce Leaves are hardcoded, so override tint colour here
                    let tint_color = match blockstate_info.block_index {
                        ident if ident == spruce_leaves_registry_index => [0x61, 0x99, 0x61, 0xFF],
                        _ => [0x91, 0xBD, 0x59, 0xFF],
                    };
                    match model.as_ref() {
                        ModelType::None => continue,
                        ModelType::Block(info) => {
                            for i in 0..6 {
                                if face_cull_map[i] {
                                    continue;
                                }
                                block_faces[i].push(graphics::chunk::block_face::Instance::new(
                                    [x as u8, y as u8, z as u8],
                                    info.per_face_atlas_uvs[i],
                                    info.per_face_uv_rotations[i],
                                ));
                            }
                        }
                        ModelType::TintedBlock(info) => {
                            for i in 0..6 {
                                if face_cull_map[i] {
                                    continue;
                                }
                                tinted_block_faces[i].push(
                                    graphics::chunk::tinted_block_face::Instance::new(
                                        [x as u8, y as u8, z as u8],
                                        info.per_face_atlas_uvs[i],
                                        info.per_face_uv_rotations[i],
                                        tint_color,
                                    ),
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
                                        graphics::chunk::tinted_block_face::Instance::new(
                                            [x as u8, y as u8, z as u8],
                                            face.atlas_uvs,
                                            face.uv_rotation,
                                            tint_color,
                                        ),
                                    );
                                } else {
                                    block_faces[face.face_i as usize].push(
                                        graphics::chunk::block_face::Instance::new(
                                            [x as u8, y as u8, z as u8],
                                            face.atlas_uvs,
                                            face.uv_rotation,
                                        ),
                                    );
                                }
                            }
                        }
                        ModelType::Other(info) => {
                            let block_instances = custom_block_instance_groups
                                .entry(info)
                                .or_insert_with(Vec::new);
                            block_instances.push(graphics::chunk::custom_block::Instance {
                                pos: [global_x, global_y, global_z],
                                tint_color,
                            });
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
            use super::Palette;
            use crate::basic_types::AxisDirection;
            use graphics::chunk::SubchunkConnectivity;
            // If we can immediately tell all the subchunk blocks are opaque, skip this entire
            // process and just return that no subchunk faces are connected.
            match chunk_section.block_states.palette() {
                Palette::SingleValue(index) => {
                    let blockstate_info =
                        &graphics_resources.block_registry.global_palette[usize::from(*index)];
                    let block_info =
                        &graphics_resources.block_registry[blockstate_info.block_index];
                    break 'connected_faces match block_info.properties.opaque {
                        true => SubchunkConnectivity::empty(),
                        false => SubchunkConnectivity::full(),
                    };
                }
                Palette::Palette(indices) => {
                    let mut num_opaque = 0;
                    for index in indices {
                        let blockstate_info =
                            &graphics_resources.block_registry.global_palette[usize::from(*index)];
                        let block_info =
                            &graphics_resources.block_registry[blockstate_info.block_index];
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
                        let blockstate_info = &graphics_resources.block_registry.global_palette
                            [usize::from(global_palette_index)];
                        let block_info =
                            &graphics_resources.block_registry[blockstate_info.block_index];
                        if !block_info.properties.opaque {
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
                    .map(|coord| {
                        queue.remove(&coord);
                        coord
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
        let subchunk_y = section_i as i32;
        let start_coords = [
            SUBCHUNK_AXIS_LEN_I32 * chunk_x,
            SUBCHUNK_AXIS_LEN_I32 * subchunk_y + MIN_HEIGHT_I32,
            SUBCHUNK_AXIS_LEN_I32 * chunk_z,
        ];
        // Block faces
        let mut block_face_quads: [Option<_>; 6] = [None; 6];
        let block_face_instance_groups: [Vec<_>; 6] = block_faces;
        for i in 0..6 {
            if block_face_instance_groups[i].len() == 0 {
                continue;
            }
            let base_quad =
                graphics::chunk::block_face::Vertex::generate_base_quad(start_coords, i);
            block_face_quads[i] = Some(base_quad);
        }
        // Tinted block faces
        let mut tinted_block_face_quads: [Option<_>; 6] = [None; 6];
        let tinted_block_face_instance_groups: [Vec<_>; 6] = tinted_block_faces;
        for i in 0..6 {
            if tinted_block_face_instance_groups[i].len() == 0 {
                continue;
            }
            let base_quad =
                graphics::chunk::tinted_block_face::Vertex::generate_base_quad(start_coords, i);
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
        new_raw_subchunks.push((
            subchunk_y,
            RawSubchunk {
                start_coords,
                block_face_quads,
                block_face_instance_groups,
                tinted_block_face_quads,
                tinted_block_face_instance_groups,
                custom_block_groups,
                connected_faces,
            },
        ));
    }
    play_state_update_tx
        .send(ClientPlayStateUpdate::PlaceChunk {
            chunk_coords,
            new_raw_subchunks,
        })
        .unwrap();
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
