#![allow(clippy::missing_safety_doc)]
#![allow(clippy::std_instead_of_alloc)]
#[cfg(not(feature = "full_std"))]
compile_error!("The OpenGL backend requires full use of `std`");

pub mod chunk;
pub mod egui_renderer;
pub mod gl;

use std::sync::mpsc::{Receiver, Sender};

use anyhow::Context;
use nalgebra::{Isometry3, Vector3};
use portable_std::{Arc, FastHashMap, FastHashSet};
use resources::GameResourceData;
use threadpool::ThreadPool;
use winit::event_loop::OwnedDisplayHandle;
use winit::window::Window;

use crate::graphics::chunk::{HasSubchunkData, SubchunkData};
use crate::graphics::debug::{Line as DebugLine, Point as DebugPoint, Triangle as DebugTriangle};
use crate::graphics::environment::sky::{STAR_QUADS, SkyExtrapolationState, get_star_brightness};
use crate::graphics::lightmap::{generate_dummy_lightmap_texture, generate_lightmap_texture};
use crate::graphics::{DebugOutput, DebugState, GraphicsBackend, GraphicsOptions};
use crate::platform::libs::winit;
use crate::portable_prelude::*;
use crate::{ClientPlayState, MIN_HEIGHT_I32, SUBCHUNK_AXIS_LEN_I32};
use gl::array::{AttributeNormalisation, AttributeType, ColorType, TextureCoordType, VertexType};
use gl::buffer::BufferType;
use gl::client_state::ClientArrayType;
use gl::texture::{
    ActiveTexture, TexEnvMode, TexEnvTarget, TexFilterMode, TexTarget, TexWrapMode,
    Texture2dFormat, Texture2dTarget, TextureDataType, TextureInternalFormat,
};

cfg_select! {
    feature = "platform_winit" => {
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

/// Custom camera near plane distance override returned by the graphics backend.
/// This is needed to improve precision at longer ranges, as unfortunately old OpenGL doesn't give
/// us a standard way to make reversed depth buffers useful.
const CAMERA_ZNEAR_OVERRIDE: f32 = 0.1;

mod chunk_vertex_program {
    use super::*;

    pub static CODE: &str = include_str!("backend_opengl/chunk_vertex.arb");

    // Environment variables
    /// `[1.0 / Atlas Width, 1.0 / Atlas Height, 1.0, 1.0]`
    pub const ENV_INV_ATLAS_TEXTURE_DIMS: gl::GLuint = 0;

    // Attribute indices
    /// `[Sky Light Level (0..=15), Block Light Level (0..=15)]`
    pub const ATTRIB_LIGHT_LEVELS: gl::GLuint = 1;
}

#[derive(Clone)]
pub struct GraphicsResources {
    pub block_registry: Arc<resources::block::Registry>,
    pub model_registry: Arc<resources::block::model::ModelRegistry>,
    #[cfg(feature = "platform_winit")]
    glutin_resources: Arc<GlutinResources>,
    pub atlas_texture: Arc<GlTexture>,
    pub moon_phases_texture: Arc<GlTexture>,
    pub sun_texture: Arc<GlTexture>,
    pub lightmap_texture_handle: Arc<gl::texture::batch_collected::TextureHandle>,
    pub chunk_vertex_program: gl::program_arb::ProgramHandle,
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
    pub sky_extrapolation_state: SkyExtrapolationState,
    pub debug_lightmap_image: egui::load::SizedTexture,
    /// "Brightness" setting, controls gamma falloff for block lightmap.
    pub debug_lightmap_time_of_day: f64,
    pub size: winit::dpi::PhysicalSize<u32>,
}

impl GraphicsBackend for GraphicsState {
    #[tracing::instrument(skip_all)]
    fn new(
        window: Arc<Window>,
        display: OwnedDisplayHandle,
        game_data: GameResourceData,
    ) -> anyhow::Result<Box<Self>> {
        let graphics_options = GraphicsOptions::default();
        let size = window.inner_size();
        cfg_select! {
            feature = "platform_winit" => {
                // Initialise various components of `glutin` to get an OpenGL environment.
                let display_handle = display
                    .display_handle()
                    .context("Error while getting display handle")?;
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
                    .with_srgb(None)
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
            }
            feature = "platform_linux_drm" => {
                // On the Linux DRM backend, `Window::new` is responsible for loading all of the
                // OpenGL function pointers, so everything's loaded by the time we get here.
                // It's also responsible for making the OpenGL context current before we get here.
                // This means we don't actually have to do anything at this point.
            }
            _ => {
                compile_error!(concat!(
                    "Support for the OpenGL graphics backend is currently unimplemented for the ",
                    "selected platform.",
                ));
            }
        }
        // Log some debug information about the OpenGL implementation.
        unsafe {
            log::debug!(
                "OpenGL Version: {:?}",
                gl::get_string_lossy(gl::StringName::Version)
            );
            log::debug!(
                "OpenGL Extensions: {:?}",
                gl::get_string_lossy(gl::StringName::Extensions)
            );
        }
        // Set initial OpenGL state.
        unsafe {
            // We're using reversed depth, so clearing to zero is "infinite distance".
            gl::framebuffer::clear_depth(0.0);
            gl::fragment::set_depth_test_function(gl::fragment::DepthTestFunction::Greater);
            gl::viewport::set(
                0,
                0,
                size.width.try_into().unwrap(),
                size.height.try_into().unwrap(),
            );
            gl::texture::set_pixel_store_i32_raw(gl::texture::PixelStoreParam::UnpackAlignment, 1);
        }
        // Generate lightmap.
        let lightmap_texture_handle = unsafe {
            let [handle] = gl::texture::batch_collected::TextureHandle::make_array();
            handle.bind(TexTarget::Texture2D);
            gl::texture::set_wrap_s(TexTarget::Texture2D, TexWrapMode::Clamp);
            gl::texture::set_wrap_t(TexTarget::Texture2D, TexWrapMode::Clamp);
            gl::texture::set_mag_filter(TexTarget::Texture2D, TexFilterMode::Nearest);
            gl::texture::set_min_filter(TexTarget::Texture2D, TexFilterMode::Nearest);
            // Texture data will be set during each frame render.
            gl::texture::bind(TexTarget::Texture2D, None);
            handle
        };
        // Load game resources.
        let resources::GameResourceData {
            block_data,
            environment_data,
        } = game_data;
        let resources::block::Data {
            block_registry,
            model_registry,
            atlas,
        } = block_data;
        let resources::environment::Data {
            moon_phases_texture,
            sun_texture,
        } = environment_data;
        let atlas_texture = unsafe { GlTexture::create_from_resource_atlas(&atlas) };
        let (moon_phases_texture, sun_texture) = unsafe {
            let moon_phases_texture = GlTexture::create_from_resource_texture(
                &moon_phases_texture,
                GlTextureCreateOptions::default(),
            );
            let sun_texture = GlTexture::create_from_resource_texture(
                &sun_texture,
                GlTextureCreateOptions::default(),
            );
            (moon_phases_texture, sun_texture)
        };
        let chunk_vertex_program = unsafe {
            use gl::program_arb::ProgramType;
            let [program] = gl::program_arb::gen_programs();
            gl::program_arb::bind(ProgramType::VertexProgram, Some(program));
            gl::program_arb::set_current_program_string(
                ProgramType::VertexProgram,
                chunk_vertex_program::CODE,
            );
            program
        };
        let mut egui_renderer = egui_renderer::Renderer::new();
        let debug_lightmap_image = egui_renderer.register_user_texture(unsafe {
            egui_renderer::ImageData::new(
                image::RgbaImage::from_vec(
                    16,
                    16,
                    generate_dummy_lightmap_texture()
                        .as_flattened()
                        .as_flattened()
                        .into(),
                )
                .unwrap(),
                egui::TextureOptions::NEAREST,
            )
        });
        let (pending_subchunk_tx, pending_subchunk_rx) = std::sync::mpsc::channel();
        Ok(Box::new(Self {
            resources: GraphicsResources {
                #[cfg(feature = "platform_winit")]
                glutin_resources: Arc::new(GlutinResources {
                    display: glutin_display,
                    context: glutin_context,
                    surface: glutin_surface,
                }),
                block_registry: Arc::new(block_registry),
                model_registry: Arc::new(model_registry),
                atlas_texture: Arc::new(atlas_texture),
                moon_phases_texture: Arc::new(moon_phases_texture),
                sun_texture: Arc::new(sun_texture),
                chunk_vertex_program,
                lightmap_texture_handle: Arc::new(lightmap_texture_handle),
                window,
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
            sky_extrapolation_state: SkyExtrapolationState::new(),
            debug_lightmap_image: egui::load::SizedTexture {
                id: debug_lightmap_image,
                size: egui::vec2(16.0, 16.0),
            },
            debug_lightmap_time_of_day: 0.0,
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
        cfg_select! {
            feature = "platform_winit" => {
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
            }
            feature = "platform_linux_drm" => {
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
        play_state: &ClientPlayState,
        current_time_s: f64,
        egui_ctx: &egui::Context,
        egui_full_output: egui::output::FullOutput,
        debug_state: &DebugState,
        debug_points: &[DebugPoint],
        debug_lines: &[DebugLine],
        debug_triangles: &[DebugTriangle],
    ) -> anyhow::Result<Option<DebugOutput>> {
        let camera = &play_state.camera;
        let pixels_per_point = egui_full_output.pixels_per_point;
        let egui_primitives = egui_ctx.tessellate(egui_full_output.shapes, pixels_per_point);
        let debug_output;
        unsafe {
            use gl::framebuffer::ClearBufferBits;
            use gl::matrix::MatrixMode;
            use gl::program_arb::ProgramType;
            cfg_select! {
                feature = "platform_winit" => {
                    let glutin_context = &self.resources.glutin_resources.context;
                    let glutin_surface = &self.resources.glutin_resources.surface;
                    glutin_context
                        .make_current(glutin_surface)
                        .context("Error while making glutin context current")?;
                }
                feature = "platform_linux_drm" => {
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
            // Calculate the time of day and a sky colour for the current frame.
            let time_of_day = self
                .sky_extrapolation_state
                .update(play_state, current_time_s);
            let [sky_r, sky_g, sky_b] = crate::graphics::environment::sky::get_rgb(time_of_day);
            // Clear framebuffer to sky colour, depth buffer to infinite distance.
            gl::framebuffer::clear_color(sky_r, sky_g, sky_b, 1.0);
            gl::framebuffer::clear(ClearBufferBits::COLOR | ClearBufferBits::DEPTH);
            // Render subchunks.
            {
                let span = tracing::trace_span!("render_subchunks");
                let _enter = span.enter();
                {
                    let span = tracing::trace_span!("subchunks_set_gl_state");
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
                }
                // Bind vertex program, set environment variables.
                {
                    gl::enable(gl::EnableComponent::VertexProgramARB);
                    let span = tracing::trace_span!("subchunks_set_vertex_program");
                    let _enter = span.enter();
                    gl::program_arb::bind(
                        ProgramType::VertexProgram,
                        Some(self.resources.chunk_vertex_program),
                    );
                    gl::program_arb::set_program_env_parameter_f32(
                        ProgramType::VertexProgram,
                        chunk_vertex_program::ENV_INV_ATLAS_TEXTURE_DIMS,
                        1.0 / self.resources.atlas_texture.width as f32,
                        1.0 / self.resources.atlas_texture.height as f32,
                        1.0,
                        1.0,
                    );
                }
                // Load and enable texture atlas.
                {
                    let span = tracing::trace_span!("subchunks_bind_texture_atlas");
                    let _enter = span.enter();
                    gl::enable(gl::EnableComponent::Texture2D);
                    gl::texture::set_env_mode(TexEnvTarget::TextureEnv, TexEnvMode::Modulate);
                    self.resources
                        .atlas_texture
                        .handle
                        .bind(TexTarget::Texture2D);
                }
                // Regenerate and enable lightmap texture.
                {
                    let span = tracing::trace_span!("subchunks_regen_and_bind_lightmap_texture");
                    let _enter = span.enter();
                    let lightmap_data = generate_lightmap_texture(
                        self.graphics_options.lightmap_gamma_setting,
                        time_of_day,
                    );
                    let lightmap_width = lightmap_data[0].len();
                    let lightmap_height = lightmap_data.len();
                    gl::texture::switch_active(ActiveTexture::Texture1);
                    gl::enable(gl::EnableComponent::Texture2D);
                    gl::texture::set_env_mode(TexEnvTarget::TextureEnv, TexEnvMode::Modulate);
                    self.resources
                        .lightmap_texture_handle
                        .bind(TexTarget::Texture2D);
                    gl::texture::set_image_2d(
                        Texture2dTarget::Texture,
                        0,
                        TextureInternalFormat::Rgb,
                        lightmap_width.try_into().unwrap(),
                        lightmap_height.try_into().unwrap(),
                        0,
                        Texture2dFormat::Rgba,
                        TextureDataType::U8,
                        lightmap_data.as_ptr() as *const (),
                    );
                    gl::matrix::switch_mode(MatrixMode::Texture);
                    gl::matrix::load_identity();
                    gl::texture::switch_active(ActiveTexture::Texture0);
                }
                // Load camera projection matrix.
                gl::matrix::switch_mode(MatrixMode::Projection);
                gl::matrix::load_f32_matrix(&camera.generate_reversed_depth_view_matrix_slice());
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
                    gl::array::enable_attribute_array(chunk_vertex_program::ATTRIB_LIGHT_LEVELS);
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
                            VertexType::I16,
                            size_of::<chunk::BlockVertex>().try_into().unwrap(),
                            core::mem::offset_of!(chunk::BlockVertex, subchunk_fixed_point_pos),
                        );
                        gl::array::color_pointer(
                            4,
                            ColorType::U8,
                            size_of::<chunk::BlockVertex>().try_into().unwrap(),
                            core::mem::offset_of!(
                                chunk::BlockVertex,
                                tint_colour_and_dir_light_rgba
                            ),
                        );
                        gl::array::texture_coord_pointer(
                            2,
                            TextureCoordType::I16,
                            size_of::<chunk::BlockVertex>().try_into().unwrap(),
                            core::mem::offset_of!(chunk::BlockVertex, uvs),
                        );
                        gl::array::attribute_pointer(
                            chunk_vertex_program::ATTRIB_LIGHT_LEVELS,
                            2,
                            AttributeType::U8,
                            AttributeNormalisation::Unnormalised,
                            size_of::<chunk::BlockVertex>().try_into().unwrap(),
                            core::mem::offset_of!(chunk::BlockVertex, light_levels),
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
                                    gl::ShapeMode::Quads,
                                    start_vertex.try_into().unwrap(),
                                    num_vertices.try_into().unwrap(),
                                );
                            }
                        }
                    },
                );
                gl::program_arb::bind(ProgramType::VertexProgram, None);
                gl::buffer::bind(BufferType::ArrayBuffer, None);
                gl::disable(gl::EnableComponent::VertexProgramARB);
                gl::disable(gl::EnableComponent::AlphaTesting);
                gl::disable(gl::EnableComponent::FaceCulling);
                gl::client_state::disable(ClientArrayType::ColorArray);
                gl::vertex::set_color_rgba_f32(1.0, 1.0, 1.0, 1.0);
                gl::array::disable_attribute_array(chunk_vertex_program::ATTRIB_LIGHT_LEVELS);
                gl::texture::switch_active(ActiveTexture::Texture1);
                gl::disable(gl::EnableComponent::Texture2D);
                gl::texture::switch_active(ActiveTexture::Texture0);
            }
            // Render sky.
            {
                gl::enable(gl::EnableComponent::Blending);
                gl::fragment::set_blend_function(
                    gl::fragment::SrcBlendFactor::One,
                    gl::fragment::DstBlendFactor::One,
                );
                gl::matrix::switch_mode(gl::matrix::MatrixMode::Texture);
                gl::matrix::load_identity();
                gl::matrix::switch_mode(gl::matrix::MatrixMode::ModelView);
                let sky_matrix = Isometry3::new(
                    camera.pos.coords,
                    Vector3::new(
                        0.0,
                        0.0,
                        crate::graphics::environment::sky::get_day_cycle_rotation(time_of_day),
                    ),
                )
                .to_matrix()
                .prepend_scaling(camera.get_zfar() * 0.95);
                gl::matrix::load_f32_matrix(sky_matrix.as_ref());
                // Draw sun.
                {
                    self.resources.sun_texture.handle.bind(TexTarget::Texture2D);
                    static SUN_POSITIONS: [[f32; 3]; 4] = [
                        [0.90453404, 0.30151135, -0.30151135],
                        [0.90453404, -0.30151135, -0.30151135],
                        [0.90453404, -0.30151135, 0.30151135],
                        [0.90453404, 0.30151135, 0.30151135],
                    ];
                    static SUN_UVS: [[f32; 2]; 4] =
                        [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
                    gl::array::vertex_pointer(
                        3,
                        VertexType::F32,
                        0,
                        (&raw const SUN_POSITIONS).addr(),
                    );
                    gl::array::texture_coord_pointer(
                        2,
                        TextureCoordType::F32,
                        0,
                        (&raw const SUN_UVS).addr(),
                    );
                    gl::array::draw(gl::ShapeMode::Quads, 0, 4);
                }
                // Draw moon.
                {
                    self.resources
                        .moon_phases_texture
                        .handle
                        .bind(TexTarget::Texture2D);
                    static MOON_POSITIONS: [[f32; 3]; 4] = [
                        [-0.90453404, 0.30151135, -0.30151135],
                        [-0.90453404, -0.30151135, -0.30151135],
                        [-0.90453404, -0.30151135, 0.30151135],
                        [-0.90453404, 0.30151135, 0.30151135],
                    ];
                    // The moon phases texture is a 4x2 grid of moons.
                    let moon_uv_width = 1.0 / 4.0;
                    let moon_uv_height = 1.0 / 2.0;
                    let moon_phase_i = ((time_of_day as i64 - 6000) / 24000).rem_euclid(8);
                    let moon_uv_start_x = (moon_phase_i % 4) as f32 / 4.0;
                    let moon_uv_start_y = (moon_phase_i / 4) as f32 / 2.0;
                    let moon_uvs: [[f32; 2]; 4] = [
                        [moon_uv_start_x, moon_uv_start_y],
                        [moon_uv_start_x + moon_uv_width, moon_uv_start_y],
                        [
                            moon_uv_start_x + moon_uv_width,
                            moon_uv_start_y + moon_uv_height,
                        ],
                        [moon_uv_start_x, moon_uv_start_y + moon_uv_height],
                    ];
                    gl::array::vertex_pointer(
                        3,
                        VertexType::F32,
                        0,
                        (&raw const MOON_POSITIONS).addr(),
                    );
                    gl::array::texture_coord_pointer(
                        2,
                        TextureCoordType::F32,
                        0,
                        (&raw const moon_uvs).addr(),
                    );
                    gl::array::draw(gl::ShapeMode::Quads, 0, 4);
                }
                // Draw stars.
                let star_brightness = get_star_brightness(time_of_day);
                if star_brightness > 0.0 {
                    gl::texture::bind(TexTarget::Texture2D, None);
                    gl::client_state::disable(ClientArrayType::TextureCoordArray);
                    gl::disable(gl::EnableComponent::Texture2D);
                    gl::vertex::set_color_rgba_f32(
                        star_brightness,
                        star_brightness,
                        star_brightness,
                        0.0,
                    );
                    gl::array::vertex_pointer(
                        3,
                        VertexType::F32,
                        0,
                        (&raw const STAR_QUADS).addr(),
                    );
                    gl::array::draw(
                        gl::ShapeMode::Quads,
                        0,
                        (STAR_QUADS.len() * 4).try_into().unwrap(),
                    );
                }
                // Reset OpenGL state for debug graphics.
                gl::disable(gl::EnableComponent::Blending);
            }
            // Render debug graphics.
            // TODO: Get the `ignore_depth` flags working.
            {
                gl::enable(gl::EnableComponent::FaceCulling);
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
                    gl::array::vertex_pointer(3, VertexType::F32, 0, points.as_ptr().addr());
                    gl::array::color_pointer(4, ColorType::U8, 0, colours.as_ptr().addr());
                    gl::array::draw(
                        gl::ShapeMode::Triangles,
                        0,
                        points.len().try_into().unwrap(),
                    );
                }
                // Render debug lines.
                // TODO: Get line widths working.
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
                        VertexType::F32,
                        size_of::<DebugLineVertex>().try_into().unwrap(),
                        (&raw const converted_lines[0].pos).addr(),
                    );
                    gl::array::color_pointer(
                        4,
                        ColorType::U8,
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
                        VertexType::F32,
                        size_of::<DebugPoint>().try_into().unwrap(),
                        (&raw const debug_points[0].pos).addr(),
                    );
                    gl::array::color_pointer(
                        4,
                        ColorType::U8,
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
            cfg_select! {
                feature = "platform_winit" => {
                    glutin_surface
                        .swap_buffers(glutin_context)
                        .context("Error while swapping glutin surface buffers")?;
                }
                feature = "platform_linux_drm" => {
                    window_context.flip_page(self.graphics_options.vsync);
                }
            }
        }
        Ok(Some(debug_output))
    }

    fn wants_egui_debug_section(&self) -> bool {
        true
    }

    fn render_egui_debug_section(&mut self, ui: &mut egui::Ui) {
        ui.heading("Lightmap");
        ui.add(
            egui::Slider::new(&mut self.debug_lightmap_time_of_day, 22850.0..=24730.0)
                .text("Time of day"),
        );
        let time_of_day = self.debug_lightmap_time_of_day;
        let sky_light_level: f32 =
            crate::graphics::environment::sky::get_light_level_percentage(time_of_day);
        ui.label(format!("Current sky light level: {sky_light_level:.1}"));
        {
            let day_cycle_tick = time_of_day.round() as u64 % 24000;
            let night_percentage = match day_cycle_tick {
                // Daytime.
                730..11270 => 0.0,
                // Turning night.
                11270..13140 => (day_cycle_tick - 11270) as f32 / (13140 - 11270) as f32,
                // Night.
                13140..22860 => 1.0,
                // Turning day.
                22860..24000 | 0..730 => {
                    let adjusted_tick = if (0_u64..730).contains(&day_cycle_tick) {
                        day_cycle_tick + 24000
                    } else {
                        day_cycle_tick
                    };
                    1.0 - ((adjusted_tick - 22860) as f32 / (24730 - 22860) as f32)
                }
                _ => unreachable!(),
            };
            ui.label(format!("Night percentage: {night_percentage:.1}"));
        }
        // Update lightmap debug image.
        unsafe {
            let new_lightmap_bytes = crate::graphics::lightmap::generate_lightmap_texture(
                self.graphics_options.lightmap_gamma_setting,
                time_of_day,
            );
            let lightmap_image_data = self
                .egui_renderer
                .get_user_image_mut(self.debug_lightmap_image.id);
            lightmap_image_data
                .image
                .copy_from_slice(new_lightmap_bytes.as_flattened().as_flattened());
            lightmap_image_data.update_gl_texture();
        }
        ui.add(
            egui::Image::from_texture(self.debug_lightmap_image)
                .fit_to_exact_size(egui::vec2(400.0, 400.0)),
        );
    }

    // See comment on `CAMERA_ZNEAR_OVERRIDE` for why we need this.
    fn get_camera_znear_override(&self) -> Option<f32> {
        Some(CAMERA_ZNEAR_OVERRIDE)
    }
}

#[derive(Debug)]
pub struct GlTexture {
    pub handle: gl::texture::batch_collected::TextureHandle,
    pub width: u32,
    pub height: u32,
}

impl GlTexture {
    /// Creates an OpenGL 2D texture object from the atlas.
    /// After calling this, the currently bound OpenGL 2D texture will be unbound.
    ///
    /// # Safety
    ///
    /// The OpenGL context must be current.
    pub unsafe fn create_from_resource_atlas(atlas: &resources::texture::Atlas) -> Self {
        unsafe {
            let [handle] = gl::texture::batch_collected::TextureHandle::make_array();
            handle.bind(TexTarget::Texture2D);
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
            Self {
                handle,
                width: atlas.width,
                height: atlas.height,
            }
        }
    }

    /// Creates an OpenGL 2D texture object from the texture.
    /// After calling this, the currently bound OpenGL 2D texture will be unbound.
    ///
    /// # Safety
    ///
    /// The OpenGL context must be current.
    pub unsafe fn create_from_resource_texture(
        texture: &resources::texture::RawTexture,
        options: GlTextureCreateOptions,
    ) -> Self {
        unsafe {
            let [handle] = gl::texture::batch_collected::TextureHandle::make_array();
            handle.bind(TexTarget::Texture2D);
            gl::texture::set_wrap_s(TexTarget::Texture2D, options.wrap_s);
            gl::texture::set_wrap_t(TexTarget::Texture2D, options.wrap_t);
            gl::texture::set_mag_filter(TexTarget::Texture2D, options.mag_filter);
            gl::texture::set_min_filter(TexTarget::Texture2D, options.min_filter);
            gl::texture::set_image_2d(
                Texture2dTarget::Texture,
                0,
                TextureInternalFormat::Rgba,
                texture.width.try_into().unwrap(),
                texture.height.try_into().unwrap(),
                0,
                Texture2dFormat::Rgba,
                TextureDataType::U8,
                texture.texture_bytes.as_ptr() as *const (),
            );
            gl::texture::bind(TexTarget::Texture2D, None);
            Self {
                handle,
                width: texture.width,
                height: texture.height,
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlTextureCreateOptions {
    pub wrap_s: TexWrapMode,
    pub wrap_t: TexWrapMode,
    pub mag_filter: TexFilterMode,
    pub min_filter: TexFilterMode,
}

impl Default for GlTextureCreateOptions {
    fn default() -> Self {
        Self {
            wrap_s: TexWrapMode::Repeat,
            wrap_t: TexWrapMode::Repeat,
            mag_filter: TexFilterMode::Nearest,
            min_filter: TexFilterMode::Nearest,
        }
    }
}
