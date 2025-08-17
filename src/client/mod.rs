#[cfg(feature = "std")]
pub mod debug;
pub mod game;
pub mod graphics;
pub mod input;
pub mod world;

use crate::physics::PlayerPhysicsState;
use crate::portable_prelude::*;
#[allow(unused)]
use portable_std::{Arc, FastHashMap, FastHashSet, VecDeque};
use portable_std::sync::mpsc;
use crate::protocol::chunk as protocol_chunk;
use crate::protocol::play::{Clientbound as ClientboundPacket, GameMode};
use crate::protocol::prelude::*;
use graphics::GraphicsState;
use nalgebra::Point3;

#[cfg(feature = "std")]
use std_imports::*;
#[cfg(feature = "std")]
mod std_imports {
    pub use std::time::Instant;
    pub use threadpool::ThreadPool;
    pub use winit::event::{DeviceEvent, Event, RawKeyEvent, StartCause, WindowEvent};
    pub use winit::event_loop::{ControlFlow, EventLoop};
    pub use winit::window::{Fullscreen, WindowBuilder};
}

#[cfg(not(feature = "std"))]
use ps2_imports::*;
#[cfg(not(feature = "std"))]
mod ps2_imports {
    pub use crate::platform::libs::{egui, winit};
    pub use crate::platform::time::Instant;
    pub use winit::event::{DeviceEvent, Event, RawKeyEvent, StartCause, WindowEvent};
    pub use winit::event_loop::{ControlFlow, EventLoop};
    pub use winit::window::{Fullscreen, WindowBuilder};
}

pub struct ClientPlayState {
    pub raw_chunks: Arc<FastHashMap<[i32; 2], Arc<RawChunk>>>,
    // TODO: Currently the Y coordinate is a chunk section index, rather than the subchunk Y
    //       coordinate. Consider changing to actually be the Y coordinate.
    pub subchunks: FastHashMap<[i32; 3], graphics::chunk::Subchunk>,
    pub visible_chunks: FastHashSet<[i32; 2]>,
    pub pending_subchunk_update_ids: FastHashMap<[i32; 3], usize>,
    pub player: Player,
    pub player_last_tick: Player,
}

#[derive(Clone, Debug)]
pub struct Player {
    pub entity_id: EntityId,
    pub game_mode: GameMode,
    pub pos: Point3<f32>,
    pub yaw: f32,
    pub pitch: f32,
    pub physics_state: PlayerPhysicsState,
}

impl Player {
    pub fn get_mc_rot(&self) -> (f32, f32) {
        ((self.yaw - 180.0) % 360.0, -self.pitch)
    }

    pub fn set_mc_rot(&mut self, yaw: f32, pitch: f32) {
        self.yaw = (yaw + 180.0) % 360.0;
        self.pitch = -pitch.clamp(-90.0, 90.0);
    }
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
    pub tinted_block_face_instance_groups:
        [Vec<graphics::chunk::tinted_block_face::Instance>; 6],
    pub custom_block_groups: Vec<RawCustomBlockGroup>,
    pub connected_faces: graphics::chunk::SubchunkConnectivity,
    #[cfg(feature = "graphics_backend_vulkan")]
    pub rt_info: RayTracingInfo,
}

#[cfg(feature = "graphics_backend_vulkan")]
#[derive(Clone, Debug)]
pub struct RayTracingInfo {
    pub vertex_positions: Vec<[f32; 3]>,
    pub block_face_triangle_quads: Vec<[[u32; 3]; 2]>,
    pub tinted_block_face_triangle_quads: Vec<[[u32; 3]; 2]>,
    pub custom_block_face_triangle_quads: Vec<[[u32; 3]; 2]>,
    /// Combined quad info lists, with offsets specified in `quads_info_offsets`.
    pub quads_info: Vec<RayTracedQuadInfo>,
    /// Offsets of tinted and custom quad info lists.
    /// Block quad info starts at 0, so isn't specified.
    pub quads_info_offsets: [u32; 2],
}

#[cfg(feature = "graphics_backend_vulkan")]
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RayTracedQuadInfo {
    pub uvs: [[u16; 2]; 4],
    pub packed_fields: RayTracedQuadPackedFields,
}

#[cfg(feature = "graphics_backend_vulkan")]
bitfield::bitfield! {
    // 0-7: Unused
    // 8-31: Tint colour
    #[repr(transparent)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct RayTracedQuadPackedFields(u32);
    impl Debug;
    pub tint_colour, set_tint_colour: 31, 8;
}

#[derive(Debug)]
pub struct RawCustomBlockGroup {
    pub start_face_and_len: [u32; 2],
    pub instances: Vec<graphics::chunk::custom_block::Instance>,
}

pub const SUBCHUNK_AXIS_LEN: usize = 16;
pub const SUBCHUNK_AXIS_LEN_I32: i32 = SUBCHUNK_AXIS_LEN as i32;
pub const MIN_HEIGHT_I32: i32 = -64;
pub const MAX_HEIGHT_I32: i32 = 319;

#[cfg(feature = "std")]
pub async fn window_run(
    server_connection: Arc<PlayConnection>,
    clientbound_rx: mpsc::Receiver<ClientboundPacket>,
    clientbound_tx: mpsc::Sender<ClientboundPacket>,
) -> anyhow::Result<()> {
    use input::PlayControlState;
    let event_loop = EventLoop::new()?;
    // TODO: Change this to an `Arc`
    let window = Box::leak(Box::new(WindowBuilder::new().build(&event_loop)?));
    window.set_title("Rust Minecraft Client");
    let window_id = window.id();
    let mut scale_factor = window.scale_factor();
    let mut graphics_state =
        GraphicsState::new(window, resources::block::register_vanilla_blocks).await?;
    let mut input_state = PlayControlState::default();
    let egui_ctx = egui::Context::default();
    let mut play_state = ClientPlayState {
        raw_chunks: Arc::new(FastHashMap::new()),
        subchunks: FastHashMap::new(),
        visible_chunks: FastHashSet::new(),
        pending_subchunk_update_ids: FastHashMap::new(),
        player: Player {
            entity_id: EntityId::placeholder(),
            game_mode: GameMode::Survival,
            pos: Point3::origin(),
            yaw: 0.0,
            pitch: 0.0,
            physics_state: PlayerPhysicsState::default(),
        },
        player_last_tick: Player {
            entity_id: EntityId::placeholder(),
            game_mode: GameMode::Survival,
            pos: Point3::origin(),
            yaw: 0.0,
            pitch: 0.0,
            physics_state: PlayerPhysicsState::default(),
        },
    };
    let (play_state_update_tx, play_state_update_rx) = mpsc::channel::<ClientPlayStateUpdate>();
    let mut last_mouse_pos = egui::Pos2::new(0.0, 0.0);
    let mut events: Vec<egui::Event> = Vec::new();
    let mut debug_state = graphics::DebugState::default();
    let mut debug_output = graphics::DebugOutput {
        subchunks_culled: 0,
        subchunk_traversal_graph: Vec::new(),
    };
    #[cfg(feature = "std")]
    let thread_pool = ThreadPool::new(
        std::thread::available_parallelism()
            .map(|num_threads_non_zero| num_threads_non_zero.get())
            .unwrap_or(2),
    );
    let mut current_update_id: usize = 0;
    let mut previous_frame_times = VecDeque::new();
    let mut last_frame_time = Instant::now();
    let mut current_time_s: f64 = 0.0;
    let mut last_tick_time_s: f64 = 0.0;
    let mut next_tick_time_s: f64 = 1.0 / 20.0;
    let window = &window;
    #[allow(unused)]
    let mut debug_frame_i: usize = 0;
    event_loop.run(move |event, window_target| {
        window_target.set_control_flow(ControlFlow::Poll);
        match event {
            Event::NewEvents(StartCause::Poll) => {
                debug_frame_i += 1;
                let new_time = Instant::now();
                let delta_time_f64 =
                    (new_time - core::mem::replace(&mut last_frame_time, new_time)).as_secs_f64();
                previous_frame_times.push_back(delta_time_f64);
                if previous_frame_times.len() > 100 {
                    previous_frame_times.pop_front();
                }
                current_time_s += delta_time_f64;
                let delta_time = delta_time_f64 as f32;
                // Debug GUI
                #[cfg(feature = "std")]
                let debug::DebugRenderOutput {
                    egui_output,
                    debug_points,
                    debug_lines,
                    debug_triangles,
                } = debug::render_debug_ui(
                    &thread_pool,
                    &server_connection,
                    &mut play_state,
                    &mut graphics_state,
                    &mut debug_state,
                    &debug_output,
                    &egui_ctx,
                    &mut events,
                    &previous_frame_times,
                    scale_factor,
                    current_time_s,
                    delta_time_f64,
                    delta_time,
                );
                // Reset cursor to middle if locked
                if input_state.mouse_locked && window.has_focus() {
                    let size = graphics_state.size;
                    let physical_x = size.width as i32 / 2;
                    let physical_y = size.height as i32 / 2;
                    last_mouse_pos = egui::Pos2 {
                        x: (physical_x as f64 / scale_factor) as f32,
                        y: (physical_y as f64 / scale_factor) as f32,
                    };
                    _ = window.set_cursor_position(winit::dpi::PhysicalPosition::new(
                        physical_x, physical_y,
                    ));
                }
                // Main rendering
                cfg_if::cfg_if! {
                    if #[cfg(feature = "graphics_backend_vulkan")] {
                        // if debug_frame_i > 144 * 2 {
                        //     graphics_state.radiance_cascades_debug_render(&play_state.subchunks);
                        // }
                        // XXX: DEBUG
                        // if debug_frame_i == 144 * 2 {
                        //     eprintln!("Updating subchunk lighting!");
                        //     graphics_state.update_all_subchunks_radiance_lighting(
                        //         &thread_pool,
                        //         &play_state.subchunks,
                        //         &play_state.raw_chunks,
                        //     );
                        // }
                        match graphics_state.render(
                            &play_state.subchunks,
                            &play_state.visible_chunks,
                            &egui_ctx,
                            egui_output,
                            &debug_state,
                            &debug_points,
                            &debug_lines,
                            &debug_triangles,
                        ) {
                            Ok(new_debug_output) => debug_output = new_debug_output,
                            // FIXME: Call resize if there's a swapchain error
                            Err(err) => panic!("Rendering error: {err}"),
                        };
                    } else if #[cfg(feature = "graphics_backend_wgpu")] {
                        match graphics_state.render(
                            &play_state.subchunks,
                            &play_state.visible_chunks,
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
                            Err(wgpu::SurfaceError::OutOfMemory) => window_target.exit(),
                        }
                    } else if #[cfg(feature = "graphics_backend_software")] {
                        debug_output = graphics_state.render(
                            &play_state.subchunks,
                            &play_state.visible_chunks,
                            &egui_ctx,
                            egui_full_output,
                            &debug_state,
                        )
                        .unwrap();
                    }
                }
                // Gameplay events and updates
                game::process_game_events(
                    #[cfg(feature = "std")]
                    &thread_pool,
                    &mut play_state,
                    &mut graphics_state,
                    &mut debug_state,
                    &mut input_state,
                    &server_connection,
                    &clientbound_tx,
                    &clientbound_rx,
                    &play_state_update_tx,
                    &play_state_update_rx,
                    &mut current_update_id,
                    current_time_s,
                    &mut last_tick_time_s,
                    &mut next_tick_time_s,
                    delta_time,
                );
                #[cfg(feature = "tracy")]
                tracing_tracy::client::frame_mark();
            }
            Event::WindowEvent {
                window_id: event_window_id,
                ref event,
            } if event_window_id == window_id => match event {
                WindowEvent::CloseRequested | WindowEvent::Destroyed => window_target.exit(),
                WindowEvent::Resized(physical_size) => {
                    graphics_state.resize(*physical_size);
                }
                WindowEvent::ScaleFactorChanged {
                    scale_factor: new_scale_factor,
                    inner_size_writer: _,
                } => scale_factor = *new_scale_factor,
                // TODO: Implement egui keyboard support
                // WindowEvent::KeyboardInput {
                //     device_id: _,
                //     event,
                //     is_synthetic: _,
                // } if !input_state.mouse_locked => {}
                WindowEvent::CursorMoved {
                    device_id: _,
                    position,
                } if !input_state.mouse_locked => {
                    last_mouse_pos = egui::Pos2 {
                        x: (position.x / scale_factor) as f32,
                        y: (position.y / scale_factor) as f32,
                    };
                    events.push(egui::Event::PointerMoved(last_mouse_pos));
                }
                WindowEvent::CursorLeft { device_id: _ } => events.push(egui::Event::PointerGone),
                WindowEvent::MouseInput {
                    device_id: _,
                    state,
                    button,
                } if !input_state.mouse_locked => events.push(egui::Event::PointerButton {
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
            Event::DeviceEvent {
                device_id: _,
                event,
            } if window.has_focus() => match event {
                DeviceEvent::MouseMotion { delta } if input_state.mouse_locked => {
                    const MOUSE_SENSITIVITY: f32 = 0.1;
                    let camera = &mut graphics_state.camera;
                    camera.yaw += delta.0 as f32 * MOUSE_SENSITIVITY;
                    camera.pitch -= delta.1 as f32 * MOUSE_SENSITIVITY;
                    camera.pitch = camera.pitch.clamp(-90.0, 90.0);
                    while camera.yaw < 0.0 {
                        camera.yaw += 360.0;
                    }
                    while camera.yaw > 360.0 {
                        camera.yaw -= 360.0;
                    }
                }
                DeviceEvent::Key(RawKeyEvent {
                    physical_key,
                    state,
                }) => {
                    let old_mouse_locked = input_state.mouse_locked;
                    let old_fullscreen = input_state.fullscreen;
                    input_state.update_from_input(
                        physical_key,
                        state,
                        // Toggle sprint if these conditions, else just enable sprint on press.
                        play_state.player.game_mode == GameMode::Spectator || debug_state.free_cam,
                    );
                    if input_state.mouse_locked != old_mouse_locked {
                        use winit::window::CursorGrabMode;
                        if input_state.mouse_locked {
                            // No issue if locking the cursor doesn't work, we hide it and
                            // keep setting the position to the centre anyway.
                            _ = window
                                .set_cursor_grab(CursorGrabMode::Locked)
                                .or_else(|_e| window.set_cursor_grab(CursorGrabMode::Confined));
                            window.set_cursor_visible(false);
                            // Report to egui that we've released all mouse buttons
                            let mouse_buttons = [
                                egui::PointerButton::Primary,
                                egui::PointerButton::Secondary,
                                egui::PointerButton::Middle,
                                egui::PointerButton::Extra1,
                                egui::PointerButton::Extra2,
                            ];
                            for mouse_button in mouse_buttons {
                                events.push(egui::Event::PointerButton {
                                    pos: last_mouse_pos,
                                    button: mouse_button,
                                    pressed: false,
                                    modifiers: egui::Modifiers::NONE,
                                });
                            }
                            events.push(egui::Event::PointerGone);
                        } else {
                            // Releasing the cursor never fails.
                            window.set_cursor_grab(CursorGrabMode::None).unwrap();
                            window.set_cursor_visible(true);
                            events.push(egui::Event::PointerMoved(last_mouse_pos));
                        }
                    }
                    if input_state.fullscreen != old_fullscreen {
                        if input_state.fullscreen {
                            window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                        } else {
                            window.set_fullscreen(None);
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }
    })?;
    Ok(())
}
