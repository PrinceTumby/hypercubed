#[cfg(feature = "mini_std")]
pub mod debug;
pub mod game;
pub mod graphics;
pub mod input;
pub mod world;

use crate::physics::PlayerPhysicsState;
use crate::portable_prelude::*;
use crate::protocol::chunk as protocol_chunk;
use crate::protocol::play::{Clientbound as ClientboundPacket, GameMode};
use crate::protocol::prelude::*;
#[allow(unused)]
use anyhow::Context;
use graphics::GraphicsState;
use nalgebra::Point3;
use portable_std::sync::mpsc;
#[allow(unused)]
use portable_std::{Arc, FastHashMap, FastHashSet, VecDeque};

use crate::platform::libs::{egui, winit};
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, RawKeyEvent, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::{Fullscreen, Window, WindowId};

cfg_if::cfg_if! {
    if #[cfg(feature = "full_std")] {
        use std::time::Instant;
        use threadpool::ThreadPool;
    } else {
        use crate::platform::time::Instant;
    }
}

// TODO: Update winit! Give Render Bundles a go as an alternative for MDI!
// TODO: (^) First, try out secondary command buffers in Vulkan. Nice way to get started on
//       graphics settings that need specialisation.

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

cfg_if::cfg_if! {
    if #[cfg(feature = "graphics_backend_opengl")] {
        #[derive(Debug)]
        pub struct RawSubchunk {
            pub start_coords: [i32; 3],
            pub face_groups: [Vec<graphics::chunk::BlockFace>; 7],
            pub connected_faces: graphics::chunk::SubchunkConnectivity,
        }
    } else {
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
            pub start_face_and_len: [u32; 2],
            pub instances: Vec<graphics::chunk::custom_block::Instance>,
        }
    }
}

pub const SUBCHUNK_AXIS_LEN: usize = 16;
pub const SUBCHUNK_AXIS_LEN_I32: i32 = SUBCHUNK_AXIS_LEN as i32;
pub const MIN_HEIGHT_I32: i32 = -64;
pub const MAX_HEIGHT_I32: i32 = 319;

pub struct App {
    window: Option<Arc<Window>>,
    graphics_state: Option<GraphicsState>,
    input_state: input::PlayControlState,
    play_state: ClientPlayState,
    server_connection: Arc<PlayConnection>,
    clientbound_tx: mpsc::Sender<ClientboundPacket>,
    clientbound_rx: mpsc::Receiver<ClientboundPacket>,
    play_state_update_tx: mpsc::Sender<ClientPlayStateUpdate>,
    play_state_update_rx: mpsc::Receiver<ClientPlayStateUpdate>,
    debug_state: graphics::DebugState,
    debug_output: graphics::DebugOutput,
    egui_ctx: egui::Context,
    pending_egui_events: Vec<egui::Event>,
    last_mouse_pos: egui::Pos2,
    current_update_id: usize,
    previous_frame_times: VecDeque<f64>,
    last_frame_time: Instant,
    current_time_s: f64,
    last_tick_time_s: f64,
    next_tick_time_s: f64,
    #[cfg(feature = "full_std")]
    thread_pool: ThreadPool,
}

impl App {
    pub fn new(
        server_connection: Arc<PlayConnection>,
        clientbound_tx: mpsc::Sender<ClientboundPacket>,
        clientbound_rx: mpsc::Receiver<ClientboundPacket>,
    ) -> Self {
        let input_state = input::PlayControlState::default();
        let play_state = ClientPlayState {
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
        let egui_ctx = egui::Context::default();
        let (play_state_update_tx, play_state_update_rx) = mpsc::channel::<ClientPlayStateUpdate>();
        let thread_pool = ThreadPool::new(
            std::thread::available_parallelism()
                .map(|num_threads_non_zero| num_threads_non_zero.get())
                .unwrap_or(2),
        );
        Self {
            window: None,
            graphics_state: None,
            input_state,
            play_state,
            server_connection,
            clientbound_tx,
            clientbound_rx,
            play_state_update_tx,
            play_state_update_rx,
            debug_state: graphics::DebugState::default(),
            debug_output: graphics::DebugOutput {
                subchunks_culled: 0,
                subchunk_traversal_graph: Vec::new(),
            },
            egui_ctx,
            pending_egui_events: Vec::new(),
            last_mouse_pos: egui::Pos2::new(0.0, 0.0),
            previous_frame_times: VecDeque::new(),
            current_update_id: 0,
            last_frame_time: Instant::now(),
            current_time_s: 0.0,
            last_tick_time_s: 0.0,
            next_tick_time_s: 1.0 / 20.0,
            thread_pool,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("Hypercubed"))
                .unwrap(),
        );
        let graphics_state =
            GraphicsState::new(window.clone(), resources::block::register_vanilla_blocks).unwrap();
        self.window = Some(window);
        self.graphics_state = Some(graphics_state);
        event_loop.set_control_flow(ControlFlow::Poll);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let window = self.window.as_ref().unwrap();
        let graphics_state = self.graphics_state.as_mut().unwrap();
        if id != window.id() {
            return;
        }
        let scale_factor = window.scale_factor();
        match event {
            WindowEvent::CloseRequested | WindowEvent::Destroyed => event_loop.exit(),
            WindowEvent::Resized(physical_size) => graphics_state.resize(physical_size),
            // TODO: Implement egui keyboard support
            // WindowEvent::KeyboardInput {
            //     device_id: _,
            //     event,
            //     is_synthetic: _,
            // } if !input_state.mouse_locked => {}
            WindowEvent::CursorMoved {
                device_id: _,
                position,
            } if !self.input_state.mouse_locked => {
                self.last_mouse_pos = egui::Pos2 {
                    x: (position.x / scale_factor) as f32,
                    y: (position.y / scale_factor) as f32,
                };
                self.pending_egui_events
                    .push(egui::Event::PointerMoved(self.last_mouse_pos));
            }
            WindowEvent::CursorLeft { device_id: _ } => {
                self.pending_egui_events.push(egui::Event::PointerGone)
            }
            WindowEvent::MouseInput {
                device_id: _,
                state,
                button,
            } if !self.input_state.mouse_locked => {
                self.pending_egui_events.push(egui::Event::PointerButton {
                    pos: self.last_mouse_pos,
                    button: match button {
                        winit::event::MouseButton::Left => egui::PointerButton::Primary,
                        winit::event::MouseButton::Right => egui::PointerButton::Secondary,
                        winit::event::MouseButton::Middle => egui::PointerButton::Middle,
                        winit::event::MouseButton::Back => egui::PointerButton::Extra1,
                        winit::event::MouseButton::Forward => egui::PointerButton::Extra2,
                        // For non-winit platforms, we define our own `winit` inside the same
                        // crate, so `#[non_exhaustive]` does nothing.
                        // Because of this, we have to just suppress the warning here.
                        #[allow(unreachable_patterns)]
                        _ => return,
                    },
                    pressed: state.is_pressed(),
                    modifiers: egui::Modifiers::NONE,
                })
            }
            _ => {}
        }
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if cause == StartCause::Init {
            return;
        }
        let window = self.window.as_ref().unwrap();
        let graphics_state = self.graphics_state.as_mut().unwrap();
        let scale_factor = window.scale_factor();
        let new_time = Instant::now();
        let delta_time_f64 =
            (new_time - core::mem::replace(&mut self.last_frame_time, new_time)).as_secs_f64();
        self.previous_frame_times.push_back(delta_time_f64);
        if self.previous_frame_times.len() > 100 {
            self.previous_frame_times.pop_front();
        }
        self.current_time_s += delta_time_f64;
        let delta_time = delta_time_f64 as f32;
        // Debug GUI
        #[cfg(feature = "full_std")]
        let debug::DebugRenderOutput {
            egui_output,
            debug_points,
            debug_lines,
            debug_triangles,
        } = debug::render_debug_ui(
            event_loop,
            &self.server_connection,
            &mut self.play_state,
            graphics_state,
            &mut self.debug_state,
            &self.debug_output,
            &self.egui_ctx,
            &mut self.pending_egui_events,
            &self.previous_frame_times,
            scale_factor,
            self.current_time_s,
            delta_time_f64,
            delta_time,
        );
        // Reset cursor to middle if locked.
        if self.input_state.mouse_locked && window.has_focus() {
            let size = graphics_state.size;
            let physical_x = size.width as i32 / 2;
            let physical_y = size.height as i32 / 2;
            self.last_mouse_pos = egui::Pos2 {
                x: (physical_x as f64 / scale_factor) as f32,
                y: (physical_y as f64 / scale_factor) as f32,
            };
            _ = window
                .set_cursor_position(winit::dpi::PhysicalPosition::new(physical_x, physical_y));
        }
        // Main rendering
        cfg_if::cfg_if! {
            if #[cfg(feature = "graphics_backend_vulkan")] {
                self.debug_output = graphics_state.render(
                    &self.play_state.subchunks,
                    &self.play_state.visible_chunks,
                    &self.egui_ctx,
                    egui_output,
                    &self.debug_state,
                    &debug_points,
                    &debug_lines,
                    &debug_triangles,
                )
                .context("Error while rendering")
                .unwrap();
            } else if #[cfg(feature = "graphics_backend_wgpu")] {
                match graphics_state.render(
                    &self.play_state.subchunks,
                    &self.play_state.visible_chunks,
                    &self.egui_ctx,
                    egui_output,
                    &self.debug_state,
                    &debug_points,
                    &debug_lines,
                    &debug_triangles,
                ) {
                    Ok(new_debug_output) => self.debug_output = new_debug_output,
                    Err(wgpu::SurfaceError::Timeout) => {}
                    // Reconfigure the surface if lost.
                    Err(wgpu::SurfaceError::Lost) | Err(wgpu::SurfaceError::Outdated) => {
                        let size = graphics_state.size;
                        graphics_state.resize(size)
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                    Err(other) => panic!("Rendering error: {other}"),
                }
            } else if #[cfg(feature = "graphics_backend_opengl")] {
                self.debug_output = graphics_state.render(
                    &mut self.play_state.subchunks,
                    &self.play_state.visible_chunks,
                    &self.egui_ctx,
                    egui_output,
                    &self.debug_state,
                    &debug_points,
                    &debug_lines,
                    &debug_triangles,
                )
                .unwrap();
            } else if #[cfg(feature = "graphics_backend_software")] {
                self.debug_output = graphics_state.render(
                    &self.play_state.subchunks,
                    &self.play_state.visible_chunks,
                    &self.egui_ctx,
                    egui_output,
                    &self.debug_state,
                )
                .unwrap();
            } else {
                compile_error!("TODO (graphics backend): graphics_state.render(...)");
            }
        }
        // Gameplay events and updates
        game::process_game_events(
            #[cfg(feature = "full_std")]
            &self.thread_pool,
            &mut self.play_state,
            graphics_state,
            &mut self.debug_state,
            &mut self.input_state,
            &self.server_connection,
            &self.clientbound_tx,
            &self.clientbound_rx,
            &self.play_state_update_tx,
            &self.play_state_update_rx,
            &mut self.current_update_id,
            self.current_time_s,
            &mut self.last_tick_time_s,
            &mut self.next_tick_time_s,
            delta_time,
        );
        #[cfg(feature = "tracy")]
        tracing_tracy::client::frame_mark();
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        let window = self.window.as_ref().unwrap();
        let graphics_state = self.graphics_state.as_mut().unwrap();
        match event {
            DeviceEvent::MouseMotion { delta } if self.input_state.mouse_locked => {
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
                let old_mouse_locked = self.input_state.mouse_locked;
                let old_fullscreen = self.input_state.fullscreen;
                self.input_state.update_from_input(
                    physical_key,
                    state,
                    // Toggle sprint if these conditions, else just enable sprint on press.
                    self.play_state.player.game_mode == GameMode::Spectator
                        || self.debug_state.free_cam,
                );
                if self.input_state.mouse_locked != old_mouse_locked {
                    use winit::window::CursorGrabMode;
                    if self.input_state.mouse_locked {
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
                            self.pending_egui_events.push(egui::Event::PointerButton {
                                pos: self.last_mouse_pos,
                                button: mouse_button,
                                pressed: false,
                                modifiers: egui::Modifiers::NONE,
                            });
                        }
                        self.pending_egui_events.push(egui::Event::PointerGone);
                    } else {
                        // Releasing the cursor shouldn't ever fail.
                        _ = window.set_cursor_grab(CursorGrabMode::None);
                        window.set_cursor_visible(true);
                        self.pending_egui_events
                            .push(egui::Event::PointerMoved(self.last_mouse_pos));
                    }
                }
                if self.input_state.fullscreen != old_fullscreen {
                    if self.input_state.fullscreen {
                        window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                    } else {
                        window.set_fullscreen(None);
                    }
                }
            }
            _ => {}
        }
    }
}
