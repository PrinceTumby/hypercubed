pub mod chunk;
pub mod debug;
pub mod egui_renderer;
pub mod environment;

use std::sync::mpsc::{Receiver, Sender};

use anyhow::bail;
use chunk::{
    block_face::{BlockFaceInstanceBufferManager, BlockFaceVertexBufferManager},
    custom_block::CustomBlockInstanceBufferManager,
    tinted_block_face::{TintedBlockFaceInstanceBufferManager, TintedBlockFaceVertexBufferManager},
};
use portable_std::{Arc, FastHashMap, FastHashMapEntry, FastHashSet};
use resources::GameResourceData;
use resources::block::model::Tint;
use threadpool::ThreadPool;
use wgpu::util::DeviceExt as _;
use winit::event_loop::OwnedDisplayHandle;
use winit::window::Window;

use crate::graphics::chunk::{HasSubchunkData, SubchunkData};
use crate::graphics::debug::{Line as DebugLine, Point as DebugPoint, Triangle as DebugTriangle};
use crate::graphics::environment::sky::{STAR_QUADS, SkyExtrapolationState};
use crate::graphics::lightmap::{RawLightmapTexture, generate_lightmap_texture};
use crate::graphics::{DebugOutput, DebugState, GraphicsBackend, GraphicsOptions};
use crate::{ClientPlayState, MIN_HEIGHT_I32, SUBCHUNK_AXIS_LEN_I32};

mod wesl_include {
    /// Makes a [`wgpu::ShaderModuleDescriptor`] from the given WESL module name.
    /// The WESL module must have been specified in the `build.rs` file.
    macro_rules! include_wesl_module {
        ($module_name:literal) => {
            ::wgpu::ShaderModuleDescriptor {
                label: Some($module_name),
                source: ::wgpu::ShaderSource::Wgsl(::wesl::include_wesl!($module_name).into()),
            }
        };
    }
    pub(crate) use include_wesl_module;
}
pub(crate) use wesl_include::include_wesl_module;

mod common_bind_group_idxs {
    include!("backend_wgpu/shaders/common_bind_group_idxs.wesl");
    /// Render info uniform buffer.
    pub const RENDER_INFO_IDX: u32 = RENDER_INFO;
    /// Lightmap storage buffer.
    pub const LIGHTMAP_IDX: u32 = LIGHTMAP;
    /// Basic sampler used for textures.
    pub const BASIC_SAMPLER_IDX: u32 = BASIC_SAMPLER;
    /// Block and item atlas texture.
    pub const BLOCK_ITEM_ATLAS_IDX: u32 = BLOCK_ITEM_ATLAS;
    /// Custom block faces storage buffer.
    pub const CUSTOM_BLOCK_FACES_IDX: u32 = CUSTOM_BLOCK_FACES;
    /// Sun texture.
    pub const SUN_IDX: u32 = SUN;
    /// Moon phases texture.
    pub const MOON_PHASES_IDX: u32 = MOON_PHASES;
}

pub struct GraphicsResources {
    pub block_registry: resources::block::Registry,
    pub model_registry: resources::block::model::ModelRegistry,
    pub surface: wgpu::Surface<'static>,
    pub queue: wgpu::Queue,
    pub device: wgpu::Device,
}

impl GraphicsOptions {
    pub fn get_wgpu_present_mode(&self) -> wgpu::PresentMode {
        match self.vsync {
            true => wgpu::PresentMode::Fifo,
            false => wgpu::PresentMode::Immediate,
        }
    }
}

pub struct SubchunkDataStorage {
    // TODO: Currently the Y coordinate is a chunk section index, rather than the subchunk Y
    //       coordinate. Consider changing to actually be the Y coordinate.
    pub subchunks: FastHashMap<[i32; 3], chunk::Subchunk>,
    pub loaded_chunks: FastHashSet<[i32; 2]>,
    pub block_face_vertex: BlockFaceVertexBufferManager,
    pub block_face_instance: BlockFaceInstanceBufferManager,
    pub tinted_block_face_vertex: TintedBlockFaceVertexBufferManager,
    pub tinted_block_face_instance: TintedBlockFaceInstanceBufferManager,
    pub custom_block_instance: CustomBlockInstanceBufferManager,
}

impl SubchunkDataStorage {
    pub fn remove_subchunk(&mut self, subchunk_coords: [i32; 3]) {
        let FastHashMapEntry::Occupied(subchunk_entry) = self.subchunks.entry(subchunk_coords)
        else {
            return;
        };
        subchunk_entry.remove();
        // Free buffer areas.
        self.block_face_vertex.free_subchunk_areas(subchunk_coords);
        self.block_face_instance
            .free_subchunk_areas(subchunk_coords);
        self.tinted_block_face_vertex
            .free_subchunk_areas(subchunk_coords);
        self.tinted_block_face_instance
            .free_subchunk_areas(subchunk_coords);
        self.custom_block_instance
            .free_subchunk_areas(subchunk_coords);
    }
}

pub struct EnvironmentGraphicsState {
    pub sun_render_pipeline: wgpu::RenderPipeline,
    pub moon_render_pipeline: wgpu::RenderPipeline,
    pub star_render_pipeline: wgpu::RenderPipeline,
    pub star_quads_buffer: wgpu::Buffer,
    pub moon_phases_texture: HypercubedWgpuTexture,
    pub sun_texture: HypercubedWgpuTexture,
}

pub struct GraphicsState {
    pub size: winit::dpi::PhysicalSize<u32>,
    pub resources: Arc<GraphicsResources>,
    pub config: wgpu::SurfaceConfiguration,
    pub graphics_options: GraphicsOptions,
    pub common_bind_group_layout: wgpu::BindGroupLayout,
    pub common_bind_group: wgpu::BindGroup,
    pub block_render_pipeline: wgpu::RenderPipeline,
    pub tinted_block_render_pipeline: wgpu::RenderPipeline,
    pub custom_block_render_pipeline: wgpu::RenderPipeline,
    pub debug_point_render_pipeline: wgpu::RenderPipeline,
    pub debug_line_render_pipeline: wgpu::RenderPipeline,
    pub debug_triangle_render_pipeline: wgpu::RenderPipeline,
    pub egui_renderer: egui_renderer::Renderer,
    pub depth_texture: Texture,
    pub custom_block_faces_buffer: wgpu::Buffer,
    pub block_item_atlas: HypercubedWgpuTexture,
    pub base_render_info: RenderInfo,
    pub render_info_buffer: wgpu::Buffer,
    pub lightmap_buffer: wgpu::Buffer,
    pub environment_state: EnvironmentGraphicsState,
    pub subchunk_data_storage: SubchunkDataStorage,
    pub pending_subchunk_tx: Sender<Option<chunk::RawSubchunk>>,
    pub pending_subchunk_rx: Receiver<Option<chunk::RawSubchunk>>,
    pub current_dispatch_id_counter: u64,
    pub num_pending_subchunks: usize,
    pub sky_extrapolation_state: SkyExtrapolationState,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RenderInfo {
    pub view_matrix: [[f32; 4]; 4],
    pub sky_matrix: [[f32; 4]; 4],
    /// `[1.0 / screen.width, 1.0 / screen.height]`
    pub recip_screen_size: [f32; 2],
    /// `[1.0 / atlas.width, 1.0 / atlas.height]`
    pub recip_block_item_atlas_size: [f32; 2],
    pub face_matrices: [[[f32; 4]; 3]; 6],
    /// `time_of_day.rem_euclid(192_000.0)`
    pub time_of_day: f32,
    pub star_brightness: f32,
    /// Required padding, as WGSL uniforms have a required alignment of 16.
    pub padding_0: [u32; 2],
}

impl GraphicsBackend for GraphicsState {
    #[tracing::instrument(skip_all)]
    fn new(
        window: Arc<Window>,
        display: OwnedDisplayHandle,
        game_data: GameResourceData,
    ) -> anyhow::Result<Box<Self>> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::from_env_or_default(),
            display: Some(Box::new(display)),
        });
        let surface = instance.create_surface(window)?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .unwrap();
        log::debug!("WGPU Adapter Info - {:?}", adapter.get_info());
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::INDIRECT_FIRST_INSTANCE,
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::Performance,
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            trace: wgpu::Trace::Off,
        }))
        .unwrap();
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats[0];
        // HACK: On Windows, the "preferred" format is an sRGB format, when this doesn't actually
        //       seem to be correct? Just fix it up here for now.
        #[cfg(target_os = "windows")]
        let surface_format = surface_format.remove_srgb_suffix();
        let graphics_options = GraphicsOptions::default();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: graphics_options.get_wgpu_present_mode(),
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &config);
        let depth_texture = Texture::create_depth_texture(&device, &config, "Depth Texture");
        let common_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Common Bind Group Layout"),
                entries: &[
                    // Render info.
                    wgpu::BindGroupLayoutEntry {
                        binding: common_bind_group_idxs::RENDER_INFO_IDX,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Lightmap storage buffer.
                    wgpu::BindGroupLayoutEntry {
                        binding: common_bind_group_idxs::LIGHTMAP_IDX,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Basic texture sampler.
                    wgpu::BindGroupLayoutEntry {
                        binding: common_bind_group_idxs::BASIC_SAMPLER_IDX,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                        count: None,
                    },
                    // Block and item atlas texture.
                    wgpu::BindGroupLayoutEntry {
                        binding: common_bind_group_idxs::BLOCK_ITEM_ATLAS_IDX,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        },
                        count: None,
                    },
                    // Custom block faces buffer.
                    wgpu::BindGroupLayoutEntry {
                        binding: common_bind_group_idxs::CUSTOM_BLOCK_FACES_IDX,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Sun texture.
                    wgpu::BindGroupLayoutEntry {
                        binding: common_bind_group_idxs::SUN_IDX,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        },
                        count: None,
                    },
                    // Moon phases texture.
                    wgpu::BindGroupLayoutEntry {
                        binding: common_bind_group_idxs::MOON_PHASES_IDX,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        },
                        count: None,
                    },
                ],
            });
        let common_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Common Sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        // Load game resources.
        let resources::GameResourceData {
            block_data,
            environment_data,
        } = game_data;
        let resources::block::ResourceData {
            block_registry,
            model_registry,
            atlas,
        } = block_data;
        let resources::environment::ResourceData {
            moon_phases_texture,
            sun_texture,
        } = environment_data;
        let (
            block_item_texture_atlas,
            block_item_atlas_size,
            custom_block_faces_buffer,
            block_registry,
            model_registry,
        ) = {
            let atlas_texture = HypercubedWgpuTexture::create_from_resource_atlas(
                &atlas,
                &device,
                &queue,
                Some("Block and Item Atlas"),
            );
            let custom_block_faces: Vec<_> = model_registry
                .custom_block_faces
                .iter()
                .map(|face| {
                    face.vertices.map(|v| chunk::custom_block::Vertex {
                        pos: *v.local_pos.coords.as_ref(),
                        uvs: v.uvs,
                        normal: *face.normal.as_ref(),
                        tint_percentage: if matches!(face.tint, Some(Tint::Biome)) {
                            1.0
                        } else {
                            0.0
                        },
                    })
                })
                .collect();
            let custom_block_faces_buffer =
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Custom Block Vertices Buffer"),
                    contents: bytemuck::cast_slice(&custom_block_faces),
                    usage: wgpu::BufferUsages::STORAGE,
                });
            (
                atlas_texture,
                [atlas.width, atlas.height],
                custom_block_faces_buffer,
                block_registry,
                model_registry,
            )
        };
        // Load sun and moon textures.
        let sun_texture = HypercubedWgpuTexture::create_from_resource_texture(
            &sun_texture,
            &device,
            &queue,
            Some("Sun"),
        );
        let moon_phases_texture = HypercubedWgpuTexture::create_from_resource_texture(
            &moon_phases_texture,
            &device,
            &queue,
            Some("Moon Phases"),
        );
        // Create base rendering info, constant between frames.
        let base_render_info = RenderInfo {
            view_matrix: Default::default(),
            recip_screen_size: Default::default(),
            recip_block_item_atlas_size: block_item_atlas_size.map(|n| (n as f32).recip()),
            face_matrices: chunk::block_face::face_matrices::generate_array(),
            sky_matrix: Default::default(),
            time_of_day: Default::default(),
            star_brightness: Default::default(),
            padding_0: Default::default(),
        };
        // Block render pipelines.
        let common_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Common Pipeline Layout"),
                bind_group_layouts: &[Some(&common_bind_group_layout)],
                immediate_size: 0,
            });
        let block_render_pipeline =
            chunk::block_face::create_render_pipeline(&device, &config, &common_pipeline_layout);
        let tinted_block_render_pipeline = chunk::tinted_block_face::create_render_pipeline(
            &device,
            &config,
            &common_pipeline_layout,
        );
        let custom_block_render_pipeline =
            chunk::custom_block::create_render_pipeline(&device, &config, &common_pipeline_layout);
        // Environment render pipelines.
        let sun_render_pipeline =
            environment::sky::create_sun_render_pipeline(&device, &config, &common_pipeline_layout);
        let moon_render_pipeline = environment::sky::create_moon_render_pipeline(
            &device,
            &config,
            &common_pipeline_layout,
        );
        let star_render_pipeline = environment::sky::create_star_render_pipeline(
            &device,
            &config,
            &common_pipeline_layout,
        );
        // Star quads buffer.
        let star_quads_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Star Quads Buffer"),
            contents: bytemuck::cast_slice(
                &STAR_QUADS
                    .iter()
                    .map(|&[p1, p2, p3, p4]| {
                        environment::sky::StarInstance(
                            [p1, p2, p4, p3].map(environment::sky::StarVertex),
                        )
                    })
                    .collect::<Vec<_>>(),
            ),
            usage: wgpu::BufferUsages::VERTEX,
        });
        // Debug render pipelines.
        let debug_graphics_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Debug Graphics Render Pipeline Layout"),
                bind_group_layouts: &[Some(&common_bind_group_layout)],
                immediate_size: 0,
            });
        let debug_point_render_pipeline =
            debug::point::create_render_pipeline(&device, &config, &debug_graphics_pipeline_layout);
        let debug_line_render_pipeline =
            debug::line::create_render_pipeline(&device, &config, &debug_graphics_pipeline_layout);
        let debug_triangle_render_pipeline = debug::triangle::create_render_pipeline(
            &device,
            &config,
            &debug_graphics_pipeline_layout,
        );
        // `egui` renderer.
        let egui_renderer =
            egui_renderer::Renderer::new(&device, &config, &common_bind_group_layout);
        // Buffer managers.
        let block_face_vertex_buffer_manager = BlockFaceVertexBufferManager::new(&device);
        let block_face_instance_buffer_manager = BlockFaceInstanceBufferManager::new(&device);
        let tinted_block_face_vertex_buffer_manager =
            TintedBlockFaceVertexBufferManager::new(&device);
        let tinted_block_face_instance_buffer_manager =
            TintedBlockFaceInstanceBufferManager::new(&device);
        let custom_block_instance_buffer_manager = CustomBlockInstanceBufferManager::new(&device);
        // Buffers
        let render_info_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Render Info Buffer"),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            size: core::mem::size_of::<RenderInfo>().try_into().unwrap(),
            mapped_at_creation: false,
        });
        let lightmap_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lightmap Buffer"),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            size: core::mem::size_of::<RawLightmapTexture>()
                .try_into()
                .unwrap(),
            mapped_at_creation: false,
        });
        let common_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Common Bind Group"),
            layout: &common_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: common_bind_group_idxs::RENDER_INFO_IDX,
                    resource: render_info_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: common_bind_group_idxs::LIGHTMAP_IDX,
                    resource: lightmap_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: common_bind_group_idxs::BASIC_SAMPLER_IDX,
                    resource: wgpu::BindingResource::Sampler(&common_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: common_bind_group_idxs::BLOCK_ITEM_ATLAS_IDX,
                    resource: wgpu::BindingResource::TextureView(&block_item_texture_atlas.view),
                },
                wgpu::BindGroupEntry {
                    binding: common_bind_group_idxs::CUSTOM_BLOCK_FACES_IDX,
                    resource: custom_block_faces_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: common_bind_group_idxs::SUN_IDX,
                    resource: wgpu::BindingResource::TextureView(&sun_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: common_bind_group_idxs::MOON_PHASES_IDX,
                    resource: wgpu::BindingResource::TextureView(&moon_phases_texture.view),
                },
            ],
        });
        let (pending_subchunk_tx, pending_subchunk_rx) = std::sync::mpsc::channel();
        Ok(Box::new(Self {
            resources: Arc::new(GraphicsResources {
                block_registry,
                model_registry,
                surface,
                queue,
                device,
            }),
            config,
            graphics_options,
            common_bind_group_layout,
            common_bind_group,
            block_render_pipeline,
            tinted_block_render_pipeline,
            custom_block_render_pipeline,
            debug_point_render_pipeline,
            debug_line_render_pipeline,
            debug_triangle_render_pipeline,
            egui_renderer,
            depth_texture,
            custom_block_faces_buffer,
            block_item_atlas: block_item_texture_atlas,
            base_render_info,
            render_info_buffer,
            lightmap_buffer,
            environment_state: EnvironmentGraphicsState {
                sun_render_pipeline,
                moon_render_pipeline,
                star_render_pipeline,
                star_quads_buffer,
                moon_phases_texture,
                sun_texture,
            },
            size,
            subchunk_data_storage: SubchunkDataStorage {
                subchunks: FastHashMap::new(),
                loaded_chunks: FastHashSet::new(),
                block_face_vertex: block_face_vertex_buffer_manager,
                block_face_instance: block_face_instance_buffer_manager,
                tinted_block_face_vertex: tinted_block_face_vertex_buffer_manager,
                tinted_block_face_instance: tinted_block_face_instance_buffer_manager,
                custom_block_instance: custom_block_instance_buffer_manager,
            },
            pending_subchunk_tx,
            pending_subchunk_rx,
            current_dispatch_id_counter: 0,
            num_pending_subchunks: 0,
            sky_extrapolation_state: SkyExtrapolationState::new(),
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
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.resources
                .surface
                .configure(&self.resources.device, &self.config);
            self.depth_texture = Texture::create_depth_texture(
                &self.resources.device,
                &self.config,
                "depth_texture",
            );
        }
    }

    fn get_graphics_options(&self) -> GraphicsOptions {
        self.graphics_options
    }

    #[tracing::instrument(skip_all)]
    fn apply_new_graphics_options(&mut self, new_options: GraphicsOptions) {
        if new_options.vsync != self.graphics_options.vsync {
            self.config.present_mode = new_options.get_wgpu_present_mode();
            self.resources
                .surface
                .configure(&self.resources.device, &self.config);
        }
        self.graphics_options = new_options;
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
            let resources = self.resources.clone();
            let raw_chunks = raw_chunks.clone();
            let pending_subchunk_tx = self.pending_subchunk_tx.clone();
            thread_pool.execute(move || {
                chunk::process_subchunk(
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
                self.subchunk_data_storage.remove_subchunk(subchunk_coords);
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
                chunk::finalise_subchunk(
                    &mut self.subchunk_data_storage,
                    &self.resources.queue,
                    raw_subchunk,
                );
                subchunks_processed_this_frame += 1;
                if subchunks_processed_this_frame >= 16 {
                    break;
                }
            }
            self.resources.queue.submit([]);
        }
        let pixels_per_point = egui_full_output.pixels_per_point;
        let egui_primitives = egui_ctx.tessellate(egui_full_output.shapes, pixels_per_point);
        // Calculate the time of day and a sky colour for the current frame.
        let time_of_day = self
            .sky_extrapolation_state
            .update(play_state, current_time_s);
        let [sky_r, sky_g, sky_b] = crate::graphics::environment::sky::get_rgb(time_of_day);
        // Generate environment info.
        let sky_matrix: [[f32; 4]; 4] = {
            let sky_model_matrix = nalgebra::Isometry3::new(
                camera.pos.coords,
                nalgebra::Vector3::new(
                    0.0,
                    0.0,
                    crate::graphics::environment::sky::get_day_cycle_rotation(time_of_day),
                ),
            )
            .to_matrix()
            .prepend_scaling(camera.get_zfar() * 0.95);
            (camera.generate_reversed_depth_view_matrix() * sky_model_matrix).into()
        };
        let star_brightness = crate::graphics::environment::sky::get_star_brightness(time_of_day);
        // Update rendering info.
        self.resources.queue.write_buffer(
            &self.render_info_buffer,
            0,
            bytemuck::bytes_of(&RenderInfo {
                view_matrix: camera.generate_reversed_depth_view_matrix_slice(),
                sky_matrix,
                recip_screen_size: <[u32; 2]>::from(self.size).map(|n| (n as f32).recip()),
                face_matrices: self.base_render_info.face_matrices,
                recip_block_item_atlas_size: self.base_render_info.recip_block_item_atlas_size,
                time_of_day: time_of_day.rem_euclid(192_000.0) as f32,
                star_brightness,
                padding_0: Default::default(),
            }),
        );
        // Update lightmap.
        self.resources.queue.write_buffer(
            &self.lightmap_buffer,
            0,
            bytemuck::bytes_of(&generate_lightmap_texture(
                self.graphics_options.lightmap_gamma_setting,
                time_of_day,
            )),
        );
        let egui_render_data = self.egui_renderer.prepare(
            &self.resources,
            &self.size,
            egui_full_output.textures_delta.set,
            egui_primitives,
            pixels_per_point,
        );
        let (output, surface_suboptimal) = match self.resources.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(output) => (output, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(output) => (output, true),
            wgpu::CurrentSurfaceTexture::Occluded => return Ok(None),
            wgpu::CurrentSurfaceTexture::Validation => {
                bail!("Validation error while getting surface texture")
            }
            wgpu::CurrentSurfaceTexture::Timeout => return Ok(None),
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.resize(self.size);
                return Ok(None);
            }
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder =
            self.resources
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });
        // Main render pass.
        let block_face_draw_args_buffer;
        let tinted_block_face_draw_args_buffer;
        let custom_block_draw_args_buffer;
        let debug_point_buffer;
        let debug_line_buffer;
        let debug_triangle_buffer;
        let debug_output;
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: sky_r as f64,
                            g: sky_g as f64,
                            b: sky_b as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            let camera_chunk_coords = {
                let camera_pos = camera.pos;
                let camera_x = (camera_pos.x.floor() as i32).div_euclid(SUBCHUNK_AXIS_LEN_I32);
                let camera_y = (camera_pos.y.floor() as i32 - MIN_HEIGHT_I32)
                    .div_euclid(SUBCHUNK_AXIS_LEN_I32);
                let camera_z = (camera_pos.z.floor() as i32).div_euclid(SUBCHUNK_AXIS_LEN_I32);
                [camera_x, camera_y, camera_z]
            };
            #[repr(C)]
            #[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
            pub struct DrawIndirectArgs {
                pub num_vertices: u32,
                pub num_instances: u32,
                pub start_vertex: u32,
                pub start_instance: u32,
            }
            let mut block_face_draw_args: Vec<DrawIndirectArgs> = Vec::new();
            let mut tinted_block_face_draw_args: Vec<DrawIndirectArgs> = Vec::new();
            let mut custom_block_draw_args: Vec<DrawIndirectArgs> = Vec::new();
            debug_output = super::for_each_visible_subchunk(
                camera,
                &self.subchunk_data_storage.subchunks,
                &self.subchunk_data_storage.loaded_chunks,
                debug_state,
                |subchunk_coords, subchunk| {
                    for i in 0..6 {
                        let skip_face_dir = match i {
                            0 => subchunk_coords[1] > camera_chunk_coords[1],
                            1 => subchunk_coords[1] < camera_chunk_coords[1],
                            2 => subchunk_coords[2] < camera_chunk_coords[2],
                            3 => subchunk_coords[2] > camera_chunk_coords[2],
                            4 => subchunk_coords[0] > camera_chunk_coords[0],
                            5 => subchunk_coords[0] < camera_chunk_coords[0],
                            6.. => unreachable!(),
                        };
                        if skip_face_dir {
                            continue;
                        }
                        // Base block faces
                        if subchunk.block_face_start_vertices[i] != u32::MAX {
                            block_face_draw_args.push(DrawIndirectArgs {
                                num_vertices: 4,
                                num_instances: subchunk.block_face_instance_groups[i].1,
                                start_vertex: subchunk.block_face_start_vertices[i],
                                start_instance: subchunk.block_face_instance_groups[i].0,
                            });
                        }
                        // Tinted block faces
                        if subchunk.tinted_block_face_start_vertices[i] != u32::MAX {
                            tinted_block_face_draw_args.push(DrawIndirectArgs {
                                num_vertices: 4,
                                num_instances: subchunk.tinted_block_face_instance_groups[i].1,
                                start_vertex: subchunk.tinted_block_face_start_vertices[i],
                                start_instance: subchunk.tinted_block_face_instance_groups[i].0,
                            });
                        }
                    }
                    // Custom blocks
                    for group in &subchunk.custom_block_groups {
                        custom_block_draw_args.push(DrawIndirectArgs {
                            num_vertices: group.start_face_and_len[1] * 6,
                            num_instances: group.start_instance_and_len[1],
                            start_vertex: group.start_face_and_len[0] * 6,
                            start_instance: group.start_instance_and_len[0],
                        });
                    }
                },
            );
            render_pass.set_bind_group(0, &self.common_bind_group, &[]);
            // Draw basic block faces.
            if !block_face_draw_args.is_empty() {
                block_face_draw_args_buffer =
                    self.resources
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Block Face Draw Args Buffer"),
                            contents: bytemuck::cast_slice(&block_face_draw_args),
                            usage: wgpu::BufferUsages::INDIRECT,
                        });
                render_pass.set_pipeline(&self.block_render_pipeline);
                render_pass
                    .set_vertex_buffer(0, self.subchunk_data_storage.block_face_vertex.get_slice());
                render_pass.set_vertex_buffer(
                    1,
                    self.subchunk_data_storage.block_face_instance.get_slice(),
                );
                render_pass.multi_draw_indirect(
                    &block_face_draw_args_buffer,
                    0,
                    block_face_draw_args.len().try_into().unwrap(),
                );
            }
            // Draw tinted block faces.
            if !tinted_block_face_draw_args.is_empty() {
                tinted_block_face_draw_args_buffer =
                    self.resources
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Tinted Block Face Draw Args Buffer"),
                            contents: bytemuck::cast_slice(&tinted_block_face_draw_args),
                            usage: wgpu::BufferUsages::INDIRECT,
                        });
                render_pass.set_pipeline(&self.tinted_block_render_pipeline);
                render_pass.set_vertex_buffer(
                    0,
                    self.subchunk_data_storage
                        .tinted_block_face_vertex
                        .get_slice(),
                );
                render_pass.set_vertex_buffer(
                    1,
                    self.subchunk_data_storage
                        .tinted_block_face_instance
                        .get_slice(),
                );
                render_pass.multi_draw_indirect(
                    &tinted_block_face_draw_args_buffer,
                    0,
                    tinted_block_face_draw_args.len().try_into().unwrap(),
                );
            }
            // Draw custom blocks.
            if !custom_block_draw_args.is_empty() {
                custom_block_draw_args_buffer =
                    self.resources
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Custom Block Draw Args Buffer"),
                            contents: bytemuck::cast_slice(&custom_block_draw_args),
                            usage: wgpu::BufferUsages::INDIRECT,
                        });
                render_pass.set_pipeline(&self.custom_block_render_pipeline);
                render_pass.set_vertex_buffer(
                    0,
                    self.subchunk_data_storage.custom_block_instance.get_slice(),
                );
                render_pass.multi_draw_indirect(
                    &custom_block_draw_args_buffer,
                    0,
                    custom_block_draw_args.len().try_into().unwrap(),
                );
            }
            // Render sky.
            {
                // Draw sun.
                render_pass.set_pipeline(&self.environment_state.sun_render_pipeline);
                render_pass.draw(0..4, 0..1);
                // Draw moon.
                render_pass.set_pipeline(&self.environment_state.moon_render_pipeline);
                render_pass.draw(0..4, 0..1);
                // Draw stars.
                render_pass.set_pipeline(&self.environment_state.star_render_pipeline);
                render_pass
                    .set_vertex_buffer(0, self.environment_state.star_quads_buffer.slice(..));
                render_pass.draw(0..4, 0..STAR_QUADS.len() as u32);
            }
            // Render debug graphics.
            if !debug_points.is_empty() {
                debug_point_buffer =
                    self.resources
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Debug Points Buffer"),
                            contents: bytemuck::cast_slice(debug_points),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                render_pass.set_pipeline(&self.debug_point_render_pipeline);
                render_pass.set_vertex_buffer(0, debug_point_buffer.slice(..));
                // Shader converts points to quads, so we need 4 vertices per instance.
                render_pass.draw(0..4, 0..debug_triangles.len().try_into().unwrap());
            }
            if !debug_lines.is_empty() {
                debug_line_buffer =
                    self.resources
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Debug lines Buffer"),
                            contents: bytemuck::cast_slice(debug_lines),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                render_pass.set_pipeline(&self.debug_line_render_pipeline);
                render_pass.set_vertex_buffer(0, debug_line_buffer.slice(..));
                // Shader converts lines to quads, so we need 4 vertices per instance.
                render_pass.draw(0..4, 0..debug_lines.len().try_into().unwrap());
            }
            if !debug_triangles.is_empty() {
                debug_triangle_buffer =
                    self.resources
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Debug triangles Buffer"),
                            contents: bytemuck::cast_slice(debug_triangles),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                render_pass.set_pipeline(&self.debug_triangle_render_pipeline);
                render_pass.set_vertex_buffer(0, debug_triangle_buffer.slice(..));
                render_pass.draw(0..4, 0..debug_triangles.len().try_into().unwrap());
            }
            // egui
            if let Some(egui_render_data) = egui_render_data {
                self.egui_renderer
                    .render(&mut render_pass, egui_render_data);
            }
        }
        self.resources
            .queue
            .submit(core::iter::once(encoder.finish()));
        self.resources.queue.present(output);
        if surface_suboptimal {
            self.resize(self.size);
        }
        self.egui_renderer
            .free_textures(&egui_full_output.textures_delta.free);
        Ok(Some(debug_output))
    }
}

#[derive(Debug)]
pub struct Texture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl Texture {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    #[tracing::instrument(skip(device, config))]
    pub fn create_depth_texture(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        label: &str,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let texture = device.create_texture(&desc);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            lod_min_clamp: 0.0,
            lod_max_clamp: 100.0,
            ..Default::default()
        });
        Self {
            texture,
            view,
            sampler,
        }
    }
}

#[derive(Debug)]
pub struct HypercubedWgpuTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
}

impl HypercubedWgpuTexture {
    pub fn create_from_resource_atlas(
        atlas: &resources::texture::Atlas,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: Option<&str>,
    ) -> Self {
        Self::create_from_raw(
            &atlas.texture_bytes,
            atlas.width,
            atlas.height,
            wgpu::TextureFormat::Rgba8Unorm,
            device,
            queue,
            label,
        )
    }

    pub fn create_from_resource_texture(
        atlas: &resources::texture::RawTexture,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: Option<&str>,
    ) -> Self {
        Self::create_from_raw(
            &atlas.texture_bytes,
            atlas.width,
            atlas.height,
            wgpu::TextureFormat::Rgba8Unorm,
            device,
            queue,
            label,
        )
    }

    pub fn create_from_raw(
        texture_bytes: &[u8],
        width: u32,
        height: u32,
        texture_format: wgpu::TextureFormat,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: Option<&str>,
    ) -> Self {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: texture_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfoBase {
                aspect: wgpu::TextureAspect::All,
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            texture_bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            size,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self { texture, view }
    }
}
