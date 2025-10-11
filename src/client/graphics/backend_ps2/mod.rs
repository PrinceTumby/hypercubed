pub mod chunk;

use crate::platform::ps2::draw::{Framebuffer, ZBuffer};
use crate::platform::ps2::libs::winit;
use crate::portable_prelude::*;
use nalgebra::{Perspective3, Point3};
use portable_std::{Arc, FastHashMap, FastHashSet};
use resources::block::model::ModelRegistry;
use winit::window::Window;

pub use super::Camera;

#[derive(Clone)]
pub struct GraphicsResources {
    pub block_registry: Arc<resources::block::Registry>,
    pub model_registry: Arc<ModelRegistry>,
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
    pub max_radiance_cascade: u32,
    pub debug_texture_zoom: f32,
}

impl Default for DebugState {
    fn default() -> Self {
        Self {
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
            max_radiance_cascade: 0,
            debug_texture_zoom: 1.0,
        }
    }
}

#[derive(Default)]
pub struct DebugOutput {
    pub subchunks_culled: usize,
    pub subchunk_traversal_graph: Vec<([i32; 3], [i32; 3])>,
}

impl GraphicsState {
    pub async fn new(framebuffer: Framebuffer, z_buffer: ZBuffer) -> anyhow::Result<Self> {
        let graphics_options = GraphicsOptions::default();
        let size = winit::dpi::PhysicalSize {
            width: framebuffer.width(),
            height: framebuffer.height(),
        };
        dbg!();
        let embedded_cache = crate::platform::get_embedded_cache();
        dbg!();
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
        Ok(Self {
            resources: GraphicsResources {
                block_registry: Arc::new(embedded_cache.block_registry),
                model_registry: Arc::new(embedded_cache.models),
            },
            graphics_options,
            size,
            camera,
        })
    }

    pub fn resize(&mut self, _new_size: winit::dpi::PhysicalSize<u32>) {
        todo!();
    }

    pub fn apply_new_graphics_options(&mut self, _new_options: GraphicsOptions) {
        todo!()
        // let old_options = std::mem::replace(&mut self.graphics_options, new_options);
        // if new_options.vsync != old_options.vsync {
        //     self.pixels.enable_vsync(new_options.vsync);
        // }
    }

    pub fn free_subchunk_data(&mut self, _subchunk_coords: [i32; 3]) {
        todo!()
    }

    pub fn render(
        &mut self,
        _subchunks: &FastHashMap<[i32; 3], chunk::Subchunk>,
        _loaded_chunks: &FastHashSet<[i32; 2]>,
        _debug_state: &DebugState,
    ) -> anyhow::Result<DebugOutput> {
        todo!()
    }
}
