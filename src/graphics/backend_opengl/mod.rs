#![allow(clippy::missing_safety_doc)]
#![allow(clippy::std_instead_of_alloc)]
#[cfg(not(feature = "full_std"))]
compile_error!("The OpenGL backend requires full use of `std`");

pub mod chunk;
pub mod egui_renderer;
pub mod gl;

use crate::graphics::chunk::{HasSubchunkData, SubchunkData};
use crate::graphics::debug::{Line as DebugLine, Point as DebugPoint, Triangle as DebugTriangle};
use crate::graphics::{Camera, DebugOutput, DebugState, GraphicsBackend, GraphicsOptions};
use crate::platform::libs::winit;
use crate::portable_prelude::*;
use crate::{MIN_HEIGHT_I32, SUBCHUNK_AXIS_LEN_I32};
use anyhow::Context;
use nalgebra::{Matrix4, Vector3};
use portable_std::{Arc, FastHashMap, FastHashSet};
use resources::block::ResourceData;
use std::sync::mpsc::{Receiver, Sender};
use threadpool::ThreadPool;
use winit::window::Window;

use gl::array::{ColorPointerType, TextureCoordPointerType, VertexPointerType};
use gl::buffer::BufferType;
use gl::client_state::ClientArrayType;

cfg_if::cfg_if! {
    if #[cfg(feature = "platform_winit")] {
        use glutin::prelude::*;
        use core::num::NonZeroU32;
        use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};

        pub struct GlutinResources {
            pub display: glutin::display::Display,
            pub context: glutin::context::PossiblyCurrentContext,
            pub surface: glutin::surface::Surface<glutin::surface::WindowSurface>,
        }

        // SAFETY: `GraphicsState` is only ever used from a main "render thread", and so these resources
        //          are only ever accessed from the same thread.
        unsafe impl Send for GlutinResources {}

        // SAFETY: Ditto.
        unsafe impl Sync for GlutinResources {}
    }
}

#[derive(Clone)]
pub struct GraphicsResources {
    pub block_registry: Arc<resources::block::Registry>,
    pub model_registry: Arc<resources::block::model::ModelRegistry>,
    #[cfg(feature = "platform_winit")]
    glutin_resources: Arc<GlutinResources>,
    pub atlas_texture: Arc<gl::texture::batch_collected::Texture>,
    pub atlas_texture_dims: (u32, u32),
    pub window: Arc<Window>,
}

pub struct SubchunkDataStorage {
    // TODO: Currently the Y coordinate is a chunk section index, rather than the subchunk Y
    //       coordinate. Consider changing to actually be the Y coordinate.
    pub subchunks: FastHashMap<[i32; 3], chunk::Subchunk>,
    pub loaded_chunks: FastHashSet<[i32; 2]>,
}

pub struct GraphicsState {
    pub resources: GraphicsResources,
    pub graphics_options: GraphicsOptions,
    pub egui_renderer: egui_renderer::Renderer,
    subchunk_data_storage: SubchunkDataStorage,
    pub pending_subchunk_tx: Sender<Option<chunk::RawSubchunk>>,
    pub pending_subchunk_rx: Receiver<Option<chunk::RawSubchunk>>,
    pub current_dispatch_id_counter: u64,
    pub num_pending_subchunks: usize,
    pub size: winit::dpi::PhysicalSize<u32>,
}

impl GraphicsBackend for GraphicsState {
    #[tracing::instrument(skip_all)]
    fn new(window: Arc<Window>, resource_data: ResourceData) -> anyhow::Result<Box<Self>> {
        let graphics_options = GraphicsOptions::default();
        let size = window.inner_size();
        cfg_if::cfg_if! {
            if #[cfg(feature = "platform_winit")] {
                // Initialise various components of `glutin` to get an OpenGL environment.
                // Display
                let display_handle = window
                    .display_handle()
                    .context("Error while getting display handle from window")?;
                let window_handle = window
                    .window_handle()
                    .context("Error while getting window handle")?;
                let glutin_display = unsafe {
                    glutin::display::Display::new(
                        display_handle.as_raw(),
                        #[cfg(target_os = "windows")]
                        glutin::display::DisplayApiPreference::Wgl(Some(window_handle.as_raw())),
                        #[cfg(not(target_os = "windows"))]
                        glutin::display::DisplayApiPreference::Egl,
                    )
                    .context("Error while creating glutin display")?
                };
                // Config
                let glutin_config_template = glutin::config::ConfigTemplateBuilder::new()
                    .with_alpha_size(0)
                    .with_depth_size(24)
                    .with_api(glutin::config::Api::OPENGL)
                    .prefer_hardware_accelerated(Some(true))
                    .compatible_with_native_window(window_handle.as_raw())
                    .build();
                let glutin_config = unsafe {
                    glutin_display
                        .find_configs(glutin_config_template)
                        .context("Failed to get glutin configs")?
                        .next()
                        .context("Failed to find any glutin configs")?
                };
                // Surface
                type GlutinWindowSurfaceAttributesBuilder =
                    glutin::surface::SurfaceAttributesBuilder<glutin::surface::WindowSurface>;
                let glutin_surface_attributes = GlutinWindowSurfaceAttributesBuilder::new()
                    .with_srgb(Some(true))
                    .build(
                        window_handle.as_raw(),
                        NonZeroU32::new(size.width).unwrap_or(NonZeroU32::MIN),
                        NonZeroU32::new(size.height).unwrap_or(NonZeroU32::MIN),
                    );
                let glutin_surface = unsafe {
                    glutin_display.create_window_surface(
                        &glutin_config,
                        &glutin_surface_attributes,
                    )
                    .context("Error while creating glutin surface")?
                };
                // Context
                let glutin_context_attributes = glutin::context::ContextAttributesBuilder::new()
                    .with_profile(glutin::context::GlProfile::Compatibility)
                    .with_context_api(glutin::context::ContextApi::OpenGl(Some(glutin::context::Version::new(1, 3))))
                    .build(Some(window_handle.as_raw()));
                let glutin_context = unsafe {
                    glutin_display
                        .create_context(&glutin_config, &glutin_context_attributes)
                        .context("Error while creating glutin context")?
                        .make_current(&glutin_surface)
                        .context("Error while making glutin context current")?
                };
                // Load OpenGL function pointers.
                unsafe {
                    gl::load_with(|name| {
                        let c_string_name = std::ffi::CString::new(name).unwrap();
                        glutin_display.get_proc_address(&c_string_name) as *const ()
                    })
                }
            } else if #[cfg(feature = "platform_linux_drm")] {
                // On the Linux DRM backend, `Window::new` is responsible for loading all of the
                // OpenGL function pointers, so everything's loaded by the time we get here.
                // It's also responsible for making the OpenGL context current before we get here.
                // This means we don't actually have to do anything at this point.
            } else {
                compile_error!(concat!(
                    "Support for the OpenGL graphics backend is currently unimplemented for the ",
                    "selected platform.",
                ));
            }
        }
        unsafe {
            gl::framebuffer::clear_color(0.471, 0.655, 1.0, 1.0);
            gl::viewport::set(
                0,
                0,
                size.width.try_into().unwrap(),
                size.height.try_into().unwrap(),
            );
            gl::texture::set_pixel_store_i32_raw(gl::texture::PixelStoreParam::UnpackAlignment, 1);
        }
        // Load game resources.
        let ResourceData {
            block_registry,
            model_registry,
            atlas,
        } = resource_data;
        let atlas_texture = unsafe {
            use gl::texture::{
                TexFilterMode, TexTarget, TexWrapMode, Texture2dFormat, Texture2dTarget,
                TextureDataType, TextureInternalFormat,
            };
            let [texture] = gl::texture::batch_collected::Texture::make_array();
            texture.bind(TexTarget::Texture2D);
            gl::texture::set_wrap_s(TexTarget::Texture2D, TexWrapMode::Repeat);
            gl::texture::set_wrap_t(TexTarget::Texture2D, TexWrapMode::Repeat);
            gl::texture::set_mag_filter(TexTarget::Texture2D, TexFilterMode::Nearest);
            gl::texture::set_min_filter(TexTarget::Texture2D, TexFilterMode::Nearest);
            gl::texture::set_image_2d(
                Texture2dTarget::Texture,
                0,
                TextureInternalFormat::Rgba,
                atlas.width.try_into().unwrap(),
                atlas.height.try_into().unwrap(),
                0,
                Texture2dFormat::Rgba,
                TextureDataType::U8,
                atlas.texture_bytes.as_ptr() as *const (),
            );
            gl::texture::bind(TexTarget::Texture2D, None);
            texture
        };
        let egui_renderer = egui_renderer::Renderer::new();
        let (pending_subchunk_tx, pending_subchunk_rx) = std::sync::mpsc::channel();
        Ok(Box::new(Self {
            resources: GraphicsResources {
                window,
                #[cfg(feature = "platform_winit")]
                glutin_resources: Arc::new(GlutinResources {
                    display: glutin_display,
                    context: glutin_context,
                    surface: glutin_surface,
                }),
                block_registry: Arc::new(block_registry),
                model_registry: Arc::new(model_registry),
                atlas_texture: Arc::new(atlas_texture),
                atlas_texture_dims: (atlas.width, atlas.height),
            },
            subchunk_data_storage: SubchunkDataStorage {
                subchunks: FastHashMap::new(),
                loaded_chunks: FastHashSet::new(),
            },
            pending_subchunk_tx,
            pending_subchunk_rx,
            current_dispatch_id_counter: 0,
            num_pending_subchunks: 0,
            graphics_options,
            egui_renderer,
            size,
        }))
    }

    fn get_block_registry(&self) -> &resources::block::Registry {
        &self.resources.block_registry
    }

    fn get_subchunks_data(&self) -> FastHashMap<[i32; 3], SubchunkData> {
        self.subchunk_data_storage
            .subchunks
            .iter()
            .map(|(&subchunk_coords, subchunk)| (subchunk_coords, subchunk.get_data()))
            .collect()
    }

    fn get_size(&self) -> winit::dpi::PhysicalSize<u32> {
        self.size
    }

    #[tracing::instrument(skip(self))]
    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        unsafe {
            #[cfg(feature = "platform_winit")]
            {
                let glutin_context = &self.resources.glutin_resources.context;
                let glutin_surface = &self.resources.glutin_resources.surface;
                glutin_surface.resize(
                    glutin_context,
                    NonZeroU32::new(new_size.width).unwrap_or(NonZeroU32::MIN),
                    NonZeroU32::new(new_size.height).unwrap_or(NonZeroU32::MIN),
                );
                glutin_context
                    .make_current(glutin_surface)
                    .expect("Failed to make glutin context current for resizing");
            }
            self.size = new_size;
            gl::viewport::set(
                0,
                0,
                new_size.width.try_into().unwrap(),
                new_size.height.try_into().unwrap(),
            );
        }
    }

    fn get_graphics_options(&self) -> GraphicsOptions {
        self.graphics_options
    }

    #[tracing::instrument(skip(self))]
    fn apply_new_graphics_options(&mut self, new_options: GraphicsOptions) {
        let old_options = core::mem::replace(&mut self.graphics_options, new_options);
        cfg_if::cfg_if! {
            if #[cfg(feature = "platform_winit")] {
                let glutin_context = &self.resources.glutin_resources.context;
                let glutin_surface = &self.resources.glutin_resources.surface;
                if new_options.vsync != old_options.vsync {
                    glutin_surface
                        .set_swap_interval(
                            glutin_context,
                            if new_options.vsync {
                                glutin::surface::SwapInterval::Wait(NonZeroU32::new(1).unwrap())
                            } else {
                                glutin::surface::SwapInterval::DontWait
                            }
                        )
                        .expect("Failed to change glutin surface swap interval for VSync change");
                }
            } else if #[cfg(feature = "platform_linux_drm")] {
                _ = old_options;
            }
        }
    }

    #[tracing::instrument(skip_all)]
    fn dispatch_subchunk_updates(
        &mut self,
        thread_pool: &ThreadPool,
        raw_chunks: Arc<FastHashMap<[i32; 2], Arc<crate::RawChunk>>>,
        subchunks: FastHashSet<[i32; 3]>,
    ) {
        // Mark that we're dispatching a number of subchunk processes.
        self.num_pending_subchunks += subchunks.len();
        // Grab a new dispatch ID.
        let dispatch_id = self.current_dispatch_id_counter;
        self.current_dispatch_id_counter += 1;
        // Dispatch subchunk tasks.
        for subchunk_coords in subchunks {
            // Mark chunk as definitely loaded (does nothing if the chunk is only being updated).
            let [chunk_x, _, chunk_z] = subchunk_coords;
            self.subchunk_data_storage
                .loaded_chunks
                .insert([chunk_x, chunk_z]);
            // Dispatch subchunk task.
            let block_registry = self.resources.block_registry.clone();
            let model_registry = self.resources.model_registry.clone();
            let raw_chunks = raw_chunks.clone();
            let pending_subchunk_tx = self.pending_subchunk_tx.clone();
            thread_pool.execute(move || {
                chunk::process_subchunk(
                    &block_registry,
                    &model_registry,
                    &raw_chunks,
                    &pending_subchunk_tx,
                    subchunk_coords,
                    dispatch_id,
                );
            });
        }
    }

    #[tracing::instrument(skip_all)]
    fn remove_chunk(&mut self, chunk_coords: [i32; 2]) {
        let [chunk_x, chunk_z] = chunk_coords;
        // Remove old subchunks.
        if self
            .subchunk_data_storage
            .loaded_chunks
            .contains(&chunk_coords)
        {
            let span = tracing::trace_span!("remove_subchunks", ?chunk_coords);
            let _enter = span.enter();
            for subchunk_y in 0..24 {
                let subchunk_coords = [chunk_x, subchunk_y, chunk_z];
                let span = tracing::trace_span!("remove_subchunk", ?subchunk_coords);
                let _enter = span.enter();
                self.subchunk_data_storage
                    .subchunks
                    .remove(&subchunk_coords);
            }
        }
        // Mark chunk as no longer loaded, so any pending subchunk tasks for this chunk finishing
        // after removal won't cause ghost chunks to appear.
        self.subchunk_data_storage
            .loaded_chunks
            .remove(&chunk_coords);
    }

    #[tracing::instrument(skip_all)]
    fn render(
        &mut self,
        camera: &Camera,
        egui_ctx: &egui::Context,
        egui_full_output: egui::output::FullOutput,
        debug_state: &DebugState,
        debug_points: &[DebugPoint],
        debug_lines: &[DebugLine],
        debug_triangles: &[DebugTriangle],
    ) -> anyhow::Result<Option<DebugOutput>> {
        let pixels_per_point = egui_full_output.pixels_per_point;
        let egui_primitives = egui_ctx.tessellate(egui_full_output.shapes, pixels_per_point);
        let debug_output;
        unsafe {
            use gl::framebuffer::ClearBufferBits;
            use gl::matrix::MatrixMode;
            use gl::texture::{TexEnvMode, TexEnvTarget, TexTarget};
            cfg_if::cfg_if! {
                if #[cfg(feature = "platform_winit")] {
                    let glutin_context = &self.resources.glutin_resources.context;
                    let glutin_surface = &self.resources.glutin_resources.surface;
                    glutin_context
                        .make_current(glutin_surface)
                        .context("Error while making glutin context current")?;
                } else if #[cfg(feature = "platform_linux_drm")] {
                    let window_context = self.resources.window.get_context_blocking();
                }
            }
            // Delete all batch collected buffers.
            gl::buffer::batch_collected::drain_pool();
            // Delete all batch collected textures.
            gl::texture::batch_collected::drain_pool();
            // Upload pending subchunks.
            if self.num_pending_subchunks > 0 {
                let span = tracing::trace_span!("upload_pending_subchunks");
                let _enter = span.enter();
                let mut subchunks_processed_this_frame: usize = 0;
                for raw_subchunk in self
                    .pending_subchunk_rx
                    .try_iter()
                    .take(self.num_pending_subchunks)
                {
                    self.num_pending_subchunks -= 1;
                    let Some(raw_subchunk) = raw_subchunk else {
                        continue;
                    };
                    // Check that the raw subchunk is newer than the subchunk it's replacing, or that
                    // it's not replacing an old subchunk. If it's not, then skip it.
                    if self
                        .subchunk_data_storage
                        .subchunks
                        .get(&raw_subchunk.subchunk_coords)
                        .map(|subchunk| subchunk.dispatch_id > raw_subchunk.dispatch_id)
                        .unwrap_or(false)
                    {
                        continue;
                    }
                    chunk::finalise_subchunk(&mut self.subchunk_data_storage, raw_subchunk);
                    subchunks_processed_this_frame += 1;
                    if subchunks_processed_this_frame >= 16 {
                        break;
                    }
                }
            }
            // Clear framebuffer to sky colour, depth buffer to infinite distance..
            gl::framebuffer::clear(ClearBufferBits::COLOR | ClearBufferBits::DEPTH);
            // Render subchunks
            {
                let span = tracing::trace_span!("render_subchunks");
                let _enter = span.enter();
                {
                    let span = tracing::trace_span!("set_gl_state");
                    let _enter = span.enter();
                    gl::disable(gl::EnableComponent::ScissorTest);
                    gl::disable(gl::EnableComponent::Blending);
                    gl::enable(gl::EnableComponent::DepthTest);
                    gl::enable(gl::EnableComponent::AlphaTesting);
                    gl::enable(gl::EnableComponent::FaceCulling);
                    gl::fragment::set_alpha_test_function(
                        gl::fragment::AlphaTestFunc::Greater,
                        0.0,
                    );
                    gl::enable(gl::EnableComponent::Texture2D);
                    gl::texture::set_env_mode(TexEnvTarget::TextureEnv, TexEnvMode::Modulate);
                }
                // Load and enable texture atlas.
                {
                    let span = tracing::trace_span!("bind_texture_atlas");
                    let _enter = span.enter();
                    self.resources
                        .atlas_texture
                        .bind(gl::texture::TexTarget::Texture2D);
                }
                gl::matrix::switch_mode(gl::matrix::MatrixMode::Texture);
                let texture_matrix = Matrix4::identity().append_nonuniform_scaling(&Vector3::new(
                    1.0 / self.resources.atlas_texture_dims.0 as f32,
                    1.0 / self.resources.atlas_texture_dims.1 as f32,
                    1.0,
                ));
                gl::matrix::load_f32_matrix(&texture_matrix.into());
                // Load camera projection matrix.
                gl::matrix::switch_mode(MatrixMode::Projection);
                gl::matrix::load_f32_matrix(&camera.generate_view_matrix_slice());
                let camera_subchunk_coords = {
                    let camera_pos = camera.pos;
                    let camera_x = (camera_pos.x.floor() as i32).div_euclid(SUBCHUNK_AXIS_LEN_I32);
                    let camera_y = (camera_pos.y.floor() as i32 - MIN_HEIGHT_I32)
                        .div_euclid(SUBCHUNK_AXIS_LEN_I32);
                    let camera_z = (camera_pos.z.floor() as i32).div_euclid(SUBCHUNK_AXIS_LEN_I32);
                    [camera_x, camera_y, camera_z]
                };
                {
                    let span = tracing::trace_span!("subchunks_set_client_state");
                    let _enter = span.enter();
                    gl::client_state::enable(ClientArrayType::VertexArray);
                    gl::client_state::enable(ClientArrayType::ColorArray);
                    gl::client_state::enable(ClientArrayType::TextureCoordArray);
                }
                debug_output = super::for_each_visible_subchunk(
                    camera,
                    &self.subchunk_data_storage.subchunks,
                    &self.subchunk_data_storage.loaded_chunks,
                    debug_state,
                    |subchunk_coords, subchunk| {
                        match &subchunk.buffer {
                            None => return,
                            Some(buffer) => buffer.bind(BufferType::ArrayBuffer),
                        }
                        gl::matrix::switch_mode(gl::matrix::MatrixMode::ModelView);
                        gl::matrix::load_f32_matrix(&chunk::generate_subchunk_matrix(
                            subchunk.start_coords,
                        ));
                        gl::array::vertex_pointer(
                            3,
                            VertexPointerType::I16,
                            size_of::<chunk::BlockVertex>().try_into().unwrap(),
                            core::mem::offset_of!(chunk::BlockVertex, subchunk_fixed_point_pos),
                        );
                        gl::array::color_pointer(
                            4,
                            ColorPointerType::U8,
                            size_of::<chunk::BlockVertex>().try_into().unwrap(),
                            core::mem::offset_of!(chunk::BlockVertex, colour_rgba),
                        );
                        gl::array::texture_coord_pointer(
                            2,
                            TextureCoordPointerType::I16,
                            size_of::<chunk::BlockVertex>().try_into().unwrap(),
                            core::mem::offset_of!(chunk::BlockVertex, uvs),
                        );
                        for i in 0..7 {
                            let skip_face_dir = match i {
                                0 => subchunk_coords[1] > camera_subchunk_coords[1],
                                1 => subchunk_coords[1] < camera_subchunk_coords[1],
                                2 => subchunk_coords[2] < camera_subchunk_coords[2],
                                3 => subchunk_coords[2] > camera_subchunk_coords[2],
                                4 => subchunk_coords[0] > camera_subchunk_coords[0],
                                5 => subchunk_coords[0] < camera_subchunk_coords[0],
                                6 => false,
                                7.. => unreachable!(),
                            };
                            if skip_face_dir {
                                continue;
                            }
                            let start_vertex = subchunk.group_start_vertices[i];
                            let num_vertices = subchunk.group_vertex_counts[i];
                            if start_vertex != u32::MAX {
                                gl::array::draw(
                                    gl::ShapeMode::Triangles,
                                    start_vertex.try_into().unwrap(),
                                    num_vertices.try_into().unwrap(),
                                );
                            }
                        }
                    },
                );
                gl::texture::bind(TexTarget::Texture2D, None);
            }
            // Render debug graphics.
            // TODO: Get the `ignore_depth` flags working.
            {
                gl::buffer::bind(BufferType::ArrayBuffer, None);
                gl::disable(gl::EnableComponent::Texture2D);
                gl::client_state::disable(ClientArrayType::TextureCoordArray);
                gl::matrix::switch_mode(gl::matrix::MatrixMode::ModelView);
                gl::matrix::load_identity();
                // Render debug triangles.
                if !debug_triangles.is_empty() {
                    let mut points = Vec::new();
                    let mut colours = Vec::new();
                    for tri in debug_triangles {
                        points.extend([tri.p1, tri.p2, tri.p3]);
                        colours.extend([tri.color; 3]);
                    }
                    gl::array::vertex_pointer(3, VertexPointerType::F32, 0, points.as_ptr().addr());
                    gl::array::color_pointer(4, ColorPointerType::U8, 0, colours.as_ptr().addr());
                    gl::array::draw(
                        gl::ShapeMode::Triangles,
                        0,
                        points.len().try_into().unwrap(),
                    );
                }
                // Render debug lines.
                if !debug_lines.is_empty() {
                    #[repr(C)]
                    #[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
                    struct DebugLineVertex {
                        pub pos: [f32; 3],
                        pub colour: [u8; 4],
                    }
                    let converted_lines: Vec<DebugLineVertex> = debug_lines
                        .iter()
                        .flat_map(|line| {
                            [
                                DebugLineVertex {
                                    pos: line.p1,
                                    colour: line.colour,
                                },
                                DebugLineVertex {
                                    pos: line.p2,
                                    colour: line.colour,
                                },
                            ]
                        })
                        .collect();
                    gl::array::vertex_pointer(
                        3,
                        VertexPointerType::F32,
                        size_of::<DebugLineVertex>().try_into().unwrap(),
                        (&raw const converted_lines[0].pos).addr(),
                    );
                    gl::array::color_pointer(
                        4,
                        ColorPointerType::U8,
                        size_of::<DebugLineVertex>().try_into().unwrap(),
                        (&raw const converted_lines[0].colour).addr(),
                    );
                    gl::array::draw(
                        gl::ShapeMode::Lines,
                        0,
                        converted_lines.len().try_into().unwrap(),
                    );
                }
                // Render debug points.
                // TODO: Get point sizes working.
                if !debug_points.is_empty() {
                    gl::array::vertex_pointer(
                        3,
                        VertexPointerType::F32,
                        size_of::<DebugPoint>().try_into().unwrap(),
                        (&raw const debug_points[0].pos).addr(),
                    );
                    gl::array::color_pointer(
                        4,
                        ColorPointerType::U8,
                        size_of::<DebugPoint>().try_into().unwrap(),
                        (&raw const debug_points[0].color).addr(),
                    );
                    gl::array::draw(
                        gl::ShapeMode::Points,
                        0,
                        debug_points.len().try_into().unwrap(),
                    );
                }
            }
            // Render egui UI.
            self.egui_renderer.render(
                &self.size,
                egui_full_output.textures_delta.set,
                egui_primitives,
                pixels_per_point,
            );
            cfg_if::cfg_if! {
                if #[cfg(feature = "platform_winit")] {
                    glutin_surface
                        .swap_buffers(glutin_context)
                        .context("Error while swapping glutin surface buffers")?;
                } else if #[cfg(feature = "platform_linux_drm")] {
                    window_context.flip_page(self.graphics_options.vsync);
                }
            }
        }
        Ok(Some(debug_output))
    }
}
