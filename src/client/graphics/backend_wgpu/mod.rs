pub mod chunk;
pub mod debug;
pub mod egui_renderer;

use super::{Camera, DebugState};
use crate::client::{MIN_HEIGHT_I32, SUBCHUNK_AXIS_LEN_I32};
use ahash::{AHashMap, AHashSet};
use chunk::{
    block_face::{BlockFaceInstanceBufferManager, BlockFaceVertexBufferManager},
    custom_block::CustomBlockInstanceBufferManager,
    tinted_block_face::{TintedBlockFaceInstanceBufferManager, TintedBlockFaceVertexBufferManager},
};
use debug::line::Instance as DebugLineInstance;
use debug::point::Vertex as DebugPointVertex;
use debug::triangle::Instance as DebugTriangleInstance;
use nalgebra::{Perspective3, Point3};
use resources::block::model::{ModelRegistry, Tint};
use std::sync::Arc;
use wgpu::util::DeviceExt as _;
use winit::window::Window;

pub struct GraphicsResources {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub block_registry: resources::block::Registry,
    pub model_registry: ModelRegistry,
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

impl GraphicsOptions {
    pub fn get_present_mode(&self) -> wgpu::PresentMode {
        match self.vsync {
            true => wgpu::PresentMode::Fifo,
            false => wgpu::PresentMode::Immediate,
        }
    }
}

pub struct GraphicsBufferManagers {
    pub block_face_vertex: BlockFaceVertexBufferManager,
    pub block_face_instance: BlockFaceInstanceBufferManager,
    pub tinted_block_face_vertex: TintedBlockFaceVertexBufferManager,
    pub tinted_block_face_instance: TintedBlockFaceInstanceBufferManager,
    pub custom_block_instance: CustomBlockInstanceBufferManager,
}

pub struct GraphicsState {
    pub resources: Arc<GraphicsResources>,
    pub buffer_managers: GraphicsBufferManagers,
    pub config: wgpu::SurfaceConfiguration,
    pub graphics_options: GraphicsOptions,
    pub block_render_pipeline: wgpu::RenderPipeline,
    pub tinted_block_render_pipeline: wgpu::RenderPipeline,
    pub custom_block_render_pipeline: wgpu::RenderPipeline,
    pub debug_point_render_pipeline: wgpu::RenderPipeline,
    pub debug_line_render_pipeline: wgpu::RenderPipeline,
    pub debug_triangle_render_pipeline: wgpu::RenderPipeline,
    pub debug_crosshair_render_pipeline: wgpu::RenderPipeline,
    pub egui_renderer: egui_renderer::Renderer,
    pub depth_texture: Texture,
    pub custom_block_faces_buffer: wgpu::Buffer,
    pub block_item_atlas: TextureAtlas,
    pub block_item_atlas_bind_group: wgpu::BindGroup,
    pub camera: Camera,
    pub view_info_buffer: wgpu::Buffer,
    pub view_info_bind_group: wgpu::BindGroup,
    pub view_info_bind_group_layout: wgpu::BindGroupLayout,
    pub matrices_bind_group: wgpu::BindGroup,
    pub custom_block_faces_bind_group: wgpu::BindGroup,
    pub debug_crosshair_view_info_buffer: wgpu::Buffer,
    pub debug_crosshair_view_info_bind_group: wgpu::BindGroup,
    pub debug_crosshair_vertex_buffer: wgpu::Buffer,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub clear_color: wgpu::Color,
}

pub struct DebugOutput {
    pub subchunks_culled: usize,
    pub subchunk_traversal_graph: Vec<([i32; 3], [i32; 3])>,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ViewInfo {
    view_matrix: [[f32; 4]; 4],
    screen_size: [u32; 2],
    padding: [u32; 2],
}

impl ViewInfo {
    pub fn new(camera: &Camera, screen_size: winit::dpi::PhysicalSize<u32>) -> Self {
        Self {
            view_matrix: camera.generate_reversed_depth_view_matrix_slice(),
            screen_size: screen_size.into(),
            padding: [0; 2],
        }
    }

    pub fn new_debug_crosshair(camera: &Camera) -> Self {
        Self {
            view_matrix: camera.generate_debug_crosshair_view_matrix_slice(),
            screen_size: [0; 2],
            padding: [0; 2],
        }
    }
}

impl GraphicsState {
    #[tracing::instrument(skip_all)]
    pub fn new<F>(window: Arc<Window>, register_blocks: F) -> anyhow::Result<Self>
    where
        F: FnOnce(
            &mut resources::block::Registry,
            &mut resources::block::model::ModelRegistryBuilder,
            &mut resources::texture::AtlasBuilder,
        ) -> anyhow::Result<()>,
    {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::from_env_or_default(),
        });
        let surface = instance.create_surface(window)?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .unwrap();
        dbg!(adapter.get_info());
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
        //       seem to be correct? Just fix it up here, for now.
        #[cfg(target_os = "windows")]
        let surface_format = surface_format.remove_srgb_suffix();
        let graphics_options = GraphicsOptions::default();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: graphics_options.get_present_mode(),
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
        };
        surface.configure(&device, &config);
        let view_info_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("View Info Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let matrices_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Matrices Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let custom_block_faces_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Custom Block Faces Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let atlas_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Atlas Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let (
            block_item_texture_atlas,
            block_item_atlas_size,
            custom_block_faces_buffer,
            block_registry,
            model_registry,
        ) = {
            use resources;
            let size = [1024; 2];
            let square_length = 16;
            let mut atlas_builder =
                resources::texture::AtlasBuilder::new(size[0], size[1], square_length);
            let mut model_registry_builder = resources::block::model::ModelRegistryBuilder::new();
            let mut block_registry = resources::block::Registry::new();
            register_blocks(
                &mut block_registry,
                &mut model_registry_builder,
                &mut atlas_builder,
            )?;
            let atlas = TextureAtlas::from_builder(
                atlas_builder,
                &device,
                &queue,
                Some("Block and Item Atlas"),
            );
            let model_registry = model_registry_builder.finish();
            let custom_block_faces: Vec<_> = model_registry
                .custom_block_faces
                .iter()
                .map(|face| {
                    face.map(|v| chunk::custom_block::Vertex {
                        pos: *v.local_pos.coords.as_ref(),
                        uvs: v.uvs,
                        normal: *v.normal.as_ref(),
                        tint_percentage: matches!(v.tint, Some(Tint::Biome)) as u8 as f32,
                    })
                })
                .collect();
            let custom_block_faces_buffer =
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Custom Block Vertices Buffer"),
                    contents: bytemuck::cast_slice(&custom_block_faces),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
                });
            (
                atlas,
                size,
                custom_block_faces_buffer,
                block_registry,
                model_registry,
            )
        };
        let block_item_atlas_size_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Block and Item Atlas Size Buffer"),
                contents: bytemuck::cast_slice(&block_item_atlas_size.map(|x| x as f32)),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let block_item_atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("block_item_atlas_bind_group"),
            layout: &atlas_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: block_item_atlas_size_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&block_item_texture_atlas.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&block_item_texture_atlas.sampler),
                },
            ],
        });
        let depth_texture = Texture::create_depth_texture(&device, &config, "depth_texture");
        // Block pipelines
        let generic_block_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Chunk Block Pipeline Layout"),
                bind_group_layouts: &[
                    &view_info_bind_group_layout,
                    &atlas_bind_group_layout,
                    &matrices_bind_group_layout,
                    &custom_block_faces_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });
        let block_render_pipeline = chunk::block_face::create_render_pipeline(
            &device,
            &config,
            &generic_block_pipeline_layout,
        );
        let tinted_block_render_pipeline = chunk::tinted_block_face::create_render_pipeline(
            &device,
            &config,
            &generic_block_pipeline_layout,
        );
        let custom_block_render_pipeline = chunk::custom_block::create_render_pipeline(
            &device,
            &config,
            &generic_block_pipeline_layout,
        );
        // Debug pipelines
        let debug_graphics_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Debug Graphics Render Pipeline Layout"),
                bind_group_layouts: &[&view_info_bind_group_layout],
                push_constant_ranges: &[],
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
        let debug_crosshair_render_pipeline = debug::crosshair::create_render_pipeline(
            &device,
            &config,
            &debug_graphics_pipeline_layout,
        );
        // egui renderer
        let egui_renderer = egui_renderer::Renderer::new(&device, &config);
        // Buffer managers
        let block_face_vertex_buffer_manager = BlockFaceVertexBufferManager::new(&device);
        let block_face_instance_buffer_manager = BlockFaceInstanceBufferManager::new(&device);
        let tinted_block_face_vertex_buffer_manager =
            TintedBlockFaceVertexBufferManager::new(&device);
        let tinted_block_face_instance_buffer_manager =
            TintedBlockFaceInstanceBufferManager::new(&device);
        let custom_block_instance_buffer_manager = CustomBlockInstanceBufferManager::new(&device);
        // Buffers
        let proj_matrix = Perspective3::new(
            (size.width as f32) / (size.height as f32),
            f32::to_radians(super::DEFAULT_FOV),
            super::DEFAULT_ZNEAR,
            super::DEFAULT_ZFAR,
        );
        let camera = Camera {
            pos: Point3::new(0.0, 124.0, 0.0),
            proj_matrix,
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
        };
        let view_info_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("View Info Buffer"),
            contents: bytemuck::cast_slice(&[ViewInfo::new(&camera, size)]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let view_info_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("View Info Bind Group"),
            layout: &view_info_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: view_info_buffer.as_entire_binding(),
            }],
        });
        let face_matrices_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Face Matrices Buffer"),
            contents: bytemuck::cast_slice(&chunk::block_face::face_matrices::generate_array()),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let matrices_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Matrices Bind Group"),
            layout: &matrices_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: face_matrices_buffer.as_entire_binding(),
            }],
        });
        let custom_block_faces_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Custom Block Faces Bind Group"),
            layout: &custom_block_faces_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: custom_block_faces_buffer.as_entire_binding(),
            }],
        });
        // Debug buffers
        let debug_crosshair_view_info_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Debug Crosshair View Info Buffer"),
                contents: bytemuck::cast_slice(&[ViewInfo::new_debug_crosshair(&camera)]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let debug_crosshair_view_info_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Debug Crosshair View Info Bind Group"),
                layout: &view_info_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: debug_crosshair_view_info_buffer.as_entire_binding(),
                }],
            });
        let debug_crosshair_vertex_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Debug Crosshair Vertex Buffer"),
                contents: bytemuck::cast_slice(debug::crosshair::VERTICES),
                usage: wgpu::BufferUsages::VERTEX,
            });
        Ok(Self {
            resources: Arc::new(GraphicsResources {
                surface,
                device,
                queue,
                block_registry,
                model_registry,
            }),
            buffer_managers: GraphicsBufferManagers {
                block_face_vertex: block_face_vertex_buffer_manager,
                block_face_instance: block_face_instance_buffer_manager,
                tinted_block_face_vertex: tinted_block_face_vertex_buffer_manager,
                tinted_block_face_instance: tinted_block_face_instance_buffer_manager,
                custom_block_instance: custom_block_instance_buffer_manager,
            },
            config,
            graphics_options,
            block_render_pipeline,
            tinted_block_render_pipeline,
            custom_block_render_pipeline,
            debug_point_render_pipeline,
            debug_line_render_pipeline,
            debug_triangle_render_pipeline,
            debug_crosshair_render_pipeline,
            egui_renderer,
            depth_texture,
            custom_block_faces_buffer,
            block_item_atlas: block_item_texture_atlas,
            block_item_atlas_bind_group,
            camera,
            view_info_buffer,
            view_info_bind_group,
            view_info_bind_group_layout,
            matrices_bind_group,
            custom_block_faces_bind_group,
            debug_crosshair_view_info_buffer,
            debug_crosshair_view_info_bind_group,
            debug_crosshair_vertex_buffer,
            size,
            // Minecraft plains biome sky color
            clear_color: wgpu::Color {
                r: 0.471,
                g: 0.655,
                b: 1.0,
                a: 1.0,
            },
        })
    }

    #[tracing::instrument(skip(self))]
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
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
            self.camera
                .proj_matrix
                .set_aspect((new_size.width as f32) / (new_size.height as f32));
        }
    }

    pub fn apply_new_graphics_options(&mut self, new_options: GraphicsOptions) {
        if new_options.vsync != self.graphics_options.vsync {
            self.config.present_mode = new_options.get_present_mode();
            self.resources
                .surface
                .configure(&self.resources.device, &self.config);
        }
        self.graphics_options = new_options;
    }

    pub fn free_subchunk_data(&mut self, subchunk_coords: [i32; 3]) {
        let buffer_managers = &mut self.buffer_managers;
        buffer_managers
            .block_face_vertex
            .free_subchunk_areas(subchunk_coords);
        buffer_managers
            .block_face_instance
            .free_subchunk_areas(subchunk_coords);
        buffer_managers
            .tinted_block_face_vertex
            .free_subchunk_areas(subchunk_coords);
        buffer_managers
            .tinted_block_face_instance
            .free_subchunk_areas(subchunk_coords);
        buffer_managers
            .custom_block_instance
            .free_subchunk_areas(subchunk_coords);
    }

    #[tracing::instrument(skip_all)]
    pub fn render(
        &mut self,
        subchunks: &AHashMap<[i32; 3], chunk::Subchunk>,
        loaded_chunks: &AHashSet<[i32; 2]>,
        egui_ctx: &egui::Context,
        egui_full_output: egui::output::FullOutput,
        debug_state: &DebugState,
        debug_points: &[DebugPointVertex],
        debug_lines: &[DebugLineInstance],
        debug_triangles: &[DebugTriangleInstance],
    ) -> Result<DebugOutput, wgpu::SurfaceError> {
        let pixels_per_point = egui_full_output.pixels_per_point;
        let egui_primitives = egui_ctx.tessellate(egui_full_output.shapes, pixels_per_point);
        self.resources.queue.write_buffer(
            &self.view_info_buffer,
            0,
            bytemuck::cast_slice(&[ViewInfo::new(&self.camera, self.size)]),
        );
        self.resources.queue.write_buffer(
            &self.debug_crosshair_view_info_buffer,
            0,
            bytemuck::cast_slice(&self.camera.generate_debug_crosshair_view_matrix_slice()),
        );
        let egui_render_data = self.egui_renderer.prepare(
            &self.resources,
            &self.size,
            egui_full_output.textures_delta.set,
            egui_primitives,
            pixels_per_point,
        );
        let output = self.resources.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder =
            self.resources
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });
        // Main render pass
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
                        load: wgpu::LoadOp::Clear(self.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        // TODO: Check this, was previously set to `Store`
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            let camera_chunk_coords = {
                let camera_pos = self.camera.pos;
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
                &self.camera,
                subchunks,
                loaded_chunks,
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
            render_pass.set_bind_group(0, &self.view_info_bind_group, &[]);
            render_pass.set_bind_group(1, &self.block_item_atlas_bind_group, &[]);
            render_pass.set_bind_group(2, &self.matrices_bind_group, &[]);
            render_pass.set_bind_group(3, &self.custom_block_faces_bind_group, &[]);
            // Base block faces
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
                    .set_vertex_buffer(0, self.buffer_managers.block_face_vertex.get_slice());
                render_pass
                    .set_vertex_buffer(1, self.buffer_managers.block_face_instance.get_slice());
                render_pass.multi_draw_indirect(
                    &block_face_draw_args_buffer,
                    0,
                    block_face_draw_args.len().try_into().unwrap(),
                );
            }
            // Tinted block faces
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
                    self.buffer_managers.tinted_block_face_vertex.get_slice(),
                );
                render_pass.set_vertex_buffer(
                    1,
                    self.buffer_managers.tinted_block_face_instance.get_slice(),
                );
                render_pass.multi_draw_indirect(
                    &tinted_block_face_draw_args_buffer,
                    0,
                    tinted_block_face_draw_args.len().try_into().unwrap(),
                );
            }
            // Custom blocks
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
                render_pass
                    .set_vertex_buffer(0, self.buffer_managers.custom_block_instance.get_slice());
                render_pass.multi_draw_indirect(
                    &custom_block_draw_args_buffer,
                    0,
                    custom_block_draw_args.len().try_into().unwrap(),
                );
            }
            // Debug graphics
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
            // Debug crosshair
            {
                render_pass.set_pipeline(&self.debug_crosshair_render_pipeline);
                render_pass.set_bind_group(0, &self.debug_crosshair_view_info_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.debug_crosshair_vertex_buffer.slice(..));
                render_pass.draw(0..6, 0..1);
            }
            // egui
            if let Some(egui_render_data) = egui_render_data {
                self.egui_renderer
                    .render(&mut render_pass, egui_render_data);
            }
        }
        self.resources
            .queue
            .submit(std::iter::once(encoder.finish()));
        output.present();
        self.egui_renderer
            .free_textures(&egui_full_output.textures_delta.free);
        Ok(debug_output)
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
            mipmap_filter: wgpu::FilterMode::Nearest,
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
pub struct TextureAtlas {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl TextureAtlas {
    pub fn from_builder(
        builder: resources::texture::AtlasBuilder,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: Option<&str>,
    ) -> Self {
        let (width, height) = (builder.texture.width(), builder.texture.height());
        let bytes = builder.texture.into_vec();
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
            format: wgpu::TextureFormat::Rgba8Unorm,
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
            &bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            size,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
        Self {
            texture,
            view,
            sampler,
        }
    }
}
