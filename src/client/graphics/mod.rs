pub mod chunk;
pub mod chunk_rc;
pub mod debug;
pub mod egui_renderer;

use crate::basic_types::AxisDirection;
use crate::resource;
use ahash::{AHashMap, AHashSet};
use nalgebra::{Isometry3, Matrix4, Perspective3, Point3, UnitQuaternion, Vector3};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use wgpu::util::DeviceExt as _;
use winit::window::Window;
//use petgraph::graph::DiGraph;
use super::{MIN_HEIGHT_I32, SUBCHUNK_AXIS_LEN_I32};
use chunk_rc::{
    block_face::{BlockFaceInstanceBufferManager, BlockFaceVertexBufferManager},
    custom_block::CustomBlockInstanceBufferManager,
    tinted_block_face::{TintedBlockFaceInstanceBufferManager, TintedBlockFaceVertexBufferManager},
};

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub pos: Point3<f32>,
    pub proj_matrix: Perspective3<f32>,
    /// Represented in degrees.
    pub yaw: f32,
    /// Represented in degrees.
    pub pitch: f32,
    /// Represented in degrees.
    pub roll: f32,
}

impl Camera {
    pub fn get_rot(&self) -> UnitQuaternion<f32> {
        UnitQuaternion::from_euler_angles(
            self.pitch.to_radians(),
            -self.yaw.to_radians(),
            -self.roll.to_radians(),
        )
    }

    pub fn generate_view_matrix(&self) -> Matrix4<f32> {
        let translate = Isometry3::new(self.pos.coords, nalgebra::zero())
            .inverse()
            .to_matrix();
        let rotate = self.get_rot().inverse().to_homogeneous();
        self.proj_matrix.as_matrix() * rotate * translate
    }

    pub fn generate_reversed_depth_view_matrix(&self) -> Matrix4<f32> {
        // Using a standard depth buffer had issues with Z-fighting on faraway objects (snow
        // clipping through spruce leaves from high enough up was a particularly bad case).
        Matrix4::new_nonuniform_scaling(&Vector3::new(1.0, 1.0, -0.5))
            .append_translation(&Vector3::new(0.0, 0.0, 0.5))
            * self.generate_view_matrix()
    }

    pub fn generate_reversed_depth_view_matrix_slice(&self) -> [[f32; 4]; 4] {
        *self.generate_reversed_depth_view_matrix().as_ref()
    }

    pub fn generate_debug_crosshair_view_matrix_slice(&self) -> [[f32; 4]; 4] {
        let fake_pos = Point3::from(self.get_rot() * Vector3::z().scale(30.0));
        let up = self.get_rot() * Vector3::y();
        let look_at = Matrix4::look_at_rh(&fake_pos, &Point3::origin(), &up);
        let view_matrix = self.proj_matrix.as_matrix() * look_at;
        *view_matrix.as_ref()
    }

    /// Generates a normal and offset for each view clipping plane.
    /// Planes are in order of left, right, bottom, top, near, far.
    pub fn generate_clipping_planes(&self) -> [(Vector3<f32>, f32); 6] {
        /// Converts constants from a plane equation to a normal vector and offset
        fn convert_abcd(a: f32, b: f32, c: f32, d: f32) -> (Vector3<f32>, f32) {
            let normal = Vector3::new(a, b, c);
            let normal_len = normal.magnitude();
            (normal / normal_len, d / normal_len)
        }
        let m = self.generate_view_matrix();
        [
            // Left
            convert_abcd(m.m41 + m.m11, m.m42 + m.m12, m.m43 + m.m13, m.m44 + m.m14),
            // Right
            convert_abcd(m.m41 - m.m11, m.m42 - m.m12, m.m43 - m.m13, m.m44 - m.m14),
            // Bottom
            convert_abcd(m.m41 + m.m21, m.m42 + m.m22, m.m43 + m.m23, m.m44 + m.m24),
            // Top
            convert_abcd(m.m41 - m.m21, m.m42 - m.m22, m.m43 - m.m23, m.m44 - m.m24),
            // Near
            convert_abcd(m.m41 + m.m31, m.m42 + m.m32, m.m43 + m.m33, m.m44 + m.m34),
            // Far
            convert_abcd(m.m41 - m.m31, m.m42 - m.m32, m.m43 - m.m33, m.m44 - m.m34),
        ]
    }
}

pub struct GraphicsResources {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    // TODO: Change to `global_palette`
    pub block_registry: resource::block::Registry,
}

#[derive(Clone, Copy, Debug)]
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

// XXX: DEBUG
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RadianceProbeDebugInfo {
    pub debug_floats: [f32; 4],
    pub debug_ints: [u32; 4],
}

pub struct GraphicsState {
    pub resources: Arc<GraphicsResources>,
    pub buffer_managers: GraphicsBufferManagers,
    pub config: wgpu::SurfaceConfiguration,
    pub graphics_options: GraphicsOptions,
    pub radiance_probe_update_pipelines: [wgpu::ComputePipeline; 2],
    pub radiance_probe_debug_pipeline: wgpu::ComputePipeline,
    pub radiance_probe_info_bind_group_layout: wgpu::BindGroupLayout,
    pub radiance_probe_debug_texture_storage_bind_group: wgpu::BindGroup,
    pub radiance_probe_debug_egui_texture: egui::load::SizedTexture,
    pub radiance_probe_debug_info_buffer: Arc<wgpu::Buffer>,
    pub radiance_probe_debug_info: Arc<Mutex<RadianceProbeDebugInfo>>,
    pub radiance_probe_lightmap_bind_group_layout: wgpu::BindGroupLayout,
    // TODO:
    // - Render to an HDR texture
    // - Implement an HDR pipeline for tonemapping to sRGB
    pub block_render_pipeline: wgpu::RenderPipeline,
    pub tinted_block_render_pipeline: wgpu::RenderPipeline,
    pub custom_block_render_pipeline: wgpu::RenderPipeline,
    pub debug_crosshair_render_pipeline: wgpu::RenderPipeline,
    pub egui_renderer: egui_renderer::Renderer,
    pub depth_texture: Texture,
    pub custom_block_vertices_buffer: wgpu::Buffer,
    pub custom_block_indices_buffer: wgpu::Buffer,
    pub block_item_atlas: resource::texture::Atlas,
    pub block_item_atlas_bind_group: wgpu::BindGroup,
    pub camera: Camera,
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group: wgpu::BindGroup,
    pub camera_bind_group_layout: wgpu::BindGroupLayout,
    pub matrices_bind_group: wgpu::BindGroup,
    pub debug_crosshair_camera_buffer: wgpu::Buffer,
    pub debug_crosshair_camera_bind_group: wgpu::BindGroup,
    pub debug_crosshair_vertex_buffer: wgpu::Buffer,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub clear_color: wgpu::Color,
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
        }
    }
}

pub struct DebugOutput {
    pub subchunks_culled: usize,
    pub subchunk_traversal_graph: Vec<([i32; 3], [i32; 3])>,
}

impl GraphicsState {
    pub const DEFAULT_FOV: f32 = 80.0;
    pub const DEFAULT_ZNEAR: f32 = 0.01;
    pub const DEFAULT_ZFAR: f32 = 1024.0;

    #[tracing::instrument(skip_all)]
    pub async fn new<F>(window: &'static Window, register_blocks: F) -> anyhow::Result<Self>
    where
        F: FnOnce(
            &mut resource::block::Registry,
            &mut resource::block::model::ModelCache,
            &mut resource::texture::AtlasBuilder,
        ) -> anyhow::Result<()>,
    {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(Default::default());
        let surface = instance.create_surface(window)?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();
        dbg!(adapter.get_info());
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::MULTI_DRAW_INDIRECT
                        | wgpu::Features::INDIRECT_FIRST_INSTANCE
                        // XXX: DEBUG
                        | wgpu::Features::POLYGON_MODE_LINE
                        | wgpu::Features::MAPPABLE_PRIMARY_BUFFERS
                        // NOTE: RADIANCE CASCADES
                        | wgpu::Features::PUSH_CONSTANTS,
                    required_limits: wgpu::Limits {
                        // max_push_constant_size: 24,
                        // XXX: DEBUG
                        max_push_constant_size: 64,
                        // NOTE: RADIANCE CASCADES
                        max_storage_buffers_per_shader_stage: 10,
                        max_storage_buffer_binding_size: 1024 * 1024 * 1024,
                        max_buffer_size: 1024 * 1024 * 1024,
                        ..Default::default()
                    },
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .unwrap();
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
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
        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("camera_bind_group_layout"),
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
                label: Some("matrices_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let atlas_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("atlas_bind_group_layout"),
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
                        visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
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
        let lightmap_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Light Map Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let (
            block_item_texture_atlas,
            block_item_atlas_size,
            block_registry,
            custom_block_vertices_buffer,
            custom_block_indices_buffer,
        ) = {
            use crate::resource;
            let size = [1024; 2];
            let square_length = 16;
            let mut atlas_builder =
                resource::texture::AtlasBuilder::new(size[0], size[1], square_length);
            let mut model_cache = resource::block::model::ModelCache::new();
            let mut block_registry = resource::block::Registry::new();
            register_blocks(&mut block_registry, &mut model_cache, &mut atlas_builder)?;
            let atlas = atlas_builder.build(&device, &queue, Some("Block and Item Atlas"));
            let custom_block_vertices_buffer =
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Custom Block Vertices Buffer"),
                    contents: bytemuck::cast_slice(&model_cache.custom_block_vertices),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
                });
            let custom_block_indices_buffer =
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Custom Block Indices Buffer"),
                    contents: bytemuck::cast_slice(&model_cache.custom_block_indices),
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::STORAGE,
                });
            (
                atlas,
                size,
                block_registry,
                custom_block_vertices_buffer,
                custom_block_indices_buffer,
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
                    &camera_bind_group_layout,
                    &atlas_bind_group_layout,
                    &matrices_bind_group_layout,
                    &lightmap_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });
        let block_render_pipeline = chunk_rc::block_face::create_render_pipeline(
            &device,
            &config,
            &generic_block_pipeline_layout,
        );
        let tinted_block_render_pipeline = chunk_rc::tinted_block_face::create_render_pipeline(
            &device,
            &config,
            &generic_block_pipeline_layout,
        );
        let custom_block_render_pipeline = chunk_rc::custom_block::create_render_pipeline(
            &device,
            &config,
            &generic_block_pipeline_layout,
        );
        let radiance_probe_info_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Radiance Probe Info Bind Group Layout"),
                entries: &[
                    // Subchunk Hash Map
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Block Face Instances
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Tinted Block Face Instances
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Custom Block Vertices
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Custom Block Indices
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Custom Block Instances
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Custom Block Groups
                    wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Update Info Buffer
                    wgpu::BindGroupLayoutEntry {
                        binding: 7,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Output Lightmap Buffer
                    wgpu::BindGroupLayoutEntry {
                        binding: 8,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Previous Cascade Buffer
                    wgpu::BindGroupLayoutEntry {
                        binding: 9,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Block and Item Luma Atlas Texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 10,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                ],
            });
        let radiance_probe_debug_texture_storage_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Debug Radiance Probe Texture Storage Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                }],
            });
        let radiance_probe_update_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Radiance Probe Update Pipeline Layout"),
                bind_group_layouts: &[
                    &atlas_bind_group_layout,
                    &matrices_bind_group_layout,
                    &radiance_probe_info_bind_group_layout,
                    &radiance_probe_debug_texture_storage_layout,
                ],
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStages::COMPUTE,
                    // range: 0..24,
                    // XXX: DEBUG
                    range: 0..64,
                }],
            });
        let radiance_probe_update_pipelines = chunk_rc::compute::create_cascade_update_pipelines(
            &device,
            &radiance_probe_update_pipeline_layout,
        );
        let radiance_probe_debug_pipeline = chunk_rc::compute::create_raytracing_debug_pipeline(
            &device,
            &radiance_probe_update_pipeline_layout,
        );
        // XXX: DEBUG
        let radiance_probe_debug_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Radiance Probe Debug Texture"),
            size: wgpu::Extent3d {
                width: 960,
                height: 540,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });
        let radiance_probe_debug_texture_storage_bind_group = {
            let view =
                radiance_probe_debug_texture.create_view(&wgpu::TextureViewDescriptor::default());
            let storage_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Debug Radiance Probe Texture Storage Bind Group"),
                layout: &radiance_probe_debug_texture_storage_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                }],
            });
            storage_bind_group
        };
        let radiance_probe_debug_info_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Radiance Probe Debug Info Buffer"),
                contents: bytemuck::cast_slice(&[RadianceProbeDebugInfo::default()]),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::MAP_READ,
            });
        let radiance_probe_debug_info_buffer = Arc::new(radiance_probe_debug_info_buffer);
        // Debug information pipelines
        let debug_crosshair_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Debug Crosshair Render Pipeline Layout"),
                bind_group_layouts: &[&camera_bind_group_layout],
                push_constant_ranges: &[],
            });
        let debug_crosshair_render_pipeline = debug::crosshair::create_render_pipeline(
            &device,
            &config,
            &debug_crosshair_pipeline_layout,
        );
        // egui renderer
        let mut egui_renderer = egui_renderer::Renderer::new(&device, &config);
        // XXX: DEBUG
        let radiance_probe_debug_egui_texture_id = egui_renderer.register_user_texture(
            &device,
            radiance_probe_debug_texture,
            egui::TextureOptions::NEAREST,
        );
        // Buffer managers
        let block_face_vertex_buffer_manager = BlockFaceVertexBufferManager::new(&device);
        let block_face_instance_buffer_manager =
            BlockFaceInstanceBufferManager::new(&device, "Block Face");
        let tinted_block_face_vertex_buffer_manager =
            TintedBlockFaceVertexBufferManager::new(&device);
        let tinted_block_face_instance_buffer_manager =
            TintedBlockFaceInstanceBufferManager::new(&device, "Tinted Block Face");
        let custom_block_instance_buffer_manager = CustomBlockInstanceBufferManager::new(&device);
        // Buffers
        let proj_matrix = Perspective3::new(
            (size.width as f32) / (size.height as f32),
            f32::to_radians(GraphicsState::DEFAULT_FOV),
            GraphicsState::DEFAULT_ZNEAR,
            GraphicsState::DEFAULT_ZFAR,
        );
        let camera = Camera {
            pos: Point3::new(0.0, 124.0, 0.0),
            proj_matrix,
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
        };
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&camera.generate_reversed_depth_view_matrix_slice()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera_bind_group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });
        let face_matrices_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Face Matrices Buffer"),
            contents: bytemuck::cast_slice(&chunk_rc::block_face::face_matrices::generate_array()),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let matrices_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("matrices_bind_group"),
            layout: &matrices_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: face_matrices_buffer.as_entire_binding(),
            }],
        });
        // Debug buffers
        let debug_crosshair_camera_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Debug Crosshair Camera Buffer"),
                contents: bytemuck::cast_slice(
                    &camera.generate_debug_crosshair_view_matrix_slice(),
                ),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let debug_crosshair_camera_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Debug Crosshair Camera Bind Group"),
                layout: &camera_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: debug_crosshair_camera_buffer.as_entire_binding(),
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
            radiance_probe_debug_pipeline,
            radiance_probe_update_pipelines,
            radiance_probe_info_bind_group_layout,
            radiance_probe_debug_texture_storage_bind_group,
            radiance_probe_debug_egui_texture: egui::load::SizedTexture {
                id: radiance_probe_debug_egui_texture_id,
                size: (960.0, 540.0).into(),
            },
            radiance_probe_debug_info_buffer,
            radiance_probe_debug_info: Arc::new(Mutex::new(RadianceProbeDebugInfo::default())),
            radiance_probe_lightmap_bind_group_layout: lightmap_bind_group_layout,
            block_render_pipeline,
            tinted_block_render_pipeline,
            custom_block_render_pipeline,
            debug_crosshair_render_pipeline,
            egui_renderer,
            depth_texture,
            custom_block_vertices_buffer,
            custom_block_indices_buffer,
            block_item_atlas: block_item_texture_atlas,
            block_item_atlas_bind_group,
            camera,
            camera_buffer,
            camera_bind_group,
            camera_bind_group_layout,
            matrices_bind_group,
            debug_crosshair_camera_buffer,
            debug_crosshair_camera_bind_group,
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

    #[tracing::instrument(skip_all)]
    pub fn render(
        &mut self,
        subchunks: &AHashMap<[i32; 3], chunk_rc::Subchunk>,
        loaded_chunks: &AHashSet<[i32; 2]>,
        egui_ctx: &egui::Context,
        egui_full_output: egui::output::FullOutput,
        debug_state: &DebugState,
    ) -> Result<DebugOutput, wgpu::SurfaceError> {
        let pixels_per_point = egui_full_output.pixels_per_point;
        let egui_primitives = egui_ctx.tessellate(egui_full_output.shapes, pixels_per_point);
        self.resources.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&self.camera.generate_reversed_depth_view_matrix_slice()),
        );
        self.resources.queue.write_buffer(
            &self.debug_crosshair_camera_buffer,
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
        let subchunks_skipped;
        let mut subchunk_traversal_graph: Vec<([i32; 3], [i32; 3])> = Vec::new();
        let block_face_draw_args_buffer;
        let tinted_block_face_draw_args_buffer;
        let custom_block_draw_args_buffer;
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            let camera_clipping_planes = self.camera.generate_clipping_planes();
            // let camera_clipping_planes = debug_state.cull_camera.generate_clipping_planes();
            let mut rendered_chunks: AHashSet<[i32; 3]> = AHashSet::new();
            let mut visited_chunks: AHashSet<[i32; 3]> = AHashSet::new();
            #[derive(Clone, Copy, Debug)]
            struct QueuedChunk {
                pub coords: [i32; 3],
                pub from_dir: Option<AxisDirection>,
                pub back_travel_amount: f32,
                pub flipping_state: FlippingState,
            }
            // TODO: Come up with a better name for this, document how it works
            #[derive(Clone, Copy, Debug, PartialEq, Eq)]
            enum FlippingState {
                Unflipped {
                    x_positive: Option<bool>,
                    y_positive: Option<bool>,
                    z_positive: Option<bool>,
                },
                Flipped,
            }
            //let mut subchunk_graph: DiGraph<QueuedChunk, ()> = DiGraph::new();
            let mut chunk_queue: VecDeque<QueuedChunk> = VecDeque::new();
            let camera_chunk_coords = {
                // let camera_pos = debug_state.cull_camera.pos;
                let camera_pos = self.camera.pos;
                let camera_x = (camera_pos.x.floor() as i32).div_euclid(SUBCHUNK_AXIS_LEN_I32);
                let camera_y = (camera_pos.y.floor() as i32 - MIN_HEIGHT_I32)
                    .div_euclid(SUBCHUNK_AXIS_LEN_I32);
                let camera_z = (camera_pos.z.floor() as i32).div_euclid(SUBCHUNK_AXIS_LEN_I32);
                let camera_chunk_coords = [camera_x, camera_y, camera_z];
                chunk_queue.push_back(QueuedChunk {
                    coords: camera_chunk_coords,
                    from_dir: None,
                    back_travel_amount: 0.0,
                    flipping_state: FlippingState::Unflipped {
                        x_positive: None,
                        y_positive: None,
                        z_positive: None,
                    },
                });
                visited_chunks.insert(camera_chunk_coords);
                camera_chunk_coords
            };
            let mut num_subchunks_rendered = 0;
            #[repr(C)]
            #[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
            pub struct DrawIndirectArgs {
                pub num_vertices: u32,
                pub num_instances: u32,
                pub start_vertex: u32,
                pub start_instance: u32,
            }
            #[repr(C)]
            #[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
            pub struct DrawIndexedIndirectArgs {
                pub num_indices: u32,
                pub num_instances: u32,
                pub start_index: u32,
                pub start_vertex: u32,
                pub start_instance: u32,
            }
            let mut block_face_draw_args: Vec<DrawIndirectArgs> = Vec::new();
            let mut tinted_block_face_draw_args: Vec<DrawIndirectArgs> = Vec::new();
            let mut custom_block_draw_args: Vec<DrawIndexedIndirectArgs> = Vec::new();
            while let Some(queued_chunk) = chunk_queue.pop_front() {
                let QueuedChunk {
                    coords: chunk_coords,
                    from_dir,
                    back_travel_amount: chunk_back_travel_amount,
                    flipping_state: chunk_flip_state,
                } = queued_chunk;
                let subchunk_maybe = subchunks.get(&chunk_coords);
                // Visit neighbours
                'neighbour_blk: {
                    let (cur_x_flip, cur_y_flip, cur_z_flip) =
                        if debug_state.cave_cull_check_unflipped {
                            match chunk_flip_state {
                                FlippingState::Unflipped {
                                    x_positive,
                                    y_positive,
                                    z_positive,
                                } => (x_positive, y_positive, z_positive),
                                FlippingState::Flipped => break 'neighbour_blk,
                            }
                        } else {
                            (None, None, None)
                        };
                    let [chunk_x, chunk_y, chunk_z] = chunk_coords;
                    let neighbour_chunks = [
                        ([chunk_x - 1, chunk_y, chunk_z], AxisDirection::West),
                        ([chunk_x + 1, chunk_y, chunk_z], AxisDirection::East),
                        ([chunk_x, chunk_y, chunk_z - 1], AxisDirection::North),
                        ([chunk_x, chunk_y, chunk_z + 1], AxisDirection::South),
                        ([chunk_x, chunk_y - 1, chunk_z], AxisDirection::Down),
                        ([chunk_x, chunk_y + 1, chunk_z], AxisDirection::Up),
                    ];
                    // let facing_dir = debug_state
                    //     .cull_camera
                    //     .get_rot()
                    //     .transform_vector(&-Vector3::z());
                    let facing_dir = self.camera.get_rot().transform_vector(&-Vector3::z());
                    'neighbour_loop: for (neighbour_coord, to_dir) in neighbour_chunks {
                        const WORLD_HEIGHT_I32: i32 = 384;
                        if neighbour_coord[1] < 0 || neighbour_coord[1] > WORLD_HEIGHT_I32 / 16 {
                            continue;
                        }
                        if !loaded_chunks.contains(&[neighbour_coord[0], neighbour_coord[2]]) {
                            continue;
                        }
                        // Check we're haven't gone backwards too much
                        let back_travel_diff = -facing_dir.dot(&to_dir.as_vector());
                        let neighbour_back_travel_amount =
                            (chunk_back_travel_amount + back_travel_diff).max(0.0);
                        if debug_state.cave_cull_check_not_backwards
                            && neighbour_back_travel_amount >= 1.1
                        {
                            continue;
                        }
                        if let Some(from_dir) = from_dir {
                            // Check we can go to the neighbour from the last subchunk through this
                            // subchunk
                            if debug_state.cave_cull_check_connectivity {
                                if let Some(subchunk) = subchunk_maybe {
                                    if !subchunk.connected_faces.connects(&from_dir, &to_dir) {
                                        continue;
                                    }
                                }
                            }
                        }
                        // Check neighbour lies in camera frustum
                        if debug_state.cave_cull_check_frustum {
                            // use super::MIN_HEIGHT_I32;
                            let start_coords = [
                                (neighbour_coord[0] * 16) as f32,
                                (neighbour_coord[1] * 16 + MIN_HEIGHT_I32) as f32,
                                (neighbour_coord[2] * 16) as f32,
                            ];
                            let end_coords = start_coords.map(|n| n + 16.0);
                            for (i, clip_plane) in camera_clipping_planes.into_iter().enumerate() {
                                let (normal, offset) = clip_plane;
                                if i >= debug_state.cull_planes_active {
                                    break;
                                }
                                let inward_point = Point3::new(
                                    match normal.x > 0.0 {
                                        false => start_coords[0],
                                        true => end_coords[0],
                                    },
                                    match normal.y > 0.0 {
                                        false => start_coords[1],
                                        true => end_coords[1],
                                    },
                                    match normal.z > 0.0 {
                                        false => start_coords[2],
                                        true => end_coords[2],
                                    },
                                );
                                if inward_point.coords.dot(&normal) + offset < 0.0 {
                                    continue 'neighbour_loop;
                                }
                            }
                        }
                        // Check we haven't already rendered the neighbour
                        if visited_chunks.contains(&neighbour_coord) {
                            continue;
                        }
                        // Calculate flip state for neighbour
                        visited_chunks.insert(neighbour_coord);
                        chunk_queue.push_back(QueuedChunk {
                            coords: neighbour_coord,
                            from_dir: Some(to_dir.invert()),
                            back_travel_amount: neighbour_back_travel_amount,
                            flipping_state: {
                                let (new_x_flip, new_y_flip, new_z_flip) = match to_dir {
                                    AxisDirection::Down => (None, Some(false), None),
                                    AxisDirection::Up => (None, Some(true), None),
                                    AxisDirection::North => (None, None, Some(false)),
                                    AxisDirection::South => (None, None, Some(true)),
                                    AxisDirection::West => (Some(false), None, None),
                                    AxisDirection::East => (Some(true), None, None),
                                };
                                if [
                                    cur_x_flip.zip(new_x_flip),
                                    cur_y_flip.zip(new_y_flip),
                                    cur_z_flip.zip(new_z_flip),
                                ]
                                .iter()
                                .any(|&flips| flips.is_some_and(|(x, y)| x != y))
                                {
                                    FlippingState::Flipped
                                } else {
                                    FlippingState::Unflipped {
                                        x_positive: new_x_flip.or(cur_x_flip),
                                        y_positive: new_y_flip.or(cur_y_flip),
                                        z_positive: new_z_flip.or(cur_z_flip),
                                    }
                                }
                            },
                        });
                        subchunk_traversal_graph.push((chunk_coords, neighbour_coord));
                    }
                }
                let Some(subchunk) = subchunk_maybe else {
                    continue;
                };
                if num_subchunks_rendered >= debug_state.max_render_chunks {
                    break;
                } else {
                    num_subchunks_rendered += 1;
                }
                rendered_chunks.insert(chunk_coords);
                for i in 0..6 {
                    let skip_face_dir = match i {
                        0 => chunk_coords[1] > camera_chunk_coords[1],
                        1 => chunk_coords[1] < camera_chunk_coords[1],
                        2 => chunk_coords[2] < camera_chunk_coords[2],
                        3 => chunk_coords[2] > camera_chunk_coords[2],
                        4 => chunk_coords[0] > camera_chunk_coords[0],
                        5 => chunk_coords[0] < camera_chunk_coords[0],
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
                    custom_block_draw_args.push(DrawIndexedIndirectArgs {
                        num_indices: group.start_index_and_len[1],
                        num_instances: group.start_instance_and_len[1],
                        start_index: group.start_index_and_len[0],
                        start_vertex: group.start_vertex,
                        start_instance: group.start_instance_and_len[0],
                    });
                }
            }
            {
                let subchunk_coord_set: AHashSet<_> = subchunks.keys().copied().collect();
                subchunks_skipped = subchunk_coord_set.difference(&rendered_chunks).count()
            }
            let lightmap_bind_group =
                self.resources
                    .device
                    .create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("Lightmap Bind Group"),
                        layout: &self.radiance_probe_lightmap_bind_group_layout,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: self.buffer_managers.block_face_instance.get_lightmaps()[0]
                                .as_entire_binding(),
                        }],
                    });
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &self.block_item_atlas_bind_group, &[]);
            render_pass.set_bind_group(2, &self.matrices_bind_group, &[]);
            render_pass.set_bind_group(3, &lightmap_bind_group, &[]);
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
                render_pass.set_vertex_buffer(0, self.custom_block_vertices_buffer.slice(..));
                render_pass
                    .set_vertex_buffer(1, self.buffer_managers.custom_block_instance.get_slice());
                render_pass.set_index_buffer(
                    self.custom_block_indices_buffer.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                render_pass.multi_draw_indexed_indirect(
                    &custom_block_draw_args_buffer,
                    0,
                    custom_block_draw_args.len().try_into().unwrap(),
                );
            }
            // Debug crosshair
            {
                render_pass.set_pipeline(&self.debug_crosshair_render_pipeline);
                render_pass.set_bind_group(0, &self.debug_crosshair_camera_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.debug_crosshair_vertex_buffer.slice(..));
                render_pass.draw(0..6, 0..1);
            }
            // egui
            self.egui_renderer
                .render(&mut render_pass, &egui_render_data);
        }
        self.resources
            .queue
            .submit(std::iter::once(encoder.finish()));
        output.present();
        self.egui_renderer
            .free_textures(&egui_full_output.textures_delta.free);
        Ok(DebugOutput {
            subchunks_culled: subchunks_skipped,
            subchunk_traversal_graph,
        })
    }

    pub fn radiance_cascades_debug_render(
        &mut self,
        subchunks: &AHashMap<[i32; 3], chunk_rc::Subchunk>,
    ) {
        let device = &self.resources.device;
        let (subchunk_hash_map_buffer, custom_block_group_buffer) = {
            #[repr(C)]
            #[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
            struct SubchunkHashMapEntry {
                pub pos: [f32; 3],
                pub block_face_instance_slices: [[u32; 2]; 6],
                pub tinted_block_face_instance_slices: [[u32; 2]; 6],
                pub custom_block_group_slice: [u32; 2],
            }
            const EMPTY_ENTRY: SubchunkHashMapEntry = SubchunkHashMapEntry {
                pos: [0.1; 3],
                block_face_instance_slices: [[0; 2]; 6],
                tinted_block_face_instance_slices: [[0; 2]; 6],
                custom_block_group_slice: [0; 2],
            };
            struct SubchunkHashMap {
                pub entries: Vec<SubchunkHashMapEntry>,
                pub num_stored_entries: usize,
            }
            impl SubchunkHashMap {
                fn murmur_32_scramble(mut k: u32) -> u32 {
                    k = k.wrapping_mul(0xCC9E2D51);
                    k = (k << 15) | (k >> 17);
                    k = k.wrapping_mul(0x1B873593);
                    k
                }

                fn subchunk_hash(key: [f32; 3]) -> u32 {
                    // Hash components of key
                    let mut hash: u32 = 0;
                    for component in key {
                        hash ^= Self::murmur_32_scramble(component.to_bits());
                        hash = (hash << 13) | (hash >> 19);
                        hash = hash.wrapping_mul(5).wrapping_add(0xE6546B64);
                    }
                    // Finalise hash
                    hash ^= hash >> 16;
                    hash = hash.wrapping_mul(0x85EBCA6B);
                    hash ^= hash >> 13;
                    hash = hash.wrapping_mul(0xC2B2AE35);
                    hash ^= hash >> 16;
                    hash
                }

                pub fn insert(&mut self, new_entry: SubchunkHashMapEntry) {
                    debug_assert!(self.num_stored_entries < self.entries.len() - 1);
                    // Insert entry
                    let mut current_slot = usize::try_from(Self::subchunk_hash(new_entry.pos))
                        .unwrap()
                        % self.entries.len();
                    loop {
                        if self.entries[current_slot].pos == [0.1; 3] {
                            self.entries[current_slot] = new_entry;
                            self.num_stored_entries += 1;
                            return;
                        }
                        current_slot += 1;
                        current_slot %= self.entries.len();
                    }
                }

                pub fn lookup(&self, pos: [f32; 3]) -> Option<SubchunkHashMapEntry> {
                    let mut entries_searched: usize = 1;
                    let mut current_slot =
                        usize::try_from(Self::subchunk_hash(pos)).unwrap() % self.entries.len();
                    loop {
                        let entry = &self.entries[current_slot];
                        if entry.pos == pos {
                            dbg!(entries_searched);
                            return Some(*entry);
                        } else if self.entries[current_slot].pos == [0.1; 3] {
                            dbg!(entries_searched);
                            return None;
                        }
                        current_slot += 1;
                        current_slot %= self.entries.len();
                        entries_searched += 1;
                    }
                }
            }
            let subchunk_hash_map_capacity = if subchunks.len().is_power_of_two() {
                (subchunks.len() + 1).next_power_of_two()
            } else {
                subchunks.len().next_power_of_two()
            };
            let mut subchunk_hash_map = SubchunkHashMap {
                entries: vec![EMPTY_ENTRY; subchunk_hash_map_capacity],
                num_stored_entries: 0,
            };
            let mut custom_block_groups: Vec<chunk_rc::CustomBlockGroup> = Vec::new();
            for (_subchunk_pos, subchunk) in subchunks {
                let custom_block_group_slice = if !subchunk.custom_block_groups.is_empty() {
                    let group_start: u32 = custom_block_groups.len().try_into().unwrap();
                    let group_len: u32 = subchunk.custom_block_groups.len().try_into().unwrap();
                    custom_block_groups.extend(&subchunk.custom_block_groups);
                    [group_start, group_len]
                } else {
                    [0; 2]
                };
                subchunk_hash_map.insert(SubchunkHashMapEntry {
                    pos: subchunk.start_coords.map(|n| (n / 16) as f32),
                    block_face_instance_slices: subchunk
                        .block_face_instance_groups
                        .map(|(start, len)| [start.try_into().unwrap(), len.try_into().unwrap()]),
                    tinted_block_face_instance_slices: subchunk
                        .tinted_block_face_instance_groups
                        .map(|(start, len)| [start.try_into().unwrap(), len.try_into().unwrap()]),
                    custom_block_group_slice,
                });
            }
            let hash_map_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Subchunk Hash Map Buffer"),
                contents: bytemuck::cast_slice(&subchunk_hash_map.entries),
                usage: wgpu::BufferUsages::STORAGE,
            });
            let group_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Custom Block Group Buffer"),
                contents: bytemuck::cast_slice(&custom_block_groups),
                usage: wgpu::BufferUsages::STORAGE,
            });
            (hash_map_buffer, group_buffer)
        };
        let radiance_probe_info_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Radiance Probe Info Bind Group"),
            layout: &self.radiance_probe_info_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: subchunk_hash_map_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self
                        .buffer_managers
                        .block_face_instance
                        .get_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self
                        .buffer_managers
                        .tinted_block_face_instance
                        .get_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.custom_block_vertices_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.custom_block_indices_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self
                        .buffer_managers
                        .custom_block_instance
                        .get_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: custom_block_group_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: self.radiance_probe_debug_info_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: self.buffer_managers.block_face_instance.get_lightmaps()[0]
                        .as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: self.buffer_managers.block_face_instance.get_lightmaps()[1]
                        .as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::TextureView(&self.block_item_atlas.luma_view),
                },
            ],
        });
        let mut encoder =
            self.resources
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Compute Encoder"),
                });
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Subchunk Radiance Cascade Update Compute Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.radiance_probe_debug_pipeline);
            compute_pass.set_bind_group(0, &self.block_item_atlas_bind_group, &[]);
            compute_pass.set_bind_group(1, &self.matrices_bind_group, &[]);
            compute_pass.set_bind_group(2, &radiance_probe_info_bind_group, &[]);
            compute_pass.set_bind_group(
                3,
                &self.radiance_probe_debug_texture_storage_bind_group,
                &[],
            );
            #[repr(C)]
            #[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
            struct UpdateInfo {
                pub inv_view_matrix: [[f32; 4]; 4],
            }
            compute_pass.set_push_constants(
                0,
                bytemuck::cast_slice(&[UpdateInfo {
                    inv_view_matrix: *self
                        .camera
                        .generate_view_matrix()
                        .try_inverse()
                        .unwrap()
                        .as_ref(),
                }]),
            );
            // compute_pass.dispatch_workgroups(1, 1, 1);
            compute_pass.dispatch_workgroups(960 / 16, 540 / 4, 1);
        }
        self.resources
            .queue
            .submit(std::iter::once(encoder.finish()));
        // Read back debug info
        {
            let buffer = &self.radiance_probe_debug_info_buffer;
            let buffer_capturable = buffer.clone();
            let debug_info = self.radiance_probe_debug_info.clone();
            buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    if result.is_ok() {
                        let view = buffer_capturable.slice(..).get_mapped_range();
                        let info_slice: &[RadianceProbeDebugInfo] = bytemuck::cast_slice(&view);
                        let info = info_slice[0];
                        drop(view);
                        buffer_capturable.unmap();
                        *debug_info.lock().unwrap() = info;
                    }
                });
        }
        self.resources.queue.submit([]);
    }

    pub fn update_all_subchunks_radiance_lighting(
        &mut self,
        subchunks: &AHashMap<[i32; 3], chunk_rc::Subchunk>,
    ) {
        let device = &self.resources.device;
        let (subchunk_hash_map_buffer, custom_block_group_buffer) = {
            #[repr(C)]
            #[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
            struct SubchunkHashMapEntry {
                pub pos: [f32; 3],
                pub block_face_instance_slices: [[u32; 2]; 6],
                pub tinted_block_face_instance_slices: [[u32; 2]; 6],
                pub custom_block_group_slice: [u32; 2],
            }
            const EMPTY_ENTRY: SubchunkHashMapEntry = SubchunkHashMapEntry {
                pos: [0.1; 3],
                block_face_instance_slices: [[0; 2]; 6],
                tinted_block_face_instance_slices: [[0; 2]; 6],
                custom_block_group_slice: [0; 2],
            };
            struct SubchunkHashMap {
                pub entries: Vec<SubchunkHashMapEntry>,
                pub num_stored_entries: usize,
            }
            impl SubchunkHashMap {
                fn murmur_32_scramble(mut k: u32) -> u32 {
                    k = k.wrapping_mul(0xCC9E2D51);
                    k = (k << 15) | (k >> 17);
                    k = k.wrapping_mul(0x1B873593);
                    k
                }

                fn subchunk_hash(key: [f32; 3]) -> u32 {
                    // Hash components of key
                    let mut hash: u32 = 0;
                    for component in key {
                        hash ^= Self::murmur_32_scramble(component.to_bits());
                        hash = (hash << 13) | (hash >> 19);
                        hash = hash.wrapping_mul(5).wrapping_add(0xE6546B64);
                    }
                    // Finalise hash
                    hash ^= hash >> 16;
                    hash = hash.wrapping_mul(0x85EBCA6B);
                    hash ^= hash >> 13;
                    hash = hash.wrapping_mul(0xC2B2AE35);
                    hash ^= hash >> 16;
                    hash
                }

                pub fn insert(&mut self, new_entry: SubchunkHashMapEntry) {
                    debug_assert!(self.num_stored_entries < self.entries.len() - 1);
                    // Insert entry
                    let mut current_slot = usize::try_from(Self::subchunk_hash(new_entry.pos))
                        .unwrap()
                        % self.entries.len();
                    loop {
                        if self.entries[current_slot].pos == [0.1; 3] {
                            self.entries[current_slot] = new_entry;
                            self.num_stored_entries += 1;
                            return;
                        }
                        current_slot += 1;
                        current_slot %= self.entries.len();
                    }
                }
            }
            let subchunk_hash_map_capacity = if subchunks.len().is_power_of_two() {
                (subchunks.len() + 1).next_power_of_two()
            } else {
                subchunks.len().next_power_of_two()
            };
            let mut subchunk_hash_map = SubchunkHashMap {
                entries: vec![EMPTY_ENTRY; subchunk_hash_map_capacity],
                num_stored_entries: 0,
            };
            let mut custom_block_groups: Vec<chunk_rc::CustomBlockGroup> = Vec::new();
            for (_subchunk_pos, subchunk) in subchunks {
                let custom_block_group_slice = if !subchunk.custom_block_groups.is_empty() {
                    let group_start: u32 = custom_block_groups.len().try_into().unwrap();
                    let group_len: u32 = subchunk.custom_block_groups.len().try_into().unwrap();
                    custom_block_groups.extend(&subchunk.custom_block_groups);
                    [group_start, group_len]
                } else {
                    [0; 2]
                };
                subchunk_hash_map.insert(SubchunkHashMapEntry {
                    pos: subchunk.start_coords.map(|n| (n / 16) as f32),
                    block_face_instance_slices: subchunk
                        .block_face_instance_groups
                        .map(|(start, len)| [start.try_into().unwrap(), len.try_into().unwrap()]),
                    tinted_block_face_instance_slices: subchunk
                        .tinted_block_face_instance_groups
                        .map(|(start, len)| [start.try_into().unwrap(), len.try_into().unwrap()]),
                    custom_block_group_slice,
                });
            }
            let hash_map_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Subchunk Hash Map Buffer"),
                contents: bytemuck::cast_slice(&subchunk_hash_map.entries),
                usage: wgpu::BufferUsages::STORAGE,
            });
            let group_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Custom Block Group Buffer"),
                contents: bytemuck::cast_slice(&custom_block_groups),
                usage: wgpu::BufferUsages::STORAGE,
            });
            (hash_map_buffer, group_buffer)
        };
        // XXX: DEBUG
        {
            let block_face_buffer_usage =
                self.buffer_managers.block_face_instance.usage_fraction() * 100.0;
            let tinted_block_face_buffer_usage = self
                .buffer_managers
                .tinted_block_face_instance
                .usage_fraction()
                * 100.0;
            println!("Block Face Buffer Usage: {block_face_buffer_usage:.1}%");
            println!("Tinted Block Face Buffer Usage: {tinted_block_face_buffer_usage:.1}%");
        }
        let (update_info_buffer, num_updates, max_dispatch_width) = {
            #[repr(C)]
            #[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
            struct UpdateInfo {
                pub subchunk_start_coords: [f32; 3],
                pub faces_start: u32,
                pub faces_len: u32,
                pub faces_dir_i: u32,
            }
            let mut updates = Vec::new();
            let mut max_dispatch_width = 0;
            for subchunk in subchunks.values() {
                for dir_i in 0..6 {
                    if subchunk.block_face_start_vertices[dir_i] != u32::MAX {
                        let instance_group = &subchunk.block_face_instance_groups[dir_i];
                        updates.push(UpdateInfo {
                            subchunk_start_coords: subchunk.start_coords.map(|n| n as f32),
                            faces_start: instance_group.0,
                            faces_len: instance_group.1,
                            faces_dir_i: dir_i.try_into().unwrap(),
                        });
                        let dispatch_width = instance_group.1;
                        if dispatch_width > max_dispatch_width {
                            max_dispatch_width = dispatch_width;
                        }
                    }
                }
            }
            let update_info_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Subchunk Hash Map Buffer"),
                contents: bytemuck::cast_slice(&updates),
                usage: wgpu::BufferUsages::STORAGE,
            });
            (
                update_info_buffer,
                u32::try_from(updates.len()).unwrap(),
                max_dispatch_width,
            )
        };
        let radiance_probe_info_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Radiance Probe Info Bind Group"),
            layout: &self.radiance_probe_info_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: subchunk_hash_map_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self
                        .buffer_managers
                        .block_face_instance
                        .get_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self
                        .buffer_managers
                        .tinted_block_face_instance
                        .get_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.custom_block_vertices_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.custom_block_indices_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self
                        .buffer_managers
                        .custom_block_instance
                        .get_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: custom_block_group_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: update_info_buffer.as_entire_binding(),
                },
                // TODO: Move input and output buffers into a new bind group
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: self.buffer_managers.block_face_instance.get_lightmaps()[0]
                        .as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: self.buffer_managers.block_face_instance.get_lightmaps()[1]
                        .as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::TextureView(&self.block_item_atlas.luma_view),
                },
            ],
        });
        let cascade_1_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Cascade 1 Radiance Probe Info Bind Group"),
            layout: &self.radiance_probe_info_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: subchunk_hash_map_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self
                        .buffer_managers
                        .block_face_instance
                        .get_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self
                        .buffer_managers
                        .tinted_block_face_instance
                        .get_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.custom_block_vertices_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.custom_block_indices_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self
                        .buffer_managers
                        .custom_block_instance
                        .get_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: custom_block_group_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: update_info_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: self.buffer_managers.block_face_instance.get_lightmaps()[1]
                        .as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: self.buffer_managers.block_face_instance.get_lightmaps()[0]
                        .as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::TextureView(&self.block_item_atlas.luma_view),
                },
            ],
        });
        let mut encoder =
            self.resources
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Compute Encoder"),
                });
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Subchunk Radiance Cascade Update Compute Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.radiance_probe_update_pipelines[1]);
            compute_pass.set_bind_group(0, &self.block_item_atlas_bind_group, &[]);
            compute_pass.set_bind_group(1, &self.matrices_bind_group, &[]);
            compute_pass.set_bind_group(2, &cascade_1_bind_group, &[]);
            compute_pass.set_bind_group(
                3,
                &self.radiance_probe_debug_texture_storage_bind_group,
                &[],
            );
            compute_pass.dispatch_workgroups(max_dispatch_width, num_updates, 1);
            compute_pass.set_pipeline(&self.radiance_probe_update_pipelines[0]);
            compute_pass.set_bind_group(2, &radiance_probe_info_bind_group, &[]);
            compute_pass.dispatch_workgroups(max_dispatch_width, num_updates, 1);
        }
        self.resources
            .queue
            .submit(std::iter::once(encoder.finish()));
        self.resources.queue.submit([]);
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
