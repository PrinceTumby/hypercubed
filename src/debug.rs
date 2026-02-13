use crate::ClientPlayState;
use crate::graphics::debug::Line as DebugLine;
use crate::graphics::debug::Point as DebugPoint;
use crate::graphics::debug::Triangle as DebugTriangle;
use crate::graphics::{self, Camera, DebugVisualisationDrawMethod, GraphicsBackend};
use crate::platform::libs::winit;
use crate::portable_prelude::*;
use crate::protocol::PlayConnection;
use crate::protocol::play::GameMode;
use nalgebra::Point3;
use portable_std::VecDeque;

pub struct DebugRenderOutput {
    pub egui_output: egui::FullOutput,
    pub debug_points: Vec<DebugPoint>,
    pub debug_lines: Vec<DebugLine>,
    pub debug_triangles: Vec<DebugTriangle>,
}

#[expect(clippy::too_many_arguments)]
pub fn render_debug_ui(
    event_loop: &winit::event_loop::ActiveEventLoop,
    server_connection: &PlayConnection,
    play_state: &mut ClientPlayState,
    graphics_backend: &mut dyn GraphicsBackend,
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
    let subchunks = graphics_backend.get_subchunks_data();
    let graphics_size = graphics_backend.get_size();
    let raw_input = egui::RawInput {
        viewport_id: egui::viewport::ViewportId::ROOT,
        viewports: egui::viewport::ViewportIdMap::from_iter([(
            egui::viewport::ViewportId::ROOT,
            egui::ViewportInfo {
                native_pixels_per_point: Some(scale_factor as f32),
                ..Default::default()
            },
        )]),
        screen_rect: Some(egui::Rect {
            min: egui::Pos2::ZERO,
            max: egui::Pos2::new(
                graphics_size.width as f32 / scale_factor as f32,
                graphics_size.height as f32 / scale_factor as f32,
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
        let width_f32 = graphics_size.width as f32 / scale_factor as f32;
        let height_f32 = graphics_size.height as f32 / scale_factor as f32;
        let painter = Painter::new(ctx.clone(), LayerId::background(), Rect::EVERYTHING);
        Window::new("Debug Info").resizable(false).show(ctx, |ui| {
            ui.label(format!("FPS: {:.2}", 1.0 / delta_time_f64));
            // Quit button.
            // Useful for fullscreen mode and embedded platforms.
            if ui.button("Quit").clicked() {
                event_loop.exit();
            }
            // Graphics options
            {
                let old_graphics_options = graphics_backend.get_graphics_options();
                let mut new_graphics_options = old_graphics_options;
                // VSync
                ui.checkbox(&mut new_graphics_options.vsync, "VSync");
                // Lightmap gamma ("brightness")
                ui.add(
                    egui::Slider::new(&mut new_graphics_options.lightmap_gamma_setting, 0.0..=1.0)
                        .text("Brightness gamma"),
                );
                if new_graphics_options != old_graphics_options {
                    graphics_backend.apply_new_graphics_options(new_graphics_options);
                }
            }
            ui.label(format!("Position: {:.2?}", play_state.camera.pos));
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
                    let camera = &mut play_state.camera;
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
                let block_registry = graphics_backend.get_block_registry();
                let raw_chunks = &play_state.raw_chunks;
                let pos = play_state.camera.pos.coords;
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
                        let blockstate = &block_registry[global_palette_index];
                        let identifier = block_registry
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
                        let line = Line::new("", points);
                        plot_ui.line(line);
                    });
            });
            if graphics_backend.wants_egui_debug_section() {
                ui.collapsing("Graphics Backend", |ui| {
                    graphics_backend.render_egui_debug_section(ui);
                });
            }
        });
        if debug_state.rendering_view_frustum {
            use nalgebra::Point3;
            let camera = &play_state.camera;
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
            let graphics_camera = &play_state.camera;
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
                let pairs = subchunk.connectivity.get_pairs();
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
                        debug_lines.push(DebugLine {
                            p1: line[0].into(),
                            p2: line[1].into(),
                            colour: Color32::from_rgba_unmultiplied(0xFF, 0x00, 0xFF, 0xFF)
                                .gamma_multiply(alpha)
                                .to_srgba_unmultiplied(),
                            size: 5.0 * alpha,
                            flags: graphics::debug::PackedFlags::IGNORE_DEPTH,
                        });
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
                    debug_lines.push(DebugLine {
                        p1: pair_centre.into(),
                        p2: end_1.into(),
                        colour: colour.gamma_multiply(alpha).to_srgba_unmultiplied(),
                        size: 5.0 * alpha,
                        flags: graphics::debug::PackedFlags::IGNORE_DEPTH,
                    });
                    debug_lines.push(DebugLine {
                        p1: pair_centre.into(),
                        p2: end_2.into(),
                        colour: colour.gamma_multiply(alpha).to_srgba_unmultiplied(),
                        size: 5.0 * alpha,
                        flags: graphics::debug::PackedFlags::IGNORE_DEPTH,
                    });
                }
            }
        }
        if debug_state.cave_cull_render_traversal_graph {
            use nalgebra::Point3;
            let graphics_camera = &play_state.camera;
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
                debug_lines.push(DebugLine {
                    p1: chunk_centres[0].into(),
                    p2: chunk_centres[1].into(),
                    colour: Color32::YELLOW
                        .gamma_multiply(alpha)
                        .to_srgba_unmultiplied(),
                    size: 5.0 * alpha,
                    flags: graphics::debug::PackedFlags::IGNORE_DEPTH,
                });
            }
        }
        {
            // TODO: Sort debug lines, points and triangles by camera distance.
        }
        // Draw world debug visuals with egui, or pass through for GPU rendering.
        match debug_state.visualisation_draw_method {
            DebugVisualisationDrawMethod::Egui => {
                let graphics_camera = &play_state.camera;
                // TODO: Points and triangles
                for line in debug_lines.drain(..) {
                    let points = [line.p1, line.p2].map(Point3::from);
                    if let Some(screen_line) = debug_clip_and_project_line(points, graphics_camera)
                    {
                        let [r, g, b, a] = line.colour;
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
        -crate::graphics::DEFAULT_ZFAR,
        -crate::graphics::DEFAULT_ZNEAR
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
        -crate::graphics::DEFAULT_ZFAR,
        -crate::graphics::DEFAULT_ZNEAR
    );
    line = line.map(|p| camera.proj_matrix.project_point(&p));
    clip_edge_pair!(x, clip_intersection_x, -1.0, 1.0);
    clip_edge_pair!(y, clip_intersection_y, -1.0, 1.0);
    Some(line)
}
