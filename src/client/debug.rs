use crate::client::graphics::debug::PackedFlags as DebugPackedFlags;
use crate::client::graphics::debug::line::Instance as DebugLine;
use crate::client::graphics::debug::point::Vertex as DebugPoint;
use crate::client::graphics::debug::triangle::Instance as DebugTriangle;
use crate::client::graphics::{self, Camera, DebugVisualisationDrawMethod, GraphicsState};
use crate::client::{ClientPlayState, MIN_HEIGHT_I32};
use portable_std::VecDeque;
use crate::portable_prelude::*;
use crate::protocol::PlayConnection;
use crate::protocol::play::GameMode;
use nalgebra::Point3;
use threadpool::ThreadPool;

pub struct DebugRenderOutput {
    pub egui_output: egui::FullOutput,
    pub debug_points: Vec<DebugPoint>,
    pub debug_lines: Vec<DebugLine>,
    pub debug_triangles: Vec<DebugTriangle>,
}

#[expect(clippy::too_many_arguments)]
pub fn render_debug_ui(
    thread_pool: &ThreadPool,
    server_connection: &PlayConnection,
    play_state: &mut ClientPlayState,
    graphics_state: &mut GraphicsState,
    debug_state: &mut graphics::DebugState,
    debug_output: &graphics::DebugOutput,
    egui_ctx: &egui::Context,
    events: &mut Vec<egui::Event>,
    previous_frame_times: &VecDeque<f64>,
    scale_factor: f64,
    current_time_s: f64,
    delta_time_f64: f64,
    delta_time: f32,
) -> DebugRenderOutput {
    let subchunks = &play_state.subchunks;
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
        events: core::mem::take(events),
        ..Default::default()
    };
    #[allow(unused_mut)]
    let mut debug_points: Vec<DebugPoint> = Vec::new();
    #[allow(unused_mut)]
    let mut debug_lines: Vec<DebugLine> = Vec::new();
    #[allow(unused_mut)]
    let mut debug_triangles: Vec<DebugTriangle> = Vec::new();
    let egui_output = egui_ctx.run(raw_input, |ctx| {
        use egui::*;
        let width_f32 = graphics_state.size.width as f32 / scale_factor as f32;
        let height_f32 = graphics_state.size.height as f32 / scale_factor as f32;
        let painter = Painter::new(ctx.clone(), LayerId::background(), Rect::EVERYTHING);
        Window::new("Debug Info").resizable(false).show(ctx, |ui| {
            ui.label(format!("FPS: {:.2}", 1.0 / delta_time_f64));
            // VSync
            {
                let mut new_graphics_options = graphics_state.graphics_options;
                ui.checkbox(&mut new_graphics_options.vsync, "VSync");
                if new_graphics_options != graphics_state.graphics_options {
                    graphics_state.apply_new_graphics_options(new_graphics_options);
                }
            }
            ui.label(format!("Position: {:.2?}", graphics_state.camera.pos));
            ui.label(format!(
                "Subchunks Culled: {}",
                debug_output.subchunks_culled
            ));
            ui.add(Slider::new(&mut debug_state.cull_planes_active, 0..=6).text("Planes active"));
            ui.add(
                Slider::new(&mut debug_state.max_render_chunks, 0..=3000)
                    .drag_value_speed(1.0)
                    .clamping(SliderClamping::Never)
                    .text("Max render chunks"),
            );
            ui.checkbox(
                &mut debug_state.rendering_view_frustum,
                "Render view frustum",
            );
            // Free cam
            {
                let old_free_cam = debug_state.free_cam;
                ui.checkbox(&mut debug_state.free_cam, "Free cam");
                // Change head rotation back to player's head rotation if we've just
                // turned off free cam.
                // Position is fixed later, so no need to change here.
                if old_free_cam && !debug_state.free_cam {
                    let player = &play_state.player;
                    let camera = &mut graphics_state.camera;
                    camera.yaw = player.yaw;
                    camera.pitch = player.pitch;
                }
            }
            // Debug graphics draw method
            ComboBox::from_id_salt("Debug visualisation draw method")
                .selected_text(debug_state.visualisation_draw_method.label_text())
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut debug_state.visualisation_draw_method,
                        graphics::DebugVisualisationDrawMethod::Egui,
                        graphics::DebugVisualisationDrawMethod::Egui.label_text(),
                    );
                    ui.selectable_value(
                        &mut debug_state.visualisation_draw_method,
                        graphics::DebugVisualisationDrawMethod::Gpu,
                        graphics::DebugVisualisationDrawMethod::Gpu.label_text(),
                    );
                });
            ui.collapsing("Game Mode", |ui| {
                let player = &play_state.player;
                let mut game_mode = player.game_mode;
                ui.radio_value(&mut game_mode, GameMode::Survival, "Survival");
                ui.radio_value(&mut game_mode, GameMode::Creative, "Creative");
                ui.radio_value(&mut game_mode, GameMode::Adventure, "Adventure");
                ui.radio_value(&mut game_mode, GameMode::Spectator, "Spectator");
                if game_mode != player.game_mode {
                    server_connection
                        .send_packet(crate::protocol::play::serverbound::ChatCommand(format!(
                            "gamemode {} @s",
                            match game_mode {
                                GameMode::Survival => "survival",
                                GameMode::Creative => "creative",
                                GameMode::Adventure => "adventure",
                                GameMode::Spectator => "spectator",
                            }
                        )))
                        .unwrap();
                    server_connection.flush().unwrap();
                }
            });
            ui.collapsing("Cave Culling", |ui| {
                ui.checkbox(&mut debug_state.cave_cull_check_unflipped, "Flip check");
                ui.checkbox(
                    &mut debug_state.cave_cull_check_not_backwards,
                    "Backwards check",
                );
                ui.checkbox(&mut debug_state.cave_cull_check_frustum, "Frustum check");
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
                    Slider::new(&mut debug_state.cave_cull_debug_render_dist, 0.0..=64.0)
                        .clamping(SliderClamping::Never)
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
                        let global_palette_index = chunk_section.block_states.get(x, y, z);
                        let blockstate =
                            &graphics_state.resources.block_registry[global_palette_index];
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
                        "Subchunk start coords: {:?}",
                        subchunk.start_coords
                    ));
                    ui.label(format!(
                        "Subchunk connectivity info: {:?}",
                        subchunk.connected_faces,
                    ));
                } else {
                    ui.label("Subchunk connectivity info: N/A");
                    ui.label("Subchunk start coords: N/A");
                }
            });
            #[cfg(feature = "graphics_backend_vulkan")]
            ui.collapsing("Radiance Cascades", |ui| {
                // if ui.button("Update Radiance Lighting").clicked() {
                //     graphics_state.update_all_subchunks_radiance_lighting(&play_state.subchunks);
                // }
                if ui.button("Calculate lighting").clicked() {
                    graphics_state.update_all_subchunks_radiance_lighting(
                        thread_pool,
                        &play_state.subchunks,
                        &play_state.raw_chunks,
                    );
                }
                ui.collapsing("Debug Frame", |ui| {
                    if ui.button("Render Debug Frame").clicked() {
                        graphics_state.radiance_cascades_debug_render(&play_state.subchunks);
                    }
                    ui.add(
                        Slider::new(&mut debug_state.debug_texture_zoom, 1.0..=8.0)
                            .text("Debug image zoom"),
                    );
                    let mut debug_egui_texture =
                        graphics_state.radiance_cascades.debug_egui_texture;
                    debug_egui_texture.size *= debug_state.debug_texture_zoom;
                    ui.image(debug_egui_texture);
                });
                let debug_info = *graphics_state.radiance_cascades.debug_info.lock().unwrap();
                ui.checkbox(
                    &mut debug_state.radiance_cascades_ray_visualiser,
                    "Ray visualiser",
                );
                ui.checkbox(
                    &mut debug_state.radiance_cascades_light_tree_visualiser,
                    "Light tree visualiser",
                );
                ui.add(
                    Slider::new(&mut debug_state.radiance_cascades_light_tree_level, 0..=12)
                        .text("Light tree level"),
                );
                ui.checkbox(
                    &mut debug_state.radiance_cascades_areaquad_visualiser,
                    "Areaquad visualiser",
                );
                ui.add(Slider::new(&mut debug_state.max_radiance_cascade, 0..=6).text("Cascade"));
                ui.label(format!("GPU Debug info: {debug_info:?}"));
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
        if debug_state.rendering_view_frustum {
            use nalgebra::Point3;
            let camera = &graphics_state.camera;
            let inv_cull_view_mat = camera.generate_view_matrix().try_inverse().unwrap();
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
                    debug_clip_and_project_points(&points, camera)
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
                let subchunk_centre =
                    Point3::new(subchunk_centre[0], subchunk_centre[1], subchunk_centre[2]);
                // Render bounding box
                {
                    let start = Point3::new(
                        subchunk.start_coords[0] as f32,
                        subchunk.start_coords[1] as f32,
                        subchunk.start_coords[2] as f32,
                    );
                    let end = Point3::new(start.x + 16.0, start.y + 16.0, start.z + 16.0);
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
                        let end_1_dist = (graphics_camera.pos - line[0]).magnitude();
                        let end_2_dist = (graphics_camera.pos - line[1]).magnitude();
                        if end_1_dist > max_dist || end_2_dist > max_dist {
                            continue;
                        }
                        let Some(line) = debug_clip_and_project_line(line, graphics_camera) else {
                            continue;
                        };
                        let centre_dist = (graphics_camera.pos - subchunk_centre).magnitude();
                        let alpha = (1.0 - (centre_dist / max_dist.max(0.01))).max(0.0);
                        painter.add(Shape::line_segment(
                            line.map(|p| {
                                Pos2::new(
                                    (p.x + 1.0) / 2.0 * width_f32,
                                    (-p.y + 1.0) / 2.0 * height_f32,
                                )
                            }),
                            (
                                5.0 * alpha,
                                Color32::from_rgba_unmultiplied(0xFF, 0x00, 0xFF, 0xFF)
                                    .gamma_multiply(alpha),
                            ),
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
                    let max_dist = debug_state.cave_cull_debug_render_dist;
                    let end_1 = pair_centre + dir_1.as_vector() * 8.0;
                    let end_2 = pair_centre + dir_2.as_vector() * 8.0;
                    let end_1_dist = (graphics_camera.pos - end_1).magnitude();
                    let end_2_dist = (graphics_camera.pos - end_2).magnitude();
                    if end_1_dist > max_dist || end_2_dist > max_dist {
                        continue;
                    }
                    let average_dist = (end_1_dist + end_2_dist) / 2.0;
                    let alpha = (1.0 - (average_dist / max_dist.max(0.01))).max(0.0);
                    // TODO: Implement raw GPU rendering for this
                    if let Some(line_1) =
                        debug_clip_and_project_line([pair_centre, end_1], graphics_camera)
                    {
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
                    if let Some(line_2) =
                        debug_clip_and_project_line([pair_centre, end_2], graphics_camera)
                    {
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
        if debug_state.radiance_cascades_light_tree_visualiser {
            // use nalgebra::{Point3, Vector3};
            // static ICOSPHERE_BASE_VERTICES: [Vector3<f32>; 12] = [
            //     Vector3::new(0.8506508, 0.5257311, 0.0),
            //     Vector3::new(0.000000101405476, 0.8506507, -0.525731),
            //     Vector3::new(0.000000101405476, 0.8506506, 0.525731),
            //     Vector3::new(0.5257309, -0.00000006267203, -0.85065067),
            //     Vector3::new(0.52573115, -0.00000006267203, 0.85065067),
            //     Vector3::new(0.8506508, -0.5257311, 0.0),
            //     Vector3::new(-0.52573115, 0.00000006267203, -0.85065067),
            //     Vector3::new(-0.8506508, 0.5257311, 0.0),
            //     Vector3::new(-0.5257309, 0.00000006267203, 0.85065067),
            //     Vector3::new(-0.000000101405476, -0.8506506, -0.525731),
            //     Vector3::new(-0.000000101405476, -0.8506507, 0.525731),
            //     Vector3::new(-0.8506508, -0.5257311, 0.0),
            // ];
            // static ICOSPHERE_INDICES: [[usize; 3]; 20] = [
            //     [0, 1, 2],
            //     [0, 3, 1],
            //     [0, 2, 4],
            //     [3, 0, 5],
            //     [0, 4, 5],
            //     [1, 3, 6],
            //     [1, 7, 2],
            //     [7, 1, 6],
            //     [4, 2, 8],
            //     [7, 8, 2],
            //     [9, 3, 5],
            //     [6, 3, 9],
            //     [5, 4, 10],
            //     [4, 8, 10],
            //     [9, 5, 10],
            //     [7, 6, 11],
            //     [7, 11, 8],
            //     [11, 6, 9],
            //     [8, 11, 10],
            //     [10, 11, 9],
            // ];
            // let mut tris: Vec<DebugTriangle> = Vec::new();
            // if let Some(tree) = &graphics_state.radiance_cascades.debug_light_tree {
            //     for (i, node) in tree.iter().enumerate() {
            //         if node.children[0] != u32::MAX {
            //             break;
            //         }
            //         let centre: Point3<f32> = node.sphere_centre.into();
            //         let radius = node.sphere_radius;
            //         let vertices = ICOSPHERE_BASE_VERTICES.map(|v| centre + (v * radius));
            //         fn colour_hash(i: usize) -> Color32 {
            //             let mut x = i as u32;
            //             x ^= x >> 16;
            //             x = x.wrapping_mul(0x7FEB352D);
            //             x ^= x >> 15;
            //             x = x.wrapping_mul(0x846CA68B);
            //             x ^= x >> 16;
            //             x %= 1 << 24;
            //             Color32::from_rgba_unmultiplied(
            //                 x as u8,
            //                 (x >> 8) as u8,
            //                 (x >> 16) as u8,
            //                 0xFF
            //             )
            //         }
            //         tris.extend(ICOSPHERE_INDICES.map(|tri_indices| {
            //             let [p1, p2, p3] = tri_indices.map(|idx| vertices[idx]);
            //             DebugTriangle {
            //                 p1: p1.into(),
            //                 p2: p2.into(),
            //                 p3: p3.into(),
            //                 color: colour_hash(i).gamma_multiply(0.25).to_array(),
            //                 packed_fields: DebugPackedFlags::NONE,
            //             }
            //         }));
            //     }
            // }
            use nalgebra::{Point3, Vector3};
            let mut tris: Vec<DebugTriangle> = Vec::new();
            if let Some(tree) = &graphics_state.radiance_cascades.debug_light_tree {
                let current_level = debug_state.radiance_cascades_light_tree_level;
                struct TreeNode {
                    pub i: usize,
                    pub level: usize,
                }
                let mut node_queue = VecDeque::from([TreeNode {
                    i: tree.len() - 1,
                    level: 0,
                }]);
                let mut out_nodes = Vec::new();
                while let Some(tree_node) = node_queue.pop_front() {
                    let node = &tree[tree_node.i];
                    if tree_node.level >= current_level {
                        out_nodes.push((tree_node.i, node));
                    } else {
                        for child_i in node.children {
                            if child_i != u32::MAX {
                                node_queue.push_back(TreeNode {
                                    i: child_i as usize,
                                    level: tree_node.level + 1,
                                });
                            }
                        }
                    }
                }
                for (i, node) in out_nodes {
                    // if node.children != [u32::MAX; 2] {
                    //     break;
                    // }
                    let margin = Vector3::repeat(0.01);
                    let corner_1 = Point3::from(node.aabb_corner_1) - margin;
                    let corner_2 = Point3::from(node.aabb_corner_2) + margin;
                    let vertices = [
                        [corner_1.x, corner_1.y, corner_1.z],
                        [corner_2.x, corner_1.y, corner_1.z],
                        [corner_1.x, corner_1.y, corner_2.z],
                        [corner_2.x, corner_1.y, corner_2.z],
                        [corner_1.x, corner_2.y, corner_1.z],
                        [corner_2.x, corner_2.y, corner_1.z],
                        [corner_1.x, corner_2.y, corner_2.z],
                        [corner_2.x, corner_2.y, corner_2.z],
                    ];
                    const BOX_QUAD_INDICES: [[usize; 4]; 6] = [
                        // Bottom/Top
                        [0, 1, 3, 2],
                        [4, 5, 7, 6],
                        // Front/Back
                        [0, 1, 5, 4],
                        [2, 3, 7, 6],
                        // Left/Right
                        [0, 2, 6, 4],
                        [1, 3, 7, 5],
                    ];
                    fn colour_hash(i: usize) -> Color32 {
                        let mut x = i as u32;
                        x ^= x >> 16;
                        x = x.wrapping_mul(0x7FEB352D);
                        x ^= x >> 15;
                        x = x.wrapping_mul(0x846CA68B);
                        x ^= x >> 16;
                        x %= 1 << 24;
                        Color32::from_rgba_unmultiplied(
                            x as u8,
                            (x >> 8) as u8,
                            (x >> 16) as u8,
                            0xFF,
                        )
                    }
                    tris.extend(BOX_QUAD_INDICES.into_iter().flat_map(|tri_indices| {
                        let points = tri_indices.map(|idx| vertices[idx]);
                        [
                            DebugTriangle {
                                p1: points[0],
                                p2: points[1],
                                p3: points[2],
                                color: colour_hash(i).gamma_multiply(0.25).to_array(),
                                packed_fields: DebugPackedFlags::NONE,
                            },
                            DebugTriangle {
                                p1: points[0],
                                p2: points[3],
                                p3: points[2],
                                color: colour_hash(i).gamma_multiply(0.25).to_array(),
                                packed_fields: DebugPackedFlags::NONE,
                            },
                        ]
                    }));
                }
            }
            debug_triangles.extend(tris);
        }
        if debug_state.radiance_cascades_areaquad_visualiser {
            use nalgebra::Point3;
            // let probe_pos = Point3::new(16.28125, 155.0, -16.21875);
            let probe_pos = Point3::new(15.4, 155.0, -14.4);
            let corner_1 = Point3::new(15.0, 155.0, -17.0);
            let corner_2 = Point3::new(16.0, 156.0, -16.0);
            let local_corner_1 = corner_1 - probe_pos.coords;
            let local_corner_2 = corner_2 - probe_pos.coords;
            let local_corners: [Point3<f32>; 8] = {
                let [a, b] = [local_corner_1, local_corner_2];
                [
                    Point3::new(a.x, a.y, a.z),
                    Point3::new(a.x, a.y, b.z),
                    Point3::new(a.x, b.y, a.z),
                    Point3::new(a.x, b.y, b.z),
                    Point3::new(b.x, a.y, a.z),
                    Point3::new(b.x, a.y, b.z),
                    Point3::new(b.x, b.y, a.z),
                    Point3::new(b.x, b.y, b.z),
                ]
            };
            debug_lines.extend(local_corners.map(|corner| DebugLine {
                p1: probe_pos.into(),
                p2: (corner + probe_pos.coords).into(),
                color: Color32::RED.to_array(),
                packed_fields: DebugPackedFlags::NONE,
            }));
            let mut min_azimuth = f32::INFINITY;
            let mut max_azimuth = f32::NEG_INFINITY;
            let mut min_elevation = f32::INFINITY;
            let mut max_elevation = f32::NEG_INFINITY;
            for corner in local_corners {
                // let corner = Point3::new(corner.x, -corner.z, corner.y);
                let azimuth =
                    corner.y.signum() * f32::acos(corner.x / corner.xy().coords.magnitude());
                let elevation = f32::acos(corner.z / corner.coords.magnitude());
                min_azimuth = min_azimuth.min(azimuth);
                max_azimuth = max_azimuth.max(azimuth);
                min_elevation = min_elevation.min(elevation);
                max_elevation = max_elevation.max(elevation);
            }
            fn random_float_01(seed: u32) -> f32 {
                let hashed = {
                    let mut x = seed;
                    x = x.wrapping_add(x << 10);
                    x ^= x >> 6;
                    x = x.wrapping_add(x << 3);
                    x ^= x >> 11;
                    x = x.wrapping_add(x << 15);
                    x
                };
                const MANTISSA_MASK: u32 = 0x007F_FFFF;
                const ONE: u32 = 0x3F80_0000;
                let mut float_bits = hashed & MANTISSA_MASK;
                float_bits |= ONE;
                f32::from_bits(float_bits) - 1.0
            }
            let azimuth_diff = max_azimuth - min_azimuth;
            let elevation_diff = max_elevation - min_elevation;
            // let mut rng = rand::rngs::StdRng::seed_from_u64(0xDEADBEEF);
            for ray_i in 0..256 {
                // let azimuth = rng.gen_range(min_azimuth..=max_azimuth);
                // let elevation = rng.gen_range(min_elevation..=max_elevation);
                let seed = ray_i as u32 * 2 + 0xDEADBEEF;
                let azimuth = (random_float_01(seed) * azimuth_diff) + min_azimuth;
                let elevation = (random_float_01(seed + 1) * elevation_diff) + min_elevation;
                let sin_azim = azimuth.sin();
                let cos_azim = azimuth.cos();
                let sin_elev = elevation.sin();
                let cos_elev = elevation.cos();
                let radius = 1.0;
                let x = radius * sin_elev * cos_azim;
                let y = radius * sin_elev * sin_azim;
                let z = radius * cos_elev;
                // let [x, y, z] = [x, z, -y];
                debug_lines.push(DebugLine {
                    p1: probe_pos.into(),
                    p2: (Point3::new(x, y, z) + probe_pos.coords).into(),
                    color: Color32::PURPLE.to_array(),
                    packed_fields: DebugPackedFlags::NONE,
                });
            }
        }
        if debug_state.radiance_cascades_ray_visualiser {
            use nalgebra::{Point3, Vector3};
            let graphics_camera = &graphics_state.camera;
            // let start_pos = Point3::new(17.5, 157.0, -13.5);
            let start_pos = Point3::new(16.03125, 155.0, -16.21875);
            // fn append_rays(
            //     output_dirs: &mut Vec<Vector3<f32>>,
            //     current_dir: Vector3<f32>,
            //     current_cascade_i: i32,
            //     target_cascade_i: i32,
            // ) {
            //     let raw_ray_dirs = [
            //         Vector3::new(-1.0, 1.0, 1.0),
            //         Vector3::new(1.0, 1.0, 1.0),
            //         Vector3::new(-1.0, 1.0, -1.0),
            //         Vector3::new(1.0, 1.0, -1.0),
            //     ]
            //     .map(|raw_ray_dir| raw_ray_dir.normalize());
            //     for dir in raw_ray_dirs {
            //         let dir_modifier = dir / (2.0_f32).powi(current_cascade_i);
            //         let new_dir = (current_dir + dir_modifier).normalize();
            //         if current_cascade_i < target_cascade_i {
            //             append_rays(output_dirs, new_dir, current_cascade_i + 1, target_cascade_i);
            //         } else {
            //             output_dirs.push(new_dir);
            //         }
            //     }
            // }
            // let rays = {
            //     let mut rays = Vec::new();
            //     append_rays(&mut rays, Vector3::zeros(), 0, debug_state.max_radiance_cascade);
            //     rays
            // };
            let rays = {
                let mut rays = Vec::new();
                let phi = core::f32::consts::PI * ((5.0_f32).sqrt() - 1.0);
                let num_samples = 16 * 8_i32.pow(debug_state.max_radiance_cascade);
                for ray_i in 0..num_samples {
                    let y = 1.0 - (ray_i as f32 / (num_samples * 2 - 1) as f32) * 2.0;
                    let radius = (1.0 - y.powi(2)).sqrt();
                    let theta = phi * ray_i as f32;
                    let x = theta.cos() * radius;
                    let z = theta.sin() * radius;
                    rays.push(Vector3::new(x, y, z));
                }
                rays
            };
            for ray_dir in rays {
                // let ray_raw = match ray_i {
                //     0 => Vector3::new(-1.0, 1.0, 1.0),
                //     1 => Vector3::new(1.0, 1.0, 1.0),
                //     2 => Vector3::new(-1.0, 1.0, -1.0),
                //     3 => Vector3::new(1.0, 1.0, -1.0),
                //     _ => unreachable!(),
                // };
                // let ray_dir = ray_raw.normalize();
                let ray_start = if debug_state.max_radiance_cascade > 0 {
                    start_pos
                        + (ray_dir
                            * (1.0 / 16.0)
                            * (8.0_f32).powi(debug_state.max_radiance_cascade as i32 - 1))
                } else {
                    start_pos
                };
                let ray_end = start_pos
                    + (ray_dir
                        * (1.0 / 16.0)
                        * (8.0_f32).powi(debug_state.max_radiance_cascade as i32));
                let max_dist: f32 = 25.0;
                let start_dist = (graphics_camera.pos - ray_start).magnitude();
                let end_dist = (graphics_camera.pos - ray_end).magnitude();
                let average_dist = f32::min(start_dist, end_dist);
                let alpha = (1.0 - (average_dist / max_dist.max(0.01))).max(0.0);
                match debug_state.visualisation_draw_method {
                    DebugVisualisationDrawMethod::Egui => {
                        let Some(line) =
                            debug_clip_and_project_line([ray_start, ray_end], graphics_camera)
                        else {
                            continue;
                        };
                        painter.add(Shape::line_segment(
                            line.map(|p| {
                                Pos2::new(
                                    (p.x + 1.0) / 2.0 * width_f32,
                                    (-p.y + 1.0) / 2.0 * height_f32,
                                )
                            }),
                            (5.0 * alpha, Color32::RED.gamma_multiply(alpha)),
                        ));
                    }
                    DebugVisualisationDrawMethod::Gpu => debug_lines.push(DebugLine {
                        p1: ray_start.into(),
                        p2: ray_end.into(),
                        color: Color32::RED.gamma_multiply(alpha).to_array(),
                        packed_fields: DebugPackedFlags::IGNORE_DEPTH,
                    }),
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
                let from_centre_dist = (graphics_camera.pos - chunk_centres[0]).magnitude();
                let to_centre_dist = (graphics_camera.pos - chunk_centres[1]).magnitude();
                if from_centre_dist > max_dist || to_centre_dist > max_dist {
                    continue;
                }
                let average_dist = (from_centre_dist + to_centre_dist) / 2.0;
                let alpha = (1.0 - (average_dist / max_dist.max(0.01))).max(0.0);
                if let Some(line) = debug_clip_and_project_line(chunk_centres, graphics_camera) {
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
        // Draw world debug visuals with egui, or pass through for GPU rendering
        match debug_state.visualisation_draw_method {
            DebugVisualisationDrawMethod::Egui => {
                let graphics_camera = &graphics_state.camera;
                // TODO: Points and triangles
                for line in debug_lines.drain(..) {
                    let points = [line.p1, line.p2].map(Point3::from);
                    if let Some(screen_line) = debug_clip_and_project_line(points, graphics_camera)
                    {
                        let [r, g, b, a] = line.color;
                        painter.add(Shape::line_segment(
                            screen_line.map(|p| {
                                Pos2::new(
                                    (p.x + 1.0) / 2.0 * width_f32,
                                    (-p.y + 1.0) / 2.0 * height_f32,
                                )
                            }),
                            (2.0, Color32::from_rgba_unmultiplied(r, g, b, a)),
                        ));
                    }
                }
            }
            DebugVisualisationDrawMethod::Gpu => {}
        }
    });
    DebugRenderOutput {
        egui_output,
        debug_points,
        debug_lines,
        debug_triangles,
    }
}

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
            core::mem::swap(&mut vec1, &mut vec2);
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
        -crate::client::graphics::DEFAULT_ZFAR,
        -crate::client::graphics::DEFAULT_ZNEAR
    );
    for point in &mut vec1 {
        *point = camera.proj_matrix.project_point(point);
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
        -crate::client::graphics::DEFAULT_ZFAR,
        -crate::client::graphics::DEFAULT_ZNEAR
    );
    line = line.map(|p| camera.proj_matrix.project_point(&p));
    clip_edge_pair!(x, clip_intersection_x, -1.0, 1.0);
    clip_edge_pair!(y, clip_intersection_y, -1.0, 1.0);
    Some(line)
}
