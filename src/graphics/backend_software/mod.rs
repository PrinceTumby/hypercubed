#![allow(clippy::std_instead_of_alloc)]

// TODO: Add lightmap and sky changes.

// TODO: Split GraphicsState across sub-backends so multiple can be compiled in, gate some by CPU
//       features (compile-time CPU arch detection, runtime CPU feature detection).
// - Rasterisation-based sub-backends will likely use the same chunk processing logic, so put that
//   in a `common` module.
cfg_if::cfg_if! {
    if #[cfg(feature = "graphics_subbackend_software_lb_simd_generic")] {
        pub mod lb_simd_generic;
        pub use lb_simd_generic as render;
    } else if #[cfg(feature = "graphics_subbackend_software_lb_simd_avx512")] {
        #[cfg(not(all(
            target_feature = "sse4.2",
            target_feature = "fma",
            target_feature = "avx512f",
            target_feature = "avx512dq",
            target_feature = "avx512bw",
            target_feature = "avx512vl",
        )))]
        compile_error!("The software AVX-512 sub-backend requires extra CPU features.");
        pub mod lb_simd_avx512;
        pub use lb_simd_avx512 as render;
    } else {
        compile_error!("The software renderer requires exactly one sub-backend to be enabled.");
    }
}

use crate::graphics::chunk::{HasSubchunkData, SubchunkData};
use crate::graphics::debug::{Line as DebugLine, Point as DebugPoint, Triangle as DebugTriangle};
use crate::graphics::{DebugOutput, DebugState, GraphicsBackend, GraphicsOptions};
use crate::ClientPlayState;
use ahash::{AHashMap, AHashSet};
use anyhow::Context;
use portable_std::{FastHashMap, FastHashSet};
use rayon::prelude::*;
use render::RenderTileBins;
use resources::GameResourceData;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use threadpool::ThreadPool;
use winit::window::Window;

pub struct GraphicsResources {
    pub block_registry: resources::block::Registry,
    pub model_registry: resources::block::model::ModelRegistry,
    pub atlas_texture: TextureAtlas,
    pub window: Arc<Window>,
}

pub struct GraphicsState {
    pub resources: Arc<GraphicsResources>,
    pub pixels: pixels::Pixels<'static>,
    pub render_tile_bins: render::RenderTileBins,
    pub graphics_options: GraphicsOptions,
    pub egui_renderer: render::egui_rendering::Renderer,
    subchunk_data_storage: SubchunkDataStorage,
    pub pending_subchunk_tx: Sender<Option<([i32; 3], render::chunk::Subchunk)>>,
    pub pending_subchunk_rx: Receiver<Option<([i32; 3], render::chunk::Subchunk)>>,
    pub current_dispatch_id_counter: u64,
    pub num_pending_subchunks: usize,
    pub size: winit::dpi::PhysicalSize<u32>,
}

pub struct SubchunkDataStorage {
    // TODO: Currently the Y coordinate is a chunk section index, rather than the subchunk Y
    //       coordinate. Consider changing to actually be the Y coordinate.
    pub subchunks: FastHashMap<[i32; 3], render::chunk::Subchunk>,
    pub loaded_chunks: FastHashSet<[i32; 2]>,
}

impl GraphicsBackend for GraphicsState {
    #[tracing::instrument(skip_all)]
    fn new(window: Arc<Window>, game_data: GameResourceData) -> anyhow::Result<Box<Self>> {
        let graphics_options = GraphicsOptions::default();
        let size = window.inner_size();
        let surface_texture = pixels::SurfaceTexture::new(size.width, size.height, window.clone());
        let pixels_builder = pixels::PixelsBuilder::new(size.width, size.height, surface_texture)
            .texture_format(pixels::wgpu::TextureFormat::Rgba8Unorm)
            .blend_state(pixels::wgpu::BlendState::REPLACE)
            .enable_vsync(graphics_options.vsync);
        #[cfg(target_os = "windows")]
        let pixels_builder =
            pixels_builder.surface_texture_format(pixels::wgpu::TextureFormat::Rgba8Unorm);
        let pixels = pixels_builder.build()?;
        let render_tile_bins = RenderTileBins::new(
            size.width.max(1).try_into().unwrap(),
            size.height.max(1).try_into().unwrap(),
        );
        // Load game resources.
        let resources::GameResourceData {
            block_data,
            environment_data: _,
        } = game_data;
        let resources::block::ResourceData {
            block_registry,
            model_registry,
            atlas,
        } = block_data;
        let atlas_texture = TextureAtlas::new(&atlas)
            .context("Error while creating block and item atlas tiled texture")?;
        let egui_renderer = render::egui_rendering::Renderer::new();
        let (pending_subchunk_tx, pending_subchunk_rx) = std::sync::mpsc::channel();
        Ok(Box::new(Self {
            resources: Arc::new(GraphicsResources {
                block_registry,
                model_registry,
                atlas_texture,
                window,
            }),
            pixels,
            render_tile_bins,
            graphics_options,
            egui_renderer,
            subchunk_data_storage: SubchunkDataStorage {
                subchunks: FastHashMap::new(),
                loaded_chunks: FastHashSet::new(),
            },
            pending_subchunk_tx,
            pending_subchunk_rx,
            current_dispatch_id_counter: 0,
            num_pending_subchunks: 0,
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

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.pixels
                .resize_surface(new_size.width, new_size.height)
                .unwrap();
            self.pixels
                .resize_buffer(new_size.width, new_size.height)
                .unwrap();
            self.render_tile_bins = RenderTileBins::new(
                new_size.width.try_into().unwrap(),
                new_size.height.try_into().unwrap(),
            );
        }
    }

    fn get_graphics_options(&self) -> GraphicsOptions {
        self.graphics_options
    }

    fn apply_new_graphics_options(&mut self, new_options: GraphicsOptions) {
        let old_options = core::mem::replace(&mut self.graphics_options, new_options);
        if new_options.vsync != old_options.vsync {
            self.pixels.enable_vsync(new_options.vsync);
        }
    }

    #[tracing::instrument(skip_all)]
    fn dispatch_subchunk_updates(
        &mut self,
        thread_pool: &ThreadPool,
        raw_chunks: Arc<AHashMap<[i32; 2], Arc<crate::RawChunk>>>,
        subchunks: AHashSet<[i32; 3]>,
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
            let resources = self.resources.clone();
            let raw_chunks = raw_chunks.clone();
            let pending_subchunk_tx = self.pending_subchunk_tx.clone();
            thread_pool.execute(move || {
                render::chunk::process_subchunk(
                    &resources.block_registry,
                    &resources.model_registry,
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
        _current_time_s: f64,
        egui_ctx: &egui::Context,
        egui_full_output: egui::output::FullOutput,
        debug_state: &DebugState,
        _debug_points: &[DebugPoint],
        _debug_lines: &[DebugLine],
        _debug_triangles: &[DebugTriangle],
    ) -> anyhow::Result<Option<DebugOutput>> {
        let camera = &play_state.camera;
        // Add pending subchunks.
        if self.num_pending_subchunks > 0 {
            let span = tracing::trace_span!("add_pending_subchunks");
            let _enter = span.enter();
            let mut subchunks_processed_this_frame: usize = 0;
            for subchunk_and_coords in self
                .pending_subchunk_rx
                .try_iter()
                .take(self.num_pending_subchunks)
            {
                self.num_pending_subchunks -= 1;
                let Some((subchunk_coords, subchunk)) = subchunk_and_coords else {
                    continue;
                };
                // Check that the new subchunk is newer than the subchunk it's replacing, or that
                // it's not replacing an old subchunk. If it's not, then skip it.
                if self
                    .subchunk_data_storage
                    .subchunks
                    .get(&subchunk_coords)
                    .map(|old_subchunk| subchunk.dispatch_id > old_subchunk.dispatch_id)
                    .unwrap_or(false)
                {
                    continue;
                }
                self.subchunk_data_storage
                    .subchunks
                    .insert(subchunk_coords, subchunk);
                subchunks_processed_this_frame += 1;
                if subchunks_processed_this_frame >= 16 {
                    break;
                }
            }
        }
        let pixels_per_point = egui_full_output.pixels_per_point;
        let egui_primitives = egui_ctx.tessellate(egui_full_output.shapes, pixels_per_point);
        self.render_tile_bins.clear();
        let reversed_view_matrix = camera.generate_reversed_depth_view_matrix();
        let camera_near_clip_plane = camera.generate_clipping_planes()[4];
        let debug_output = {
            // Gather visible subchunks.
            let mut subchunks = Vec::new();
            let debug_output = super::for_each_visible_subchunk(
                camera,
                &self.subchunk_data_storage.subchunks,
                &self.subchunk_data_storage.loaded_chunks,
                debug_state,
                |_subchunk_coords, subchunk| subchunks.push(subchunk),
            );
            // Bin visible subchunks.
            {
                let span = tracing::trace_span!("bin_visible_subchunks");
                let _enter = span.enter();
                let (width, height) = (self.render_tile_bins.width, self.render_tile_bins.height);
                let tiles_per_row: usize = self
                    .render_tile_bins
                    .tiles_per_row
                    .get()
                    .try_into()
                    .unwrap();
                let tile_bins_mutex = Mutex::new(&mut self.render_tile_bins);
                subchunks.into_par_iter().for_each(|subchunk| {
                    render::chunk::bin_subchunk(
                        &tile_bins_mutex,
                        (width, height),
                        tiles_per_row,
                        &reversed_view_matrix,
                        &camera_near_clip_plane,
                        &self.resources.atlas_texture,
                        subchunk,
                    )
                });
            }
            debug_output
        };
        self.egui_renderer.bin_to_tiles(
            &mut self.render_tile_bins,
            &self.size,
            egui_full_output.textures_delta.set,
            egui_primitives,
            pixels_per_point,
        );
        let window_buffer = self.pixels.frame_mut();
        let window_linear_framebuffer = render::LinearFramebufferRgba::from_raw(
            window_buffer,
            self.size.width,
            self.size.height,
        );
        render::render_tile_bins(
            &window_linear_framebuffer,
            &self.resources.atlas_texture,
            &self.egui_renderer,
            &mut self.render_tile_bins,
            render::Rgba::new(0.471, 0.655, 1.0, 1.0),
        );
        {
            let span = tracing::trace_span!("render_pixel_buffer");
            let _enter = span.enter();
            self.pixels
                .render()
                .context("Error while rendering pixel buffer")?;
        }
        self.egui_renderer
            .free_textures(&egui_full_output.textures_delta.free);
        Ok(Some(debug_output))
    }
}

pub struct TextureAtlas {
    texture: render::TiledTextureRgba,
}

impl TextureAtlas {
    pub fn new(atlas: &resources::texture::Atlas) -> anyhow::Result<Self> {
        Ok(Self {
            texture: render::TiledTextureRgba::from_atlas(atlas)?,
        })
    }

    pub fn get_texture(&self) -> &render::TiledTextureRgba {
        &self.texture
    }
}
