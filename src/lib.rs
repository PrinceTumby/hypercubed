#![warn(clippy::all)]
#![deny(clippy::correctness)]
#![deny(
    clippy::std_instead_of_core,
    reason = "we want to be portable to platforms with `no_std`"
)]
#![deny(
    clippy::std_instead_of_alloc,
    reason = "the types in `crate::prelude` can be used for portability"
)]
#![deny(clippy::alloc_instead_of_core)]
#![cfg_attr(not(feature = "mini_std"), no_std)]
#![cfg_attr(feature = "graphics_backend_software", feature(portable_simd))]

#[cfg(not(any(feature = "mini_std", test)))]
#[macro_use]
extern crate alloc;

// #[cfg(not(feature = "mini_std"))]
// extern crate portable_std as std;
// TODO: Try this (^), to see if we can simplify imports in crates.
// TODO: Rename `portable_std` to `hypercubed_std`.

pub mod portable_prelude {
    pub use portable_std::prelude::*;

    cfg_if::cfg_if! {
        if #[cfg(not(feature = "full_std"))] {
            #[allow(unused)]
            pub(crate) use crate::platform::{dbg, println, eprintln};
            pub use nalgebra::{ComplexField, RealField};
        } else {
            pub use std::{dbg, println, eprintln};
        }
    }
}

pub mod basic_types;
#[cfg(feature = "mini_std")]
pub mod debug;
pub mod game;
pub mod graphics;
pub mod input;
pub mod physics;
pub mod platform;
pub mod protocol;
pub mod world;

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL_ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

use crate::physics::PlayerPhysicsState;
use crate::portable_prelude::*;
use crate::protocol::chunk as protocol_chunk;
use crate::protocol::play::{Clientbound as ClientboundPacket, GameMode};
use crate::protocol::prelude::*;
#[allow(unused)]
use anyhow::Context;
use graphics::{Camera, GraphicsBackend, SelectedGraphicsBackend};
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
    pub camera: Camera,
    pub raw_chunks: Arc<FastHashMap<[i32; 2], Arc<RawChunk>>>,
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

pub const SUBCHUNK_AXIS_LEN: usize = 16;
pub const SUBCHUNK_AXIS_LEN_I32: i32 = SUBCHUNK_AXIS_LEN as i32;
pub const MIN_HEIGHT_I32: i32 = -64;
pub const MAX_HEIGHT_I32: i32 = 319;

pub struct App {
    window: Option<Arc<Window>>,
    #[cfg(any(feature = "platform_winit", feature = "platform_linux_drm"))]
    selected_graphics_backend: SelectedGraphicsBackend,
    graphics_backend: Option<Box<dyn GraphicsBackend>>,
    input_state: input::PlayControlState,
    play_state: ClientPlayState,
    server_connection: Arc<PlayConnection>,
    clientbound_tx: mpsc::Sender<ClientboundPacket>,
    clientbound_rx: mpsc::Receiver<ClientboundPacket>,
    debug_state: graphics::DebugState,
    debug_output: graphics::DebugOutput,
    egui_ctx: egui::Context,
    pending_egui_events: Vec<egui::Event>,
    last_mouse_pos: egui::Pos2,
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
        #[cfg(any(feature = "platform_winit", feature = "platform_linux_drm"))]
        selected_graphics_backend: Option<SelectedGraphicsBackend>,
    ) -> Self {
        let input_state = input::PlayControlState::default();
        let play_state = ClientPlayState {
            camera: Camera::dummy(),
            raw_chunks: Arc::new(FastHashMap::new()),
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
        #[cfg(feature = "full_std")]
        let thread_pool = ThreadPool::new(
            std::thread::available_parallelism()
                .map(|num_threads_non_zero| num_threads_non_zero.get())
                .unwrap_or(1),
        );
        #[cfg(all(feature = "full_std", feature = "tracy"))]
        for thread_i in 0..thread_pool.max_count() {
            thread_pool.execute(move || {
                let tracy_client = tracing_tracy::client::Client::running().unwrap();
                let thread_name = format!("Thread Pool Worker {thread_i}");
                std::thread::sleep(std::time::Duration::from_millis(500));
                tracy_client.set_thread_name(&thread_name);
            });
        }
        Self {
            selected_graphics_backend: selected_graphics_backend.unwrap_or_default(),
            window: None,
            graphics_backend: None,
            input_state,
            play_state,
            server_connection,
            clientbound_tx,
            clientbound_rx,
            debug_state: graphics::DebugState::default(),
            debug_output: graphics::DebugOutput {
                subchunks_culled: 0,
                subchunk_traversal_graph: Vec::new(),
            },
            egui_ctx,
            pending_egui_events: Vec::new(),
            last_mouse_pos: egui::Pos2::new(0.0, 0.0),
            previous_frame_times: VecDeque::new(),
            last_frame_time: Instant::now(),
            current_time_s: 0.0,
            last_tick_time_s: 0.0,
            next_tick_time_s: 1.0 / 20.0,
            #[cfg(feature = "full_std")]
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
        // Load resources, and setup a graphics backend.
        let resource_data =
            platform::load_resource_data().expect("Error while loading resource data");
        cfg_if::cfg_if! {
            if #[cfg(any(feature = "platform_winit", feature = "platform_linux_drm"))] {
                let graphics_backend: Box<dyn GraphicsBackend> =
                    match self.selected_graphics_backend {
                        #[cfg(feature = "graphics_backend_opengl")]
                        SelectedGraphicsBackend::OpenGL => {
                            graphics::backend_opengl::GraphicsState::new(
                                window.clone(),
                                resource_data,
                            )
                            .unwrap()
                        }
                        #[cfg(feature = "graphics_backend_software")]
                        SelectedGraphicsBackend::Software => {
                            graphics::backend_software::GraphicsState::new(
                                window.clone(),
                                resource_data,
                            )
                            .unwrap()
                        }
                        #[cfg(feature = "graphics_backend_vulkan")]
                        SelectedGraphicsBackend::Vulkan => {
                            graphics::backend_vulkan::GraphicsState::new(
                                window.clone(),
                                resource_data,
                            )
                            .unwrap()
                        }
                        #[cfg(feature = "graphics_backend_wgpu")]
                        SelectedGraphicsBackend::Wgpu => {
                            graphics::backend_wgpu::GraphicsState::new(
                                window.clone(),
                                resource_data,
                            )
                            .unwrap()
                        }
                    };
            } else {
                let graphics_backend =
                    platform::create_graphics_backend(window.clone()).unwrap();
            }
        }
        let graphics_size = graphics_backend.get_size();
        self.play_state
            .camera
            .proj_matrix
            .set_aspect((graphics_size.width as f32) / (graphics_size.height as f32));
        self.graphics_backend = Some(graphics_backend);
        self.window = Some(window);
        event_loop.set_control_flow(ControlFlow::Poll);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let window = self.window.as_ref().unwrap();
        let graphics_backend = self.graphics_backend.as_mut().unwrap();
        if id != window.id() {
            return;
        }
        let scale_factor = window.scale_factor();
        match event {
            WindowEvent::CloseRequested | WindowEvent::Destroyed => event_loop.exit(),
            WindowEvent::Resized(physical_size) => {
                graphics_backend.resize(physical_size);
                let graphics_size = graphics_backend.get_size();
                self.play_state
                    .camera
                    .proj_matrix
                    .set_aspect((graphics_size.width as f32) / (graphics_size.height as f32));
            }
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
        let graphics_backend = self.graphics_backend.as_mut().unwrap();
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
            graphics_backend.as_mut(),
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
            let size = graphics_backend.get_size();
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
        let maybe_new_debug_output = graphics_backend
            .render(
                &self.play_state.camera,
                &self.egui_ctx,
                egui_output,
                &self.debug_state,
                &debug_points,
                &debug_lines,
                &debug_triangles,
            )
            .context("Error while rendering")
            .unwrap();
        if let Some(new_debug_output) = maybe_new_debug_output {
            self.debug_output = new_debug_output;
        }
        // Gameplay events and updates
        game::process_game_events(
            #[cfg(feature = "full_std")]
            &self.thread_pool,
            &mut self.play_state,
            graphics_backend.as_mut(),
            &mut self.debug_state,
            &mut self.input_state,
            &self.server_connection,
            &self.clientbound_tx,
            &self.clientbound_rx,
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
        match event {
            DeviceEvent::MouseMotion { delta } if self.input_state.mouse_locked => {
                const MOUSE_SENSITIVITY: f32 = 0.1;
                let camera = &mut self.play_state.camera;
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
