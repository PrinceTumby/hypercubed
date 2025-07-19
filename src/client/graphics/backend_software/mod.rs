pub mod chunk;
pub mod chunk_rc;
pub mod egui_renderer;
pub mod render;

use ahash::{AHashMap, AHashSet};
use image::{GrayImage, RgbaImage};
use nalgebra::{Perspective3, Point3};
use std::sync::Arc;
use threadpool::ThreadPool;
use winit::window::Window;

pub use super::Camera;

#[derive(Clone)]
pub struct GraphicsResources {
    pub block_registry: Arc<crate::resource::block::Registry>,
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
    pub pixels: pixels::Pixels,
    pub graphics_options: GraphicsOptions,
    pub egui_renderer: egui_renderer::Renderer,
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
    pub async fn new<F>(window: &'static Window, register_blocks: F) -> anyhow::Result<Self>
    where
        F: FnOnce(
            &mut crate::resource::block::Registry,
            &mut crate::resource::block::model::ModelCache,
            &mut crate::resource::texture::AtlasBuilder,
        ) -> anyhow::Result<()>,
    {
        let graphics_options = GraphicsOptions::default();
        let size = window.inner_size();
        let surface_texture = pixels::SurfaceTexture::new(size.width, size.height, window);
        let pixels = pixels::PixelsBuilder::new(size.width, size.height, surface_texture)
            .blend_state(pixels::wgpu::BlendState::REPLACE)
            .enable_vsync(graphics_options.vsync)
            .build_async()
            .await?;
        // Initialise game state
        let (
            block_item_texture_atlas,
            block_item_atlas_size,
            block_registry,
            custom_block_vertices,
            custom_block_indices,
        ) = {
            use crate::resource;
            let size = [1024; 2];
            let square_length = 16;
            let mut atlas_builder =
                resource::texture::AtlasBuilder::new(size[0], size[1], square_length);
            let mut model_cache = resource::block::model::ModelCache::new();
            let mut block_registry = resource::block::Registry::new();
            register_blocks(&mut block_registry, &mut model_cache, &mut atlas_builder)?;
            let atlas = atlas_builder.build();
            (
                atlas,
                size,
                block_registry,
                model_cache.custom_block_vertices,
                model_cache.custom_block_indices,
            )
        };
        let egui_renderer = egui_renderer::Renderer::new(&pixels);
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
                block_registry: Arc::new(block_registry),
            },
            pixels,
            graphics_options,
            egui_renderer,
            size,
            camera,
        })
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.pixels
                .resize_surface(new_size.width, new_size.height)
                .unwrap();
            self.pixels
                .resize_buffer(new_size.width, new_size.height)
                .unwrap();
            self.camera
                .proj_matrix
                .set_aspect((new_size.width as f32) / (new_size.height as f32));
        }
    }

    pub fn apply_new_graphics_options(&mut self, new_options: GraphicsOptions) {
        let old_options = std::mem::replace(&mut self.graphics_options, new_options);
        if new_options.vsync != old_options.vsync {
            self.pixels.enable_vsync(new_options.vsync);
        }
    }

    pub fn free_subchunk_data(&mut self, subchunk_coords: [i32; 3]) {
        _ = subchunk_coords;
        todo!();
    }

    pub fn render(
        &mut self,
        _subchunks: &AHashMap<[i32; 3], chunk_rc::Subchunk>,
        _loaded_chunks: &AHashSet<[i32; 2]>,
        egui_ctx: &egui::Context,
        egui_full_output: egui::output::FullOutput,
        _debug_state: &DebugState,
    ) -> anyhow::Result<DebugOutput> {
        let pixels_per_point = egui_full_output.pixels_per_point;
        let egui_primitives = egui_ctx.tessellate(egui_full_output.shapes, pixels_per_point);
        let window_buffer = self.pixels.frame_mut();
        let mut window_image_buffer =
            image::ImageBuffer::from_raw(self.size.width, self.size.height, window_buffer).unwrap();
        // Clear buffer
        for pixel in window_image_buffer.pixels_mut() {
            *pixel = image::Rgba([0x00, 0xFF, 0xFF, 0xFF]);
        }
        let render_data = self.egui_renderer.prepare(
            &self.pixels,
            &self.size,
            egui_full_output.textures_delta.set,
            egui_primitives,
            pixels_per_point,
        );
        self.pixels.render_with(|encoder, render_target, context| {
            context.scaling_renderer.render(encoder, render_target);
            self.egui_renderer
                .render(encoder, render_target, &render_data);
            Ok(())
        })?;
        self.egui_renderer
            .free_textures(&egui_full_output.textures_delta.free);
        Ok(DebugOutput::default())
    }

    pub fn radiance_cascades_debug_render(
        &mut self,
        _subchunks: &AHashMap<[i32; 3], chunk_rc::Subchunk>,
    ) {
        todo!();
    }

    pub fn update_all_subchunks_radiance_lighting(
        &mut self,
        _thread_pool: &ThreadPool,
        _subchunks: &AHashMap<[i32; 3], chunk_rc::Subchunk>,
    ) {
        todo!();
    }
}

#[derive(Debug)]
pub struct TextureAtlas {
    pub texture: RgbaImage,
    pub luma_texture: GrayImage,
}

impl TextureAtlas {
    pub fn from_builder(builder: crate::resource::texture::AtlasBuilder) -> Self {
        Self {
            texture: builder.texture,
            luma_texture: builder.luma_texture,
        }
    }
}
