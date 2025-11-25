pub mod chunk;
pub mod debug;
pub mod egui_renderer;
pub mod gl;

use crate::client::{MIN_HEIGHT_I32, RawSubchunk, SUBCHUNK_AXIS_LEN_I32};
use crate::platform::libs::winit;
use crate::portable_prelude::*;
use anyhow::Context;
use debug::line::Instance as DebugLineInstance;
use debug::point::Vertex as DebugPointVertex;
use debug::triangle::Instance as DebugTriangleInstance;
use nalgebra::{Matrix4, Perspective3, Point3, Vector3};
use portable_std::{Arc, FastHashMap, FastHashSet};
use resources::block::model::ModelRegistry;
use resources::texture::RawAtlas;
use winit::window::Window;

use gl::array::{ColorPointerType, TextureCoordPointerType, VertexPointerType};
use gl::buffer::BufferType;
use gl::client_state::ClientArrayType;

pub use super::Camera;

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
    pub window: Arc<Window>,
    #[cfg(feature = "platform_winit")]
    glutin_resources: Arc<GlutinResources>,
    pub block_registry: Arc<resources::block::Registry>,
    pub model_registry: Arc<ModelRegistry>,
    pub atlas: Arc<RawAtlas>,
    pub atlas_texture: Arc<gl::texture::batch_collected::Texture>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GraphicsOptions {
    pub vsync: bool,
}

impl Default for GraphicsOptions {
    fn default() -> Self {
        Self { vsync: true }
    }
}

pub struct GraphicsState {
    pub resources: GraphicsResources,
    pub graphics_options: GraphicsOptions,
    pub egui_renderer: egui_renderer::Renderer,
    pub subchunk_data_queue: Vec<([i32; 3], RawSubchunk)>,
    // pub block_face_buffer_manager: BlockFaceBufferManager,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub camera: Camera,
}

#[derive(Clone, Copy, Debug)]
pub struct DebugState {
    pub visualisation_draw_method: DebugVisualisationDrawMethod,
    pub cull_planes_active: usize,
    pub rendering_view_frustum: bool,
    pub free_cam: bool,
    pub cave_cull_check_unflipped: bool,
    pub cave_cull_check_not_backwards: bool,
    pub cave_cull_check_frustum: bool,
    pub cave_cull_check_connectivity: bool,
    pub cave_cull_render_connectivity: bool,
    pub cave_cull_render_traversal_graph: bool,
    pub cave_cull_debug_render_dist: f32,
    pub max_render_chunks: usize,
    pub debug_texture_zoom: f32,
}

impl Default for DebugState {
    fn default() -> Self {
        Self {
            visualisation_draw_method: DebugVisualisationDrawMethod::default(),
            cull_planes_active: 6,
            rendering_view_frustum: false,
            free_cam: false,
            cave_cull_check_unflipped: true,
            cave_cull_check_not_backwards: false,
            cave_cull_check_frustum: true,
            cave_cull_check_connectivity: true,
            cave_cull_render_connectivity: false,
            cave_cull_render_traversal_graph: false,
            cave_cull_debug_render_dist: 24.0,
            max_render_chunks: 3000,
            debug_texture_zoom: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DebugVisualisationDrawMethod {
    #[default]
    Egui,
    Gpu,
}

impl DebugVisualisationDrawMethod {
    pub fn label_text(&self) -> &'static str {
        match *self {
            Self::Egui => "Use egui for debug visualisation",
            Self::Gpu => "Use the GPU directly for debug visualisation",
        }
    }
}

#[derive(Default)]
pub struct DebugOutput {
    pub subchunks_culled: usize,
    pub subchunk_traversal_graph: Vec<([i32; 3], [i32; 3])>,
}

impl GraphicsState {
    pub async fn new<F>(window: Arc<Window>, register_blocks: F) -> anyhow::Result<Self>
    where
        F: FnOnce(
            &mut resources::block::Registry,
            &mut resources::block::model::ModelRegistryBuilder,
            &mut resources::texture::AtlasBuilder,
        ) -> anyhow::Result<()>,
    {
        let graphics_options = GraphicsOptions::default();
        let size = window.inner_size();
        let camera = Camera {
            pos: Point3::new(0.0, 124.0, 0.0),
            proj_matrix: Perspective3::new(
                (size.width as f32) / (size.height as f32),
                f32::to_radians(super::DEFAULT_FOV),
                super::DEFAULT_ZNEAR,
                super::DEFAULT_ZFAR,
            ),
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
        };
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
        // Initialise game state.
        let (block_item_atlas, block_item_atlas_texture, block_registry, model_registry) = {
            let size = [1024; 2];
            let square_length = 16;
            let mut atlas_builder =
                resources::texture::AtlasBuilder::new(size[0], size[1], square_length);
            let mut model_cache = resources::block::model::ModelRegistryBuilder::new();
            let mut block_registry = resources::block::Registry::new();
            register_blocks(&mut block_registry, &mut model_cache, &mut atlas_builder)
                .context("Error while registering blocks")?;
            let atlas = atlas_builder.finish().into_raw();
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
            (atlas, atlas_texture, block_registry, model_cache.finish())
        };
        let egui_renderer = egui_renderer::Renderer::new();
        Ok(Self {
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
                atlas: Arc::new(block_item_atlas),
                atlas_texture: Arc::new(block_item_atlas_texture),
            },
            graphics_options,
            egui_renderer,
            subchunk_data_queue: Vec::new(),
            // block_face_buffer_manager,
            size,
            camera,
        })
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
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
            self.camera
                .proj_matrix
                .set_aspect((new_size.width as f32) / (new_size.height as f32));
        }
    }

    pub fn apply_new_graphics_options(&mut self, new_options: GraphicsOptions) {
        let old_options = std::mem::replace(&mut self.graphics_options, new_options);
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

    pub fn free_subchunk_data(&mut self, _subchunk_coords: [i32; 3]) {
        // We don't store any subchunk data here, so no need to do anything.
    }

    #[tracing::instrument(skip_all)]
    pub fn render(
        &mut self,
        subchunks: &mut FastHashMap<[i32; 3], chunk::Subchunk>,
        loaded_chunks: &FastHashSet<[i32; 2]>,
        egui_ctx: &egui::Context,
        egui_full_output: egui::output::FullOutput,
        debug_state: &DebugState,
        debug_points: &[DebugPointVertex],
        debug_lines: &[DebugLineInstance],
        debug_triangles: &[DebugTriangleInstance],
    ) -> anyhow::Result<DebugOutput> {
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
            // Upload new subchunks.
            for (subchunk_coords, raw_subchunk) in self.subchunk_data_queue.drain(..) {
                let mut faces = Vec::new();
                let mut group_start_vertices = [u32::MAX; 7];
                let mut group_vertex_counts = [0; 7];
                for (i, face_group) in raw_subchunk
                    .face_groups
                    .into_iter()
                    .enumerate()
                    .filter(|(_i, group)| !group.is_empty())
                {
                    group_start_vertices[i] = (faces.len() * 6).try_into().unwrap();
                    group_vertex_counts[i] = (face_group.len() * 6).try_into().unwrap();
                    faces.extend(face_group);
                }
                let buffer = if !faces.is_empty() {
                    let [buffer] = gl::buffer::batch_collected::Buffer::make_array();
                    buffer.bind(BufferType::ArrayBuffer);
                    let faces_bytes: &[u8] = bytemuck::cast_slice(&faces);
                    gl::buffer::set_current_buffer_data_raw(
                        BufferType::ArrayBuffer,
                        faces_bytes.len().try_into().unwrap(),
                        faces_bytes.as_ptr() as *const (),
                        gl::buffer::DataUsageHint::StaticDraw,
                    );
                    Some(buffer)
                } else {
                    None
                };
                subchunks.insert(
                    subchunk_coords,
                    chunk::Subchunk {
                        start_coords: raw_subchunk.start_coords,
                        buffer,
                        group_start_vertices,
                        group_vertex_counts,
                        connected_faces: raw_subchunk.connected_faces,
                    },
                );
            }
            // Clear framebuffer and depth buffer.
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
                    1.0 / self.resources.atlas.width as f32,
                    1.0 / self.resources.atlas.height as f32,
                    1.0,
                ));
                gl::matrix::load_f32_matrix(&texture_matrix.into());
                // Load camera projection matrix.
                gl::matrix::switch_mode(MatrixMode::Projection);
                gl::matrix::load_f32_matrix(&self.camera.generate_view_matrix_slice());
                let camera_subchunk_coords = {
                    // let camera_pos = debug_state.cull_camera.pos;
                    let camera_pos = self.camera.pos;
                    let camera_x = (camera_pos.x.floor() as i32).div_euclid(SUBCHUNK_AXIS_LEN_I32);
                    let camera_y = (camera_pos.y.floor() as i32 - MIN_HEIGHT_I32)
                        .div_euclid(SUBCHUNK_AXIS_LEN_I32);
                    let camera_z = (camera_pos.z.floor() as i32).div_euclid(SUBCHUNK_AXIS_LEN_I32);
                    let camera_subchunk_coords = [camera_x, camera_y, camera_z];
                    camera_subchunk_coords
                };
                {
                    let span = tracing::trace_span!("subchunks_set_client_state");
                    let _enter = span.enter();
                    gl::client_state::enable(ClientArrayType::VertexArray);
                    gl::client_state::enable(ClientArrayType::ColorArray);
                    gl::client_state::enable(ClientArrayType::TextureCoordArray);
                }
                debug_output = super::for_each_visible_subchunk(
                    &self.camera,
                    subchunks,
                    loaded_chunks,
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
                    let mut points = Vec::new();
                    let mut colours = Vec::new();
                    for line in debug_lines {
                        points.extend([line.p1, line.p2]);
                        colours.extend([line.color; 2]);
                    }
                    gl::array::vertex_pointer(3, VertexPointerType::F32, 0, points.as_ptr().addr());
                    gl::array::color_pointer(4, ColorPointerType::U8, 0, colours.as_ptr().addr());
                    gl::array::draw(gl::ShapeMode::Lines, 0, points.len().try_into().unwrap());
                }
                // Render debug points.
                // TODO: Get point sizes working.
                if !debug_points.is_empty() {
                    gl::array::vertex_pointer(
                        3,
                        VertexPointerType::F32,
                        size_of::<debug::point::Vertex>().try_into().unwrap(),
                        (&raw const debug_points[0].pos).addr(),
                    );
                    gl::array::color_pointer(
                        4,
                        ColorPointerType::U8,
                        size_of::<debug::point::Vertex>().try_into().unwrap(),
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
        // TODO:
        Ok(debug_output)
    }
}
