pub mod chunk;
pub mod debug;

use crate::platform::libs::winit;
use crate::portable_prelude::*;
use debug::line::Instance as DebugLineInstance;
use debug::point::Vertex as DebugPointVertex;
use debug::triangle::Instance as DebugTriangleInstance;
use nalgebra::{Perspective3, Point3};
use portable_std::{Arc, FastHashMap, FastHashSet};
use resources::block::model::ModelRegistry;
use resources::texture::RawAtlas;

pub use super::Camera;

#[derive(Clone)]
pub struct GraphicsResources {
    pub block_registry: Arc<resources::block::Registry>,
    pub model_registry: Arc<ModelRegistry>,
    pub atlas: Arc<RawAtlas>,
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
    pub radiance_cascades_ray_visualiser: bool,
    pub radiance_cascades_light_tree_visualiser: bool,
    pub radiance_cascades_light_tree_level: usize,
    pub radiance_cascades_areaquad_visualiser: bool,
    pub max_radiance_cascade: u32,
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
            radiance_cascades_ray_visualiser: false,
            radiance_cascades_light_tree_visualiser: false,
            radiance_cascades_light_tree_level: 0,
            radiance_cascades_areaquad_visualiser: false,
            max_radiance_cascade: 0,
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
    pub fn new(width: u32, height: u32) -> anyhow::Result<Self> {
        let graphics_options = GraphicsOptions::default();
        let size = winit::dpi::PhysicalSize { width, height };
        let embedded_cache = crate::platform::get_embedded_cache();
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
        // unsafe {
        // gl::clear_color(0.471, 0.655, 1.0, 1.0);
        // gl::viewport(0, 0, width.try_into().unwrap(), height.try_into().unwrap());
        // gl::enable(gl::EnableComponent::DepthTest);
        // gl::pixel_store_i32(gl::PixelStoreParam::UnpackAlignment, 1);
        // }
        if true {
            todo!("GraphicsState::new");
        }
        Ok(Self {
            resources: GraphicsResources {
                block_registry: Arc::new(embedded_cache.block_registry),
                model_registry: Arc::new(embedded_cache.models),
                atlas: Arc::new(embedded_cache.atlas),
            },
            graphics_options,
            size,
            camera,
        })
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        todo!("GraphicsState::resize({new_size:?})");
        unsafe {
            self.size = new_size;
            // gl::viewport(
            // 0,
            // 0,
            // new_size.width.try_into().unwrap(),
            // new_size.height.try_into().unwrap(),
            // );
            self.camera
                .proj_matrix
                .set_aspect((new_size.width as f32) / (new_size.height as f32));
        }
    }

    pub fn apply_new_graphics_options(&mut self, _new_options: GraphicsOptions) {
        todo!("GraphicsState::apply_new_graphics_options")
        // let old_options = std::mem::replace(&mut self.graphics_options, new_options);
        // if new_options.vsync != old_options.vsync {
        //     self.pixels.enable_vsync(new_options.vsync);
        // }
    }

    pub fn free_subchunk_data(&mut self, _subchunk_coords: [i32; 3]) {
        todo!("GraphicsState::free_subchunk_data")
    }

    // pub fn render(
    //     &mut self,
    //     _subchunks: &FastHashMap<[i32; 3], chunk::Subchunk>,
    //     _loaded_chunks: &FastHashSet<[i32; 2]>,
    //     _debug_state: &DebugState,
    // ) -> anyhow::Result<DebugOutput> {
    //     static TEST_RECTANGLE_VERTICES: [[f32; 3]; 4] = [
    //         [-0.5, 123.0, -1.0],
    //         [0.5, 123.0, -1.0],
    //         [-0.5, 125.0, -1.0],
    //         [0.5, 125.0, -1.0],
    //     ];
    //     static TEST_RECTANGLE_COLORS: [[f32; 3]; 4] = [
    //         [1.0, 0.0, 0.0],
    //         [0.0, 1.0, 0.0],
    //         [0.0, 0.0, 1.0],
    //         [1.0, 0.0, 1.0],
    //     ];
    //     static TEST_RECTANGLE_UV_COORDS: [[f32; 2]; 4] = [
    //         [0.0, 1.0],
    //         [1.0, 1.0],
    //         [0.0, 0.0],
    //         [1.0, 0.0],
    //     ];
    //     unsafe {
    //         gl::clear(gl::ClearBufferBits::COLOR | gl::ClearBufferBits::DEPTH);
    //         gl::enable(gl::EnableComponent::Texture2d);
    //         gl::client_active_texture(0);
    //         gl::tex_env_mode(gl::TexEnvTarget::TextureEnv, gl::TexEnvMode::Modulate);
    //         gl::tex_image_2d(
    //             gl::Texture2dTarget::Texture,
    //             0,
    //             gl::TextureInternalFormat::Rgba,
    //             self.resources.atlas.width as usize,
    //             self.resources.atlas.height as usize,
    //             0,
    //             gl::Texture2dFormat::Rgba,
    //             gl::TextureDataType::U8,
    //             self.resources.atlas.texture_bytes.as_ptr(),
    //         );
    //         gl::vertex_pointer(
    //             3,
    //             gl::VertexPointerType::F32,
    //             0,
    //             &raw const TEST_RECTANGLE_VERTICES as *const u8,
    //         );
    //         gl::color_pointer(
    //             3,
    //             gl::ColorPointerType::F32,
    //             0,
    //             &raw const TEST_RECTANGLE_COLORS as *const u8,
    //         );
    //         gl::texture_coord_pointer(
    //             2,
    //             gl::TextureCoordPointerType::F32,
    //             0,
    //             &raw const TEST_RECTANGLE_UV_COORDS as *const u8,
    //         );
    //         gl::enable_client_state(gl::ClientArrayType::VertexArray);
    //         gl::enable_client_state(gl::ClientArrayType::ColorArray);
    //         gl::enable_client_state(gl::ClientArrayType::TextureCoordArray);
    //         gl::draw_arrays(gl::ShapeMode::TriangleStrip, 0, TEST_RECTANGLE_VERTICES.len());
    //         gl::disable(gl::EnableComponent::Texture2d);
    //     }
    //     // TODO:
    //     Ok(DebugOutput::default())
    // }

    pub fn render(
        &mut self,
        _subchunks: &FastHashMap<[i32; 3], chunk::Subchunk>,
        _loaded_chunks: &FastHashSet<[i32; 2]>,
    ) -> anyhow::Result<DebugOutput> {
        todo!("GraphicsState::render")
    }
}
