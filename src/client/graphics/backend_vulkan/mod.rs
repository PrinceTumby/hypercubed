pub mod chunk;
pub mod chunk_rc;
pub mod debug;
pub mod egui_renderer;
pub mod shader_exports;

use crate::basic_types::AxisDirection;
use crate::client::{MIN_HEIGHT_I32, RawChunk, RayTracedQuadInfo, SUBCHUNK_AXIS_LEN_I32};
use crate::physics::AABB;
use crate::resource;
use ahash::{AHashMap, AHashSet};
use anyhow::{Context, anyhow};
use chunk_rc::{
    block_face::{BlockFaceInstanceBufferManager, BlockFaceVertexBufferManager},
    custom_block::CustomBlockInstanceBufferManager,
    tinted_block_face::{TintedBlockFaceInstanceBufferManager, TintedBlockFaceVertexBufferManager},
};
use debug::line::Instance as DebugLineInstance;
use debug::point::Vertex as DebugPointVertex;
use debug::triangle::Instance as DebugTriangleInstance;
use nalgebra::{Perspective3, Point3, Vector3};
use ordered_float::NotNan;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use threadpool::ThreadPool;
use vulkan_prelude::*;
use winit::window::Window;

pub use super::Camera;

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

#[derive(Clone)]
pub struct GraphicsResources {
    pub device: Arc<vulkano::device::Device>,
    pub render_queue: Arc<vulkano::device::Queue>,
    pub compute_queue: Arc<vulkano::device::Queue>,
    pub memory_allocator: Arc<VulkanStandardMemoryAllocator>,
    pub command_buffer_allocator: Arc<VulkanStandardCommandBufferAllocator>,
    pub descriptor_set_allocator: Arc<VulkanStandardDescriptorSetAllocator>,
    pub render_pass: Arc<VulkanRenderPass>,
    pub block_registry: Arc<resource::block::Registry>,
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
    pub fn get_present_mode(&self) -> VulkanPresentMode {
        match self.vsync {
            true => VulkanPresentMode::Fifo,
            false => VulkanPresentMode::Immediate,
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

pub struct RadianceCascadeState {
    pub tlas_info: Option<TlasInfo>,
    pub update_pipelines: [Arc<VulkanComputePipeline>; 2],
    pub update_pipeline_layout: Arc<VulkanPipelineLayout>,
    pub probe_debug_info_descriptor_set_layout: Arc<VulkanDescriptorSetLayout>,
    pub probe_info_descriptor_set_layout: Arc<VulkanDescriptorSetLayout>,
    pub lightmaps_descriptor_set_layout: Arc<VulkanDescriptorSetLayout>,
    pub debug_texture_descriptor_set: Arc<VulkanDescriptorSet>,
    pub lightmap_render_descriptor_set_layout: Arc<VulkanDescriptorSetLayout>,
    // XXX: DEBUG
    pub debug_info: Arc<Mutex<RadianceProbeDebugInfo>>,
    pub debug_egui_texture: egui::load::SizedTexture,
    pub debug_pipeline: Arc<VulkanComputePipeline>,
    pub debug_pipeline_layout: Arc<VulkanPipelineLayout>,
    pub debug_light_tree: Option<Vec<LightNode>>,
}

// TODO: Move this to shader chunk_rc types
// Node 0 is a dummy node, only containing the number of root nodes.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightNode {
    pub sphere_centre: [f32; 3],
    pub sphere_radius: f32,
    pub aabb_corner_1: [f32; 3],
    pub aabb_corner_2: [f32; 3],
    /// Equal to `[u32::MAX; 2]` if the node has no children.
    pub children: [u32; 2],
}

#[derive(Clone)]
pub struct TlasInfo {
    pub world_tlas: Arc<VulkanAccelerationStructure>,
    pub instance_info_buffer: VulkanSubbuffer<[TlasInstanceInfo]>,
    pub quads_info_buffer: VulkanSubbuffer<[RayTracedQuadInfo]>,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TlasInstanceInfo {
    pub quads_info_offsets: [u32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct RadianceCascadeUpdateInfo {
    pub inv_view_matrix: [[f32; 4]; 4],
}

pub struct GraphicsState {
    pub resources: GraphicsResources,
    pub swapchain: Arc<vulkano::swapchain::Swapchain>,
    pub swapchain_images: Vec<Arc<VulkanImage>>,
    pub depth_image: Arc<VulkanImage>,
    pub egui_renderer: egui_renderer::Renderer,
    pub graphics_options: GraphicsOptions,
    pub radiance_cascades: RadianceCascadeState,
    pub block_graphics_pipeline: Arc<VulkanGraphicsPipeline>,
    pub generic_block_graphics_pipeline_layout: Arc<VulkanPipelineLayout>,
    pub tinted_block_graphics_pipeline: Arc<VulkanGraphicsPipeline>,
    pub custom_block_graphics_pipeline: Arc<VulkanGraphicsPipeline>,
    pub custom_block_vertices_buffer: VulkanSubbuffer<[chunk_rc::custom_block::Vertex]>,
    pub custom_block_indices_buffer: VulkanSubbuffer<[u32]>,
    pub camera_buffer: VulkanSubbuffer<[[f32; 4]; 4]>,
    pub camera_descriptor_set: Arc<VulkanDescriptorSet>,
    pub matrices_descriptor_set: Arc<VulkanDescriptorSet>,
    pub block_item_atlas: TextureAtlas,
    pub block_item_atlas_descriptor_set: Arc<VulkanDescriptorSet>,
    pub buffer_managers: GraphicsBufferManagers,
    // Debug state
    pub debug_point_pipeline: Arc<VulkanGraphicsPipeline>,
    pub debug_line_pipeline: Arc<VulkanGraphicsPipeline>,
    pub debug_triangle_pipeline: Arc<VulkanGraphicsPipeline>,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub camera: Camera,
}

impl GraphicsState {
    pub async fn new<F>(window: &'static Window, register_blocks: F) -> anyhow::Result<Self>
    where
        F: FnOnce(
            &mut resource::block::Registry,
            &mut resource::block::model::ModelCache,
            &mut resource::texture::AtlasBuilder,
        ) -> anyhow::Result<()>,
    {
        let graphics_options = GraphicsOptions::default();
        // Initialise Vulkan state
        let library = VulkanLibrary::new().context("Failed to load Vulkan library")?;
        let surface_required_extensions = VulkanSurface::required_extensions(window)
            .context("Failed to retrieve required surface extensions from window")?;
        let instance = VulkanInstance::new(
            &library,
            &VulkanInstanceCreateInfo {
                enabled_extensions: &VulkanInstanceExtensions {
                    ext_debug_utils: true,
                    ..surface_required_extensions
                },
                ..VulkanInstanceCreateInfo::application_from_cargo_toml()
            },
        )
        .context("Failed to create Vulkan instance")?;
        // SAFETY: `window` is 'static lifetime, so will definitely outlive surface.
        let surface = unsafe {
            VulkanSurface::from_window_ref(&instance, window).context("Failed to create surface")?
        };
        // Find a suitable physical device
        let required_extensions = VulkanDeviceExtensions {
            khr_swapchain: true,
            // NOTE: RADIANCE CASCADES
            khr_shader_non_semantic_info: true,
            khr_acceleration_structure: true,
            khr_ray_query: true,
            ..VulkanDeviceExtensions::empty()
        };
        let required_features = VulkanDeviceFeatures {
            vulkan_memory_model: true,
            multi_draw_indirect: true,
            shader_int8: true,
            shader_int16: true,
            storage_buffer8_bit_access: true,
            storage_buffer16_bit_access: true,
            // NOTE: RADIANCE CASCADES
            acceleration_structure: true,
            buffer_device_address: true,
            ray_query: true,
            ..VulkanDeviceFeatures::empty()
        };
        let (physical_device, render_queue_family_index, compute_queue_family_index) = instance
            .enumerate_physical_devices()
            .context("Failed to enumerate Vulkan devices")?
            .filter(|device| {
                device.supported_extensions().contains(&required_extensions)
                    && device.supported_features().contains(&required_features)
            })
            .filter_map(|device| {
                let Some(render_queue_family_usize) = device
                    .queue_family_properties()
                    .iter()
                    .enumerate()
                    .position(|(i, queue_family)| {
                        queue_family
                            .queue_flags
                            .contains(VulkanQueueFlags::GRAPHICS | VulkanQueueFlags::COMPUTE)
                            && device.surface_support(i as u32, &surface).unwrap_or(false)
                    })
                else {
                    return None;
                };
                let render_queue_family = render_queue_family_usize as u32;
                let compute_queue_family = device
                    .queue_family_properties()
                    .iter()
                    .enumerate()
                    .position(|(i, queue_family)| {
                        queue_family.queue_flags.contains(VulkanQueueFlags::COMPUTE)
                            && i != render_queue_family_usize
                    })
                    .map(|idx| idx as u32)
                    .unwrap_or(render_queue_family);
                Some((device, render_queue_family, compute_queue_family))
            })
            .min_by_key(|(device, _, _)| match device.properties().device_type {
                VulkanPhysicalDeviceType::DiscreteGpu => 0,
                // XXX: DEBUG
                // VulkanPhysicalDeviceType::IntegratedGpu => -1,
                VulkanPhysicalDeviceType::IntegratedGpu => 1,
                VulkanPhysicalDeviceType::VirtualGpu => 2,
                VulkanPhysicalDeviceType::Cpu => 3,
                _ => 4,
            })
            .context("Error while finding any suitable Vulkan devices")?;
        dbg!(render_queue_family_index, compute_queue_family_index);
        let surface_capabilities = physical_device
            .surface_capabilities(&surface, &Default::default())
            .context("Error while getting Vulkan surface capabilities")?;
        let size = window.inner_size();
        let composite_alpha = surface_capabilities
            .supported_composite_alpha
            .into_iter()
            .next()
            .unwrap();
        // HACK: My computer (Windows) returns B8G8R8A8_UNORM as the first surface format for some
        //       reason? Completely messes up the colours, so forcing this for now. Best solution
        //       is probably to use R8G8B8A8_SRGB or B8G8R8A8_SRGB by default if they're supported,
        //       and only then fall back to whatever's actually supported and hope for the best.
        let image_format = VulkanFormat::R8G8B8A8_SRGB;
        // let image_format = physical_device
        //     .surface_formats(&surface, Default::default())
        //     .unwrap()[0]
        //     .0;
        let (device, mut queue_iter) = VulkanDevice::new(
            &physical_device,
            &VulkanDeviceCreateInfo {
                queue_create_infos: &if render_queue_family_index == compute_queue_family_index {
                    vec![VulkanQueueCreateInfo {
                        queue_family_index: render_queue_family_index,
                        queues: &[1.0, 0.0],
                        ..Default::default()
                    }]
                } else {
                    vec![
                        VulkanQueueCreateInfo {
                            queue_family_index: render_queue_family_index,
                            queues: &[1.0],
                            ..Default::default()
                        },
                        VulkanQueueCreateInfo {
                            queue_family_index: compute_queue_family_index,
                            queues: &[0.0],
                            ..Default::default()
                        },
                    ]
                },
                enabled_extensions: &required_extensions,
                enabled_features: &required_features,
                ..Default::default()
            },
        )
        .context("Error while creating Vulkan device")?;
        let render_queue = queue_iter.next().unwrap();
        let compute_queue = queue_iter.next().unwrap();
        let (swapchain, swapchain_images) = VulkanSwapchain::new(
            &device,
            &surface,
            &VulkanSwapchainCreateInfo {
                min_image_count: surface_capabilities.min_image_count + 1,
                image_format,
                image_extent: size.into(),
                image_usage: VulkanImageUsage::COLOR_ATTACHMENT,
                composite_alpha,
                present_mode: graphics_options.get_present_mode(),
                ..Default::default()
            },
        )
        .context("Error while creating swapchain")?;
        let memory_allocator = Arc::new(VulkanStandardMemoryAllocator::new(
            &device,
            &Default::default(),
        ));
        let command_buffer_allocator = Arc::new(VulkanStandardCommandBufferAllocator::new(
            &device,
            // Default options
            &Default::default(),
        ));
        let descriptor_set_allocator = Arc::new(VulkanStandardDescriptorSetAllocator::new(
            &device,
            // Default options
            &Default::default(),
        ));
        let render_pass = vulkan_single_pass_renderpass!(
            &device,
            attachments: {
                color: {
                    format: swapchain.image_format(),
                    samples: 1,
                    load_op: Clear,
                    store_op: Store,
                },
                depth: {
                    format: VulkanFormat::D32_SFLOAT,
                    samples: 1,
                    load_op: Clear,
                    store_op: DontCare,
                },
            },
            pass: {
                color: [color],
                depth_stencil: {depth},
            },
        )
        .context("Error while creating render pass")?;
        let depth_image = VulkanImage::new(
            &memory_allocator,
            &VulkanImageCreateInfo {
                image_type: VulkanImageType::Dim2d,
                format: render_pass.attachments()[1].format,
                extent: [size.width, size.height, 1],
                usage: VulkanImageUsage::DEPTH_STENCIL_ATTACHMENT,
                ..Default::default()
            },
            &VulkanAllocationCreateInfo {
                memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE,
                ..Default::default()
            },
        )
        .context("Error while creating depth image")?;
        // Initialise egui renderer, for debug UI
        let mut egui_renderer = egui_renderer::Renderer::new(
            &device,
            &(memory_allocator.clone() as Arc<_>),
            &(descriptor_set_allocator.clone() as Arc<_>),
            &render_pass,
        )
        .context("Error while creating egui renderer")?;
        // Initialise game state
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
            let atlas = atlas_builder
                .build(
                    &device,
                    &render_queue,
                    &(memory_allocator.clone() as Arc<_>),
                    command_buffer_allocator.clone(),
                )
                .context("Failed while building block and item atlas")?;
            let custom_block_vertices_buffer = VulkanBuffer::from_iter(
                &memory_allocator,
                &VulkanBufferCreateInfo {
                    usage: VulkanBufferUsage::VERTEX_BUFFER | VulkanBufferUsage::STORAGE_BUFFER,
                    ..Default::default()
                },
                &VulkanAllocationCreateInfo {
                    memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                        | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    allocate_preference: VulkanMemoryAllocatePreference::AlwaysAllocate,
                    ..Default::default()
                },
                model_cache.custom_block_vertices,
            )
            .context("Error while creating custom block vertices buffer")?;
            let custom_block_indices_buffer = VulkanBuffer::from_iter(
                &memory_allocator,
                &VulkanBufferCreateInfo {
                    usage: VulkanBufferUsage::INDEX_BUFFER | VulkanBufferUsage::STORAGE_BUFFER,
                    ..Default::default()
                },
                &VulkanAllocationCreateInfo {
                    memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                        | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    allocate_preference: VulkanMemoryAllocatePreference::AlwaysAllocate,
                    ..Default::default()
                },
                model_cache.custom_block_indices,
            )
            .context("Error while creating custom block indices buffer")?;
            (
                atlas,
                size,
                block_registry,
                custom_block_vertices_buffer,
                custom_block_indices_buffer,
            )
        };
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
        // Block graphics descriptor set layouts
        let camera_descriptor_set_layout = VulkanDescriptorSetLayout::new(
            &device.clone(),
            &VulkanDescriptorSetLayoutCreateInfo {
                bindings: &[VulkanDescriptorSetLayoutBinding {
                    binding: 0,
                    binding_flags: VulkanDescriptorBindingFlags::empty(),
                    descriptor_type: VulkanDescriptorType::UniformBuffer,
                    descriptor_count: 1,
                    stages: VulkanShaderStages::VERTEX,
                    immutable_samplers: &[],
                    _ne: vulkano_non_exhaustive(),
                }],
                ..Default::default()
            },
        )
        .context("Error while creating camera descriptor set layout")?;
        let matrices_descriptor_set_layout = VulkanDescriptorSetLayout::new(
            &device,
            &VulkanDescriptorSetLayoutCreateInfo {
                bindings: &[VulkanDescriptorSetLayoutBinding {
                    binding: 0,
                    binding_flags: VulkanDescriptorBindingFlags::empty(),
                    descriptor_type: VulkanDescriptorType::UniformBuffer,
                    descriptor_count: 1,
                    stages: VulkanShaderStages::VERTEX | VulkanShaderStages::COMPUTE,
                    immutable_samplers: &[],
                    _ne: vulkano_non_exhaustive(),
                }],
                ..Default::default()
            },
        )
        .context("Error while creating matrices descriptor set layout")?;
        let block_item_atlas_descriptor_set_layout = VulkanDescriptorSetLayout::new(
            &device,
            &VulkanDescriptorSetLayoutCreateInfo {
                bindings: &[
                    // Block and item atlas image
                    VulkanDescriptorSetLayoutBinding {
                        binding: 0,
                        binding_flags: VulkanDescriptorBindingFlags::empty(),
                        descriptor_type: VulkanDescriptorType::SampledImage,
                        descriptor_count: 1,
                        stages: VulkanShaderStages::FRAGMENT | VulkanShaderStages::COMPUTE,
                        immutable_samplers: &[],
                        _ne: vulkano_non_exhaustive(),
                    },
                    // Block and item sampler
                    VulkanDescriptorSetLayoutBinding {
                        binding: 1,
                        binding_flags: VulkanDescriptorBindingFlags::empty(),
                        descriptor_type: VulkanDescriptorType::Sampler,
                        descriptor_count: 1,
                        stages: VulkanShaderStages::FRAGMENT | VulkanShaderStages::COMPUTE,
                        immutable_samplers: &[&VulkanSampler::new(
                            &device,
                            &VulkanSamplerCreateInfo::default(),
                        )
                        .context("Error while creating block atlas image sampler")?],
                        _ne: vulkano_non_exhaustive(),
                    },
                    VulkanDescriptorSetLayoutBinding {
                        binding: 2,
                        binding_flags: VulkanDescriptorBindingFlags::empty(),
                        descriptor_type: VulkanDescriptorType::UniformBuffer,
                        descriptor_count: 1,
                        stages: VulkanShaderStages::VERTEX | VulkanShaderStages::COMPUTE,
                        immutable_samplers: &[],
                        _ne: vulkano_non_exhaustive(),
                    },
                ],
                ..Default::default()
            },
        )
        .context("Error while creating image descriptor set layout")?;
        // Block graphics buffers and descriptor sets
        let camera_buffer: VulkanSubbuffer<[[f32; 4]; 4]> = VulkanBuffer::new_sized(
            &memory_allocator,
            &VulkanBufferCreateInfo {
                usage: VulkanBufferUsage::UNIFORM_BUFFER | VulkanBufferUsage::TRANSFER_DST,
                ..Default::default()
            },
            &VulkanAllocationCreateInfo {
                memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE,
                ..Default::default()
            },
        )
        .context("Error while creating camera buffer")?;
        let camera_descriptor_set = VulkanDescriptorSet::new(
            descriptor_set_allocator.clone(),
            camera_descriptor_set_layout.clone(),
            [VulkanWriteDescriptorSet::buffer(0, camera_buffer.clone())],
            [],
        )
        .context("Error while creating camera descriptor set")?;
        let matrices_buffer = VulkanBuffer::from_data(
            &memory_allocator,
            &VulkanBufferCreateInfo {
                usage: VulkanBufferUsage::UNIFORM_BUFFER,
                ..Default::default()
            },
            &VulkanAllocationCreateInfo {
                memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                    | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            chunk_rc::block_face::face_matrices::generate_array(),
        )
        .context("Error while creating face matrices buffer")?;
        let matrices_descriptor_set = VulkanDescriptorSet::new(
            descriptor_set_allocator.clone(),
            matrices_descriptor_set_layout.clone(),
            [VulkanWriteDescriptorSet::buffer(0, matrices_buffer.clone())],
            [],
        )
        .context("Error while creating matrices descriptor set")?;
        let block_item_atlas_size_buffer = VulkanBuffer::from_data(
            &memory_allocator,
            &VulkanBufferCreateInfo {
                usage: VulkanBufferUsage::UNIFORM_BUFFER,
                ..Default::default()
            },
            &VulkanAllocationCreateInfo {
                memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                    | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            block_item_atlas_size.map(|n| n as f32),
        )
        .context("Error while creating face matrices buffer")?;
        let block_item_atlas_descriptor_set = VulkanDescriptorSet::new(
            descriptor_set_allocator.clone(),
            block_item_atlas_descriptor_set_layout.clone(),
            [
                VulkanWriteDescriptorSet::image_view(0, block_item_texture_atlas.view.clone()),
                // Sampler already in binding 1
                VulkanWriteDescriptorSet::buffer(2, block_item_atlas_size_buffer),
            ],
            [],
        )
        .context("Error while creating matrices descriptor set")?;
        let radiance_probe_lightmap_render_descriptor_set_layout = VulkanDescriptorSetLayout::new(
            &device,
            &VulkanDescriptorSetLayoutCreateInfo {
                bindings: &[VulkanDescriptorSetLayoutBinding {
                    binding: 0,
                    binding_flags: VulkanDescriptorBindingFlags::empty(),
                    descriptor_type: VulkanDescriptorType::StorageBuffer,
                    descriptor_count: 1,
                    stages: VulkanShaderStages::FRAGMENT,
                    immutable_samplers: &[],
                    _ne: vulkano_non_exhaustive(),
                }],
                ..Default::default()
            },
        )
        .context("Error while creating lightmap render descriptor set layout")?;
        // Block graphics pipelines
        let generic_block_graphics_pipeline_layout = VulkanPipelineLayout::new(
            &device,
            &VulkanPipelineLayoutCreateInfo {
                set_layouts: &[
                    &camera_descriptor_set_layout,
                    &block_item_atlas_descriptor_set_layout,
                    &matrices_descriptor_set_layout,
                    &radiance_probe_lightmap_render_descriptor_set_layout,
                ],
                ..Default::default()
            },
        )
        .context("Error while creating block graphics pipeline layout")?;
        let block_graphics_pipeline = chunk_rc::block_face::create_graphics_pipeline(
            &device,
            &generic_block_graphics_pipeline_layout,
            &render_pass.first_subpass(),
        )
        .context("Error while creating block graphics pipeline")?;
        let tinted_block_graphics_pipeline = chunk_rc::tinted_block_face::create_graphics_pipeline(
            &device,
            &generic_block_graphics_pipeline_layout,
            &render_pass.first_subpass(),
        )
        .context("Error while creating tinted block graphics pipeline")?;
        let custom_block_graphics_pipeline = chunk_rc::custom_block::create_graphics_pipeline(
            &device,
            &generic_block_graphics_pipeline_layout,
            &render_pass.first_subpass(),
        )
        .context("Error while creating custom block graphics pipeline")?;
        // Buffer managers
        let render_queue_family_index = render_queue.queue_family_index();
        let compute_queue_family_index = compute_queue.queue_family_index();
        let block_face_vertex_buffer_manager =
            BlockFaceVertexBufferManager::new(&device)
                .context("Error while creating block face vertex buffer manager")?;
        let block_face_instance_buffer_manager = BlockFaceInstanceBufferManager::new(
            &device,
            render_queue_family_index,
            compute_queue_family_index,
        )
        .context("Error while creating block face instance buffer manager")?;
        let tinted_block_face_vertex_buffer_manager =
            TintedBlockFaceVertexBufferManager::new(&device)
                .context("Error while creating tinted block face vertex buffer manager")?;
        let tinted_block_face_instance_buffer_manager = TintedBlockFaceInstanceBufferManager::new(
            &device,
            render_queue_family_index,
            compute_queue_family_index,
        )
        .context("Error while creating tinted block face instance buffer manager")?;
        let custom_block_instance_buffer_manager =
            CustomBlockInstanceBufferManager::new(&device)
                .context("Error while creating custom block instance buffer manager")?;
        // XXX: DEBUG
        let radiance_cascade_debug_egui_image = VulkanImage::new(
            &memory_allocator,
            &VulkanImageCreateInfo {
                image_type: VulkanImageType::Dim2d,
                format: VulkanFormat::R8G8B8A8_UNORM,
                extent: [960, 540, 1],
                usage: VulkanImageUsage::STORAGE | VulkanImageUsage::SAMPLED,
                ..Default::default()
            },
            &VulkanAllocationCreateInfo {
                memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE,
                allocate_preference: VulkanMemoryAllocatePreference::AlwaysAllocate,
                ..Default::default()
            },
        )
        .context("Error while creating radiance probe debug image")?;
        let radiance_probe_debug_egui_image_id = egui_renderer
            .register_user_image(
                &device,
                &(descriptor_set_allocator.clone() as Arc<_>),
                radiance_cascade_debug_egui_image.clone(),
                egui::TextureOptions::NEAREST,
            )
            .context("Error while registering radiance probe debug image ID")?;
        let radiance_probe_debug_info_descriptor_set_layout = VulkanDescriptorSetLayout::new(
            &device,
            &VulkanDescriptorSetLayoutCreateInfo {
                bindings: &[
                    // Block and item raw atlas colour image
                    VulkanDescriptorSetLayoutBinding {
                        binding: 0,
                        binding_flags: VulkanDescriptorBindingFlags::empty(),
                        descriptor_type: VulkanDescriptorType::SampledImage,
                        descriptor_count: 1,
                        stages: VulkanShaderStages::COMPUTE,
                        immutable_samplers: &[],
                        _ne: vulkano_non_exhaustive(),
                    },
                    // Block and item raw atlas luma image
                    VulkanDescriptorSetLayoutBinding {
                        binding: 1,
                        binding_flags: VulkanDescriptorBindingFlags::empty(),
                        descriptor_type: VulkanDescriptorType::SampledImage,
                        descriptor_count: 1,
                        stages: VulkanShaderStages::COMPUTE,
                        immutable_samplers: &[],
                        _ne: vulkano_non_exhaustive(),
                    },
                    // World TLAS
                    VulkanDescriptorSetLayoutBinding {
                        binding: 2,
                        binding_flags: VulkanDescriptorBindingFlags::empty(),
                        descriptor_type: VulkanDescriptorType::AccelerationStructure,
                        descriptor_count: 1,
                        stages: VulkanShaderStages::COMPUTE,
                        immutable_samplers: &[],
                        _ne: vulkano_non_exhaustive(),
                    },
                    // World TLAS info buffer
                    VulkanDescriptorSetLayoutBinding {
                        binding: 3,
                        binding_flags: VulkanDescriptorBindingFlags::empty(),
                        descriptor_type: VulkanDescriptorType::StorageBuffer,
                        descriptor_count: 1,
                        stages: VulkanShaderStages::COMPUTE,
                        immutable_samplers: &[],
                        _ne: vulkano_non_exhaustive(),
                    },
                    // World TLAS quads buffer
                    VulkanDescriptorSetLayoutBinding {
                        binding: 4,
                        binding_flags: VulkanDescriptorBindingFlags::empty(),
                        descriptor_type: VulkanDescriptorType::StorageBuffer,
                        descriptor_count: 1,
                        stages: VulkanShaderStages::COMPUTE,
                        immutable_samplers: &[],
                        _ne: vulkano_non_exhaustive(),
                    },
                ],
                ..Default::default()
            },
        )
        .context("Error while creating radiance probe debug info descriptor set layout")?;
        let radiance_probe_info_descriptor_set_layout = VulkanDescriptorSetLayout::new(
            &device,
            &VulkanDescriptorSetLayoutCreateInfo {
                bindings: &[
                    // Block and item raw atlas colour image
                    VulkanDescriptorSetLayoutBinding {
                        binding: 0,
                        binding_flags: VulkanDescriptorBindingFlags::empty(),
                        descriptor_type: VulkanDescriptorType::SampledImage,
                        descriptor_count: 1,
                        stages: VulkanShaderStages::COMPUTE,
                        immutable_samplers: &[],
                        _ne: vulkano_non_exhaustive(),
                    },
                    // Block and item raw atlas luma image
                    VulkanDescriptorSetLayoutBinding {
                        binding: 1,
                        binding_flags: VulkanDescriptorBindingFlags::empty(),
                        descriptor_type: VulkanDescriptorType::SampledImage,
                        descriptor_count: 1,
                        stages: VulkanShaderStages::COMPUTE,
                        immutable_samplers: &[],
                        _ne: vulkano_non_exhaustive(),
                    },
                    // World TLAS
                    VulkanDescriptorSetLayoutBinding {
                        binding: 2,
                        binding_flags: VulkanDescriptorBindingFlags::empty(),
                        descriptor_type: VulkanDescriptorType::AccelerationStructure,
                        descriptor_count: 1,
                        stages: VulkanShaderStages::COMPUTE,
                        immutable_samplers: &[],
                        _ne: vulkano_non_exhaustive(),
                    },
                    // World TLAS info buffer
                    VulkanDescriptorSetLayoutBinding {
                        binding: 3,
                        binding_flags: VulkanDescriptorBindingFlags::empty(),
                        descriptor_type: VulkanDescriptorType::StorageBuffer,
                        descriptor_count: 1,
                        stages: VulkanShaderStages::COMPUTE,
                        immutable_samplers: &[],
                        _ne: vulkano_non_exhaustive(),
                    },
                    // World TLAS quads buffer
                    VulkanDescriptorSetLayoutBinding {
                        binding: 4,
                        binding_flags: VulkanDescriptorBindingFlags::empty(),
                        descriptor_type: VulkanDescriptorType::StorageBuffer,
                        descriptor_count: 1,
                        stages: VulkanShaderStages::COMPUTE,
                        immutable_samplers: &[],
                        _ne: vulkano_non_exhaustive(),
                    },
                    // Cascade update info buffer
                    VulkanDescriptorSetLayoutBinding {
                        binding: 5,
                        binding_flags: VulkanDescriptorBindingFlags::empty(),
                        descriptor_type: VulkanDescriptorType::StorageBuffer,
                        descriptor_count: 1,
                        stages: VulkanShaderStages::COMPUTE,
                        immutable_samplers: &[],
                        _ne: vulkano_non_exhaustive(),
                    },
                    // Block face instances
                    VulkanDescriptorSetLayoutBinding {
                        binding: 6,
                        binding_flags: VulkanDescriptorBindingFlags::empty(),
                        descriptor_type: VulkanDescriptorType::StorageBuffer,
                        descriptor_count: 1,
                        stages: VulkanShaderStages::COMPUTE,
                        immutable_samplers: &[],
                        _ne: vulkano_non_exhaustive(),
                    },
                    // Light block tree
                    VulkanDescriptorSetLayoutBinding {
                        binding: 7,
                        binding_flags: VulkanDescriptorBindingFlags::empty(),
                        descriptor_type: VulkanDescriptorType::StorageBuffer,
                        descriptor_count: 1,
                        stages: VulkanShaderStages::COMPUTE,
                        immutable_samplers: &[],
                        _ne: vulkano_non_exhaustive(),
                    },
                ],
                ..Default::default()
            },
        )
        .context("Error while creating radiance probe info descriptor set layout")?;
        let radiance_probe_debug_texture_descriptor_set_layout = VulkanDescriptorSetLayout::new(
            &device,
            &VulkanDescriptorSetLayoutCreateInfo {
                bindings: &[VulkanDescriptorSetLayoutBinding {
                    binding: 0,
                    binding_flags: VulkanDescriptorBindingFlags::empty(),
                    descriptor_type: VulkanDescriptorType::StorageImage,
                    descriptor_count: 1,
                    stages: VulkanShaderStages::COMPUTE,
                    immutable_samplers: &[],
                    _ne: vulkano_non_exhaustive(),
                }],
                ..Default::default()
            },
        )
        .context("Error while creating radiance probe debug texture descriptor set layout")?;
        let radiance_cascade_debug_egui_image_view =
            VulkanImageView::new_default(&radiance_cascade_debug_egui_image)
                .context("Error while creating radiance probe debug image view")?;
        let radiance_probe_debug_texture_descriptor_set = VulkanDescriptorSet::new(
            descriptor_set_allocator.clone(),
            radiance_probe_debug_texture_descriptor_set_layout.clone(),
            [VulkanWriteDescriptorSet::image_view(
                0,
                radiance_cascade_debug_egui_image_view,
            )],
            [],
        )
        .context("Error while creating radiance probe debug texture descriptor set")?;
        let radiance_probe_lightmaps_descriptor_set_layout = VulkanDescriptorSetLayout::new(
            &device,
            &VulkanDescriptorSetLayoutCreateInfo {
                bindings: &[
                    VulkanDescriptorSetLayoutBinding {
                        binding: 0,
                        binding_flags: VulkanDescriptorBindingFlags::empty(),
                        descriptor_type: VulkanDescriptorType::StorageBuffer,
                        descriptor_count: 1,
                        stages: VulkanShaderStages::COMPUTE,
                        immutable_samplers: &[],
                        _ne: vulkano_non_exhaustive(),
                    },
                    // (
                    //     1,
                    //     VulkanDescriptorSetLayoutBinding {
                    //         binding_flags: VulkanDescriptorBindingFlags::empty(),
                    //         descriptor_type: VulkanDescriptorType::StorageBuffer,
                    //         descriptor_count: 1,
                    //         stages: VulkanShaderStages::COMPUTE,
                    //         immutable_samplers: vec![],
                    //         _ne: vulkano_non_exhaustive(),
                    //     },
                    // ),
                ],
                ..Default::default()
            },
        )
        .context("Error while creating radiance probe lightmaps descriptor set layout")?;
        let radiance_probe_debug_pipeline_layout = VulkanPipelineLayout::new(
            &device,
            &VulkanPipelineLayoutCreateInfo {
                set_layouts: &[
                    &radiance_probe_debug_info_descriptor_set_layout,
                    &radiance_probe_debug_texture_descriptor_set_layout,
                ],
                push_constant_ranges: &[VulkanPushConstantRange {
                    stages: VulkanShaderStages::COMPUTE,
                    offset: 0,
                    size: std::mem::size_of::<RadianceCascadeUpdateInfo>()
                        .try_into()
                        .unwrap(),
                }],
                ..Default::default()
            },
        )
        .context("Error while creating radiance probe debug pipeline layout")?;
        let radiance_probe_update_pipeline_layout = VulkanPipelineLayout::new(
            &device,
            &VulkanPipelineLayoutCreateInfo {
                set_layouts: &[
                    &radiance_probe_info_descriptor_set_layout,
                    &radiance_probe_lightmaps_descriptor_set_layout,
                    &matrices_descriptor_set_layout,
                ],
                push_constant_ranges: &[
                    // `update_info_idx`
                    VulkanPushConstantRange {
                        stages: VulkanShaderStages::COMPUTE,
                        offset: 0,
                        size: 4,
                    },
                ],
                ..Default::default()
            },
        )
        .context("Error while creating radiance cascade update pipeline layout")?;
        let radiance_probe_debug_pipeline = chunk_rc::compute::create_raytracing_debug_pipeline(
            &device,
            &radiance_probe_debug_pipeline_layout,
        )
        .context("Error while creating radiance probe debug pipeline")?;
        let radiance_probe_update_pipelines = chunk_rc::compute::create_cascade_update_pipelines(
            &device,
            &radiance_probe_update_pipeline_layout,
        )
        .context("Error while creating radiance probe update pipelines")?;
        // Debug pipelines
        let debug_point_pipeline = debug::point::create_graphics_pipeline(
            &device,
            &generic_block_graphics_pipeline_layout,
            &render_pass.first_subpass(),
        )?;
        let debug_line_pipeline = debug::line::create_graphics_pipeline(
            &device,
            &generic_block_graphics_pipeline_layout,
            &render_pass.first_subpass(),
        )?;
        let debug_triangle_pipeline = debug::triangle::create_graphics_pipeline(
            &device,
            &generic_block_graphics_pipeline_layout,
            &render_pass.first_subpass(),
        )?;
        Ok(Self {
            resources: GraphicsResources {
                device,
                render_queue,
                compute_queue,
                memory_allocator,
                command_buffer_allocator,
                descriptor_set_allocator,
                render_pass,
                block_registry: Arc::new(block_registry),
            },
            swapchain,
            swapchain_images,
            depth_image,
            egui_renderer,
            graphics_options,
            radiance_cascades: RadianceCascadeState {
                tlas_info: None,
                update_pipelines: radiance_probe_update_pipelines,
                update_pipeline_layout: radiance_probe_update_pipeline_layout,
                probe_debug_info_descriptor_set_layout:
                    radiance_probe_debug_info_descriptor_set_layout,
                probe_info_descriptor_set_layout: radiance_probe_info_descriptor_set_layout,
                lightmaps_descriptor_set_layout: radiance_probe_lightmaps_descriptor_set_layout,
                debug_texture_descriptor_set: radiance_probe_debug_texture_descriptor_set,
                lightmap_render_descriptor_set_layout:
                    radiance_probe_lightmap_render_descriptor_set_layout,
                debug_info: Arc::new(Mutex::new(RadianceProbeDebugInfo::default())),
                debug_egui_texture: egui::load::SizedTexture {
                    id: radiance_probe_debug_egui_image_id,
                    size: (960.0, 540.0).into(),
                },
                debug_pipeline: radiance_probe_debug_pipeline,
                debug_pipeline_layout: radiance_probe_debug_pipeline_layout,
                debug_light_tree: None,
            },
            block_graphics_pipeline,
            generic_block_graphics_pipeline_layout,
            tinted_block_graphics_pipeline,
            custom_block_graphics_pipeline,
            custom_block_vertices_buffer,
            custom_block_indices_buffer,
            camera_buffer,
            camera_descriptor_set,
            matrices_descriptor_set,
            block_item_atlas: block_item_texture_atlas,
            block_item_atlas_descriptor_set,
            buffer_managers: GraphicsBufferManagers {
                block_face_vertex: block_face_vertex_buffer_manager,
                block_face_instance: block_face_instance_buffer_manager,
                tinted_block_face_vertex: tinted_block_face_vertex_buffer_manager,
                tinted_block_face_instance: tinted_block_face_instance_buffer_manager,
                custom_block_instance: custom_block_instance_buffer_manager,
            },
            debug_point_pipeline,
            debug_line_pipeline,
            debug_triangle_pipeline,
            size,
            camera,
        })
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            (self.swapchain, self.swapchain_images) = self
                .swapchain
                .recreate(&VulkanSwapchainCreateInfo {
                    image_extent: new_size.into(),
                    present_mode: self.graphics_options.get_present_mode(),
                    ..self.swapchain.create_info()
                })
                .context("Error while recreating swapchain")
                .unwrap();
            self.depth_image = VulkanImage::new(
                &self.resources.memory_allocator,
                &VulkanImageCreateInfo {
                    image_type: VulkanImageType::Dim2d,
                    format: self.resources.render_pass.attachments()[1].format,
                    extent: [new_size.width, new_size.height, 1],
                    usage: VulkanImageUsage::DEPTH_STENCIL_ATTACHMENT,
                    ..Default::default()
                },
                &VulkanAllocationCreateInfo {
                    memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE,
                    ..Default::default()
                },
            )
            .context("Error while creating depth image")
            .unwrap();
            self.camera
                .proj_matrix
                .set_aspect((new_size.width as f32) / (new_size.height as f32));
        }
    }

    pub fn apply_new_graphics_options(&mut self, new_options: GraphicsOptions) {
        let old_options = std::mem::replace(&mut self.graphics_options, new_options);
        // Swapchain update in `resize` also updates VSync
        if new_options.vsync != old_options.vsync {
            self.resize(self.size);
        }
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

    pub fn render(
        &mut self,
        subchunks: &AHashMap<[i32; 3], chunk_rc::Subchunk>,
        loaded_chunks: &AHashSet<[i32; 2]>,
        egui_ctx: &egui::Context,
        egui_full_output: egui::output::FullOutput,
        debug_state: &DebugState,
        debug_points: &[DebugPointVertex],
        debug_lines: &[DebugLineInstance],
        debug_triangles: &[DebugTriangleInstance],
    ) -> anyhow::Result<DebugOutput> {
        let pixels_per_point = egui_full_output.pixels_per_point;
        let egui_primitives = egui_ctx.tessellate(egui_full_output.shapes, pixels_per_point);
        let mut command_buffer = VulkanAutoCommandBufferBuilder::primary(
            self.resources.command_buffer_allocator.clone(),
            self.resources.render_queue.queue_family_index(),
            VulkanCommandBufferUsage::OneTimeSubmit,
        )
        .context("Error while creating command buffer builder")?;
        command_buffer
            .update_buffer(
                self.camera_buffer.clone(),
                Box::new(self.camera.generate_reversed_depth_view_matrix_slice()),
            )
            .unwrap();
        let egui_render_data = self
            .egui_renderer
            .prepare(
                &self.resources,
                &mut command_buffer,
                &self.size,
                egui_full_output.textures_delta.set,
                egui_primitives,
                pixels_per_point,
            )
            .context("Error while preparing egui renderer")?;
        // Main render subpass
        let (swapchain_image_i, is_swapchain_suboptimal, swapchain_semaphore) = unsafe {
            let semaphore = VulkanSemaphore::from_pool(&self.resources.device)
                .map(Arc::new)
                .context("Error while creating swapchain semaphore")?;
            let acquired_image = match self
                .swapchain
                .acquire_next_image(&VulkanAcquireNextImageInfo {
                    timeout: Some(std::time::Duration::from_millis(20)),
                    semaphore: Some(&semaphore),
                    ..Default::default()
                })
                .map_err(VulkanValidated::unwrap)
            {
                Ok(image) => image,
                Err(VulkanError::OutOfDate) => {
                    self.resize(self.size);
                    return Ok(DebugOutput::default());
                }
                Err(VulkanError::Timeout) => return Ok(DebugOutput::default()),
                Err(err) => return Err(anyhow!("Error while acquiring swapchain image: {err}")),
            };
            (
                acquired_image.image_index,
                acquired_image.is_suboptimal,
                semaphore,
            )
        };
        let swapchain_image_view =
            VulkanImageView::new_default(&self.swapchain_images[swapchain_image_i as usize])
                .context("Error while creating swapchain image view")?;
        let depth_image_view = VulkanImageView::new_default(&self.depth_image)
            .context("Error while creating depth image view")?;
        let framebuffer = VulkanFramebuffer::new(
            &self.resources.render_pass,
            &VulkanFramebufferCreateInfo {
                attachments: &[&swapchain_image_view, &depth_image_view],
                ..Default::default()
            },
        )
        .context("Error while creating framebuffer object")?;
        command_buffer
            .begin_render_pass(
                VulkanRenderPassBeginInfo {
                    clear_values: vec![Some([0.471, 0.655, 1.0, 1.0].into()), Some(0.0.into())],
                    ..VulkanRenderPassBeginInfo::framebuffer(framebuffer.clone())
                },
                VulkanSubpassBeginInfo {
                    contents: VulkanSubpassContents::Inline,
                    ..Default::default()
                },
            )
            .unwrap();
        // Block rendering
        let subchunks_skipped;
        let mut subchunk_traversal_graph: Vec<([i32; 3], [i32; 3])> = Vec::new();
        let mut block_face_draw_commands_buffer = None;
        let mut tinted_block_face_draw_commands_buffer = None;
        let mut custom_block_draw_commands_buffer = None;
        {
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
            let mut block_face_draw_commands: Vec<VulkanDrawIndirectCommand> = Vec::new();
            let mut tinted_block_face_draw_commands: Vec<VulkanDrawIndirectCommand> = Vec::new();
            let mut custom_block_draw_commands: Vec<VulkanDrawIndexedIndirectCommand> = Vec::new();
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
                        block_face_draw_commands.push(VulkanDrawIndirectCommand {
                            vertex_count: 4,
                            instance_count: subchunk.block_face_instance_groups[i].1,
                            first_vertex: subchunk.block_face_start_vertices[i],
                            first_instance: subchunk.block_face_instance_groups[i].0,
                        });
                    }
                    // Tinted block faces
                    if subchunk.tinted_block_face_start_vertices[i] != u32::MAX {
                        tinted_block_face_draw_commands.push(VulkanDrawIndirectCommand {
                            vertex_count: 4,
                            instance_count: subchunk.tinted_block_face_instance_groups[i].1,
                            first_vertex: subchunk.tinted_block_face_start_vertices[i],
                            first_instance: subchunk.tinted_block_face_instance_groups[i].0,
                        });
                    }
                }
                // Custom blocks
                for group in &subchunk.custom_block_groups {
                    custom_block_draw_commands.push(VulkanDrawIndexedIndirectCommand {
                        index_count: group.start_index_and_len[1],
                        instance_count: group.start_instance_and_len[1],
                        first_index: group.start_index_and_len[0],
                        vertex_offset: group.start_vertex,
                        first_instance: group.start_instance_and_len[0],
                    });
                }
            }
            {
                let subchunk_coord_set: AHashSet<_> = subchunks.keys().copied().collect();
                subchunks_skipped = subchunk_coord_set.difference(&rendered_chunks).count()
            }
            if !block_face_draw_commands.is_empty() {
                block_face_draw_commands_buffer = Some(
                    VulkanBuffer::from_iter(
                        &self.resources.memory_allocator,
                        &VulkanBufferCreateInfo {
                            usage: VulkanBufferUsage::INDIRECT_BUFFER,
                            ..Default::default()
                        },
                        &VulkanAllocationCreateInfo {
                            memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                                | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                            ..Default::default()
                        },
                        block_face_draw_commands,
                    )
                    .context("Error while creating block face draw commands buffer")?,
                );
            }
            if !tinted_block_face_draw_commands.is_empty() {
                tinted_block_face_draw_commands_buffer = Some(
                    VulkanBuffer::from_iter(
                        &self.resources.memory_allocator,
                        &VulkanBufferCreateInfo {
                            usage: VulkanBufferUsage::INDIRECT_BUFFER,
                            ..Default::default()
                        },
                        &VulkanAllocationCreateInfo {
                            memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                                | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                            ..Default::default()
                        },
                        tinted_block_face_draw_commands,
                    )
                    .context("Error while creating tinted block face draw commands buffer")?,
                );
            }
            if !custom_block_draw_commands.is_empty() {
                custom_block_draw_commands_buffer = Some(
                    VulkanBuffer::from_iter(
                        &self.resources.memory_allocator,
                        &VulkanBufferCreateInfo {
                            usage: VulkanBufferUsage::INDIRECT_BUFFER,
                            ..Default::default()
                        },
                        &VulkanAllocationCreateInfo {
                            memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                                | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                            ..Default::default()
                        },
                        custom_block_draw_commands,
                    )
                    .context("Error while creating custom block draw commands buffer")?,
                );
            }
        }
        // let lightmap_buffers = self.buffer_managers.block_face_instance.get_lightmap_buffers();
        let lightmap_buffer = self
            .buffer_managers
            .block_face_instance
            .get_lightmap_render_buffer();
        let lightmap_render_descriptor_set = VulkanDescriptorSet::new(
            self.resources.descriptor_set_allocator.clone(),
            self.radiance_cascades
                .lightmap_render_descriptor_set_layout
                .clone(),
            [VulkanWriteDescriptorSet::buffer(0, lightmap_buffer.clone())],
            [],
        )
        .context("Error while creating matrices descriptor set")?;
        // Render blocks
        command_buffer
            // Need to bind a pipeline with compatible layout for binding descriptor sets
            .bind_pipeline_graphics(self.block_graphics_pipeline.clone())
            .unwrap()
            .bind_descriptor_sets(
                VulkanPipelineBindPoint::Graphics,
                self.generic_block_graphics_pipeline_layout.clone(),
                0,
                (
                    self.camera_descriptor_set.clone(),
                    self.block_item_atlas_descriptor_set.clone(),
                    self.matrices_descriptor_set.clone(),
                    lightmap_render_descriptor_set,
                ),
            )
            .unwrap()
            .set_viewport(
                0,
                SmallVec::from(&[VulkanViewport {
                    extent: [self.size.width as f32, self.size.height as f32],
                    ..Default::default()
                }] as &[_]),
            )
            .unwrap();
        if let Some(draw_commands_buffer) = block_face_draw_commands_buffer {
            unsafe {
                // Block graphics pipeline already bound
                command_buffer
                    // .bind_descriptor_sets(
                    //     VulkanPipelineBindPoint::Graphics,
                    //     self.generic_block_graphics_pipeline_layout.clone(),
                    //     3,
                    //     vec![lightmap_render_descriptor_set],
                    // )
                    // .unwrap()
                    .bind_vertex_buffers(
                        0,
                        (
                            self.buffer_managers.block_face_vertex.get_buffer(),
                            self.buffer_managers.block_face_instance.get_buffer(),
                        ),
                    )
                    .unwrap()
                    .draw_indirect(draw_commands_buffer)
                    .unwrap();
            }
        }
        if let Some(draw_commands_buffer) = tinted_block_face_draw_commands_buffer {
            unsafe {
                command_buffer
                    .bind_pipeline_graphics(self.tinted_block_graphics_pipeline.clone())
                    .unwrap()
                    .bind_vertex_buffers(
                        0,
                        (
                            self.buffer_managers.tinted_block_face_vertex.get_buffer(),
                            self.buffer_managers.tinted_block_face_instance.get_buffer(),
                        ),
                    )
                    .unwrap()
                    .draw_indirect(draw_commands_buffer)
                    .unwrap();
            }
        }
        if let Some(draw_commands_buffer) = custom_block_draw_commands_buffer {
            unsafe {
                command_buffer
                    .bind_pipeline_graphics(self.custom_block_graphics_pipeline.clone())
                    .unwrap()
                    .bind_vertex_buffers(
                        0,
                        (
                            self.custom_block_vertices_buffer.clone(),
                            self.buffer_managers.custom_block_instance.get_buffer(),
                        ),
                    )
                    .unwrap()
                    .bind_index_buffer(self.custom_block_indices_buffer.clone())
                    .unwrap()
                    .draw_indexed_indirect(draw_commands_buffer)
                    .unwrap();
            }
        }
        // Render debug graphics
        let mut debug_point_buffer = None;
        let mut debug_line_buffer = None;
        let mut debug_triangle_buffer = None;
        if !debug_points.is_empty() {
            debug_point_buffer = Some(
                VulkanBuffer::from_iter(
                    &self.resources.memory_allocator,
                    &VulkanBufferCreateInfo {
                        usage: VulkanBufferUsage::VERTEX_BUFFER,
                        ..Default::default()
                    },
                    &VulkanAllocationCreateInfo {
                        memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                            | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                        ..Default::default()
                    },
                    debug_points.iter().copied(),
                )
                .context("Error while creating debug point vertex buffer")?,
            );
        }
        if !debug_lines.is_empty() {
            debug_line_buffer = Some(
                VulkanBuffer::from_iter(
                    &self.resources.memory_allocator,
                    &VulkanBufferCreateInfo {
                        usage: VulkanBufferUsage::VERTEX_BUFFER,
                        ..Default::default()
                    },
                    &VulkanAllocationCreateInfo {
                        memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                            | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                        ..Default::default()
                    },
                    debug_lines.iter().copied(),
                )
                .context("Error while creating debug line instance buffer")?,
            );
        }
        if !debug_triangles.is_empty() {
            debug_triangle_buffer = Some(
                VulkanBuffer::from_iter(
                    &self.resources.memory_allocator,
                    &VulkanBufferCreateInfo {
                        usage: VulkanBufferUsage::VERTEX_BUFFER,
                        ..Default::default()
                    },
                    &VulkanAllocationCreateInfo {
                        memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                            | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                        ..Default::default()
                    },
                    debug_triangles.iter().copied(),
                )
                .context("Error while creating debug triangle instance buffer")?,
            );
        }
        if let Some(buffer) = debug_point_buffer {
            unsafe {
                command_buffer
                    .bind_pipeline_graphics(self.debug_point_pipeline.clone())
                    .unwrap()
                    .bind_vertex_buffers(0, (buffer,))
                    .unwrap()
                    .draw(debug_points.len().try_into().unwrap(), 1, 0, 0)
                    .unwrap();
            }
        }
        if let Some(buffer) = debug_line_buffer {
            unsafe {
                command_buffer
                    .bind_pipeline_graphics(self.debug_line_pipeline.clone())
                    .unwrap()
                    .bind_vertex_buffers(0, (buffer,))
                    .unwrap()
                    .draw(2, debug_lines.len().try_into().unwrap(), 0, 0)
                    .unwrap();
            }
        }
        if let Some(buffer) = debug_triangle_buffer {
            unsafe {
                command_buffer
                    .bind_pipeline_graphics(self.debug_triangle_pipeline.clone())
                    .unwrap()
                    .bind_vertex_buffers(0, (buffer,))
                    .unwrap()
                    .draw(3, debug_triangles.len().try_into().unwrap(), 0, 0)
                    .unwrap();
            }
        }
        // Render egui UI
        if let Some(egui_render_data) = egui_render_data {
            self.egui_renderer
                .render(&mut command_buffer, self.size, egui_render_data);
        }
        command_buffer
            .end_render_pass(VulkanSubpassEndInfo::default())
            .unwrap();
        // Submit command buffer to GPU
        let built_command_buffer = command_buffer.build().unwrap();
        // vulkano::sync::now(self.resources.device.clone())
        //     .join(swapchain_image_future)
        //     .then_execute(self.resources.queues[0].clone(), built_command_buffer)
        //     .unwrap()
        //     .then_swapchain_present(
        //         self.resources.queues[0].clone(),
        //         VulkanSwapchainPresentInfo::swapchain_image_index(
        //             self.swapchain.clone(),
        //             swapchain_image_i,
        //         ),
        //     )
        //     .then_signal_fence_and_flush()
        //     .unwrap()
        //     .wait(None)
        //     .unwrap();
        {
            self.resources.render_queue.with(|mut queue_guard| unsafe {
                queue_guard.wait_idle().unwrap();
                let render_semaphore = VulkanSemaphore::from_pool(&self.resources.device)
                    .map(Arc::new)
                    .context("Error while creating render semaphore")
                    .unwrap();
                let finish_fence = VulkanFence::from_pool(&self.resources.device)
                    .map(Arc::new)
                    .context("Error while creating render fence")
                    .unwrap();
                queue_guard
                    .submit(
                        &[VulkanSubmitInfo {
                            command_buffers: vec![VulkanCommandBufferSubmitInfo::new(
                                built_command_buffer.clone(),
                            )],
                            wait_semaphores: vec![VulkanSemaphoreSubmitInfo::new(
                                swapchain_semaphore,
                            )],
                            signal_semaphores: vec![
                                VulkanSemaphoreSubmitInfo::new(render_semaphore.clone()),
                                // VulkanSemaphoreSubmitInfo::new(render_semaphore_2.clone()),
                            ],
                            ..Default::default()
                        }],
                        // None,
                        Some(&finish_fence),
                    )
                    .unwrap();
                queue_guard
                    .present(&VulkanPresentInfo {
                        wait_semaphores: vec![VulkanSemaphorePresentInfo::new(render_semaphore)],
                        swapchain_infos: vec![VulkanSwapchainPresentInfo::new(
                            self.swapchain.clone(),
                            swapchain_image_i,
                        )],
                        ..Default::default()
                    })
                    .unwrap()
                    .for_each(|result| _ = result.unwrap());
                finish_fence.wait(None).unwrap();
            })
        }
        self.egui_renderer
            .free_textures(&egui_full_output.textures_delta.free);
        if is_swapchain_suboptimal {
            (self.swapchain, self.swapchain_images) = self
                .swapchain
                .recreate(&self.swapchain.create_info())
                .context("Error while recreating swapchain")?;
        }
        Ok(DebugOutput {
            subchunks_culled: subchunks_skipped,
            subchunk_traversal_graph,
        })
    }

    pub fn radiance_cascades_debug_render(
        &mut self,
        subchunks: &AHashMap<[i32; 3], chunk_rc::Subchunk>,
    ) {
        // XXX: DEBUG
        // TODO: Clear output image if there's nothing to display
        if subchunks.is_empty() {
            return;
        }
        let Some(tlas_info) = self.radiance_cascades.tlas_info.clone() else {
            return;
        };
        let radiance_probe_info_descriptor_set = VulkanDescriptorSet::new(
            self.resources.descriptor_set_allocator.clone(),
            self.radiance_cascades
                .probe_debug_info_descriptor_set_layout
                .clone(),
            [
                VulkanWriteDescriptorSet::image_view(0, self.block_item_atlas.view.clone()),
                VulkanWriteDescriptorSet::image_view(1, self.block_item_atlas.luma_view.clone()),
                VulkanWriteDescriptorSet::acceleration_structure(2, tlas_info.world_tlas),
                VulkanWriteDescriptorSet::buffer(3, tlas_info.instance_info_buffer),
                VulkanWriteDescriptorSet::buffer(4, tlas_info.quads_info_buffer),
            ],
            [],
        )
        .context("Error while creating radiance probe info descriptor set")
        .unwrap();
        let queue = &self.resources.compute_queue;
        // let queue = &self.resources.queues[0];
        let mut command_buffer = VulkanAutoCommandBufferBuilder::primary(
            self.resources.command_buffer_allocator.clone(),
            queue.queue_family_index(),
            VulkanCommandBufferUsage::OneTimeSubmit,
        )
        .context("Error while creating radiance probe debug command buffer builder")
        .unwrap();
        unsafe {
            command_buffer
                .bind_pipeline_compute(self.radiance_cascades.debug_pipeline.clone())
                .unwrap()
                .bind_descriptor_sets(
                    VulkanPipelineBindPoint::Compute,
                    self.radiance_cascades.debug_pipeline_layout.clone(),
                    0,
                    (
                        radiance_probe_info_descriptor_set,
                        self.radiance_cascades.debug_texture_descriptor_set.clone(),
                    ),
                )
                .unwrap()
                .push_constants(
                    self.radiance_cascades.debug_pipeline_layout.clone(),
                    0,
                    RadianceCascadeUpdateInfo {
                        inv_view_matrix: *self
                            .camera
                            .generate_view_matrix()
                            .try_inverse()
                            .unwrap()
                            .as_ref(),
                    },
                )
                .unwrap()
                .dispatch([960 / 16, 540 / 4, 1])
                // .dispatch([1, 1, 1])
                .unwrap();
        }
        let built_command_buffer = command_buffer.build().unwrap();
        vulkano::sync::now(self.resources.device.clone())
            .then_execute(queue.clone(), built_command_buffer)
            .unwrap()
            .flush()
            .unwrap();
    }

    pub fn update_all_subchunks_radiance_lighting(
        &mut self,
        thread_pool: &ThreadPool,
        subchunks: &AHashMap<[i32; 3], chunk_rc::Subchunk>,
        raw_chunks: &AHashMap<[i32; 2], Arc<RawChunk>>,
    ) {
        if subchunks.is_empty() {
            return;
        }
        let Some(tlas_info) = self.radiance_cascades.tlas_info.clone() else {
            return;
        };
        let queue = self.resources.compute_queue.clone();
        let mut command_buffer = VulkanAutoCommandBufferBuilder::primary(
            self.resources.command_buffer_allocator.clone(),
            queue.queue_family_index(),
            VulkanCommandBufferUsage::OneTimeSubmit,
        )
        .context("Error while creating radiance probe update command buffer builder")
        .unwrap();
        let (update_info_buffer, num_updates, update_lengths, _buffer_copy_regions) = {
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
            let mut buffer_copy_regions: SmallVec<[VulkanBufferCopy; 1]> = SmallVec::new();
            for subchunk in subchunks.values() {
                // XXX: DEBUG
                let subchunk_list = [
                    [0, 112, -16],
                    [0, 112, 0],
                    [-16, 112, 0],
                    [-16, 112, -16],
                    [0, 144, -32],
                    [16, 144, -32],
                    [16, 144, -16],
                    [0, 144, -16],
                ];
                if !subchunk_list.contains(&subchunk.start_coords) {
                    continue;
                }
                // if subchunk.start_coords[1] < 60 {
                //     continue;
                // }
                for dir_i in 0..6 {
                    if subchunk.block_face_start_vertices[dir_i] != u32::MAX {
                        let instance_group = &subchunk.block_face_instance_groups[dir_i];
                        updates.push(UpdateInfo {
                            subchunk_start_coords: subchunk.start_coords.map(|n| n as f32),
                            faces_start: instance_group.0,
                            faces_len: instance_group.1,
                            faces_dir_i: dir_i.try_into().unwrap(),
                        });
                        let instance_byte_size =
                            std::mem::size_of::<chunk_rc::block_face::Instance>()
                                as VulkanDeviceSize;
                        let byte_offset =
                            (instance_group.0 as VulkanDeviceSize) * instance_byte_size;
                        buffer_copy_regions.push(VulkanBufferCopy {
                            src_offset: byte_offset,
                            dst_offset: byte_offset,
                            size: (instance_group.1 as VulkanDeviceSize) * instance_byte_size,
                            ..Default::default()
                        });
                        let dispatch_width = instance_group.1;
                        if dispatch_width > max_dispatch_width {
                            max_dispatch_width = dispatch_width;
                        }
                    }
                }
            }
            let num_updates: u32 = updates.len().try_into().unwrap();
            let update_lengths: Vec<u32> = updates
                .iter()
                .map(|update_info| update_info.faces_len)
                .collect();
            // let update_info_buffer = VulkanBuffer::from_iter(
            //     self.resources.memory_allocator.clone(),
            //     VulkanBufferCreateInfo {
            //         usage: VulkanBufferUsage::STORAGE_BUFFER,
            //         ..Default::default()
            //     },
            //     VulkanAllocationCreateInfo {
            //         memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
            //             | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
            //         ..Default::default()
            //     },
            //     updates,
            // )
            // .context("Error while creating update info buffer")
            // .unwrap();
            let update_info_buffer = vulkan_new_buffer_slice(
                &(self.resources.memory_allocator.clone() as Arc<_>),
                &VulkanBufferCreateInfo {
                    usage: VulkanBufferUsage::STORAGE_BUFFER | VulkanBufferUsage::TRANSFER_DST,
                    ..Default::default()
                },
                &VulkanAllocationCreateInfo {
                    memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE,
                    ..Default::default()
                },
                updates.len().try_into().unwrap(),
            )
            .context("Error while creating update info buffer")
            .unwrap();
            command_buffer
                .update_buffer(update_info_buffer.clone(), updates.into_boxed_slice())
                .unwrap();
            (
                update_info_buffer,
                num_updates,
                update_lengths,
                buffer_copy_regions,
            )
        };
        // Find all light-emitting blocks, organise into a tree
        let light_tree_buffer = {
            const EMISSION_TO_BRIGHTNESS_COEF: f32 = 1.0;
            const BRIGHTNESS_TO_MAX_RADIUS_COEF: f32 = 1.2;
            #[derive(Clone, Copy, Debug, Default)]
            struct RawLightNode {
                pub sphere_centre: Point3<f32>,
                pub sphere_radius: f32,
                pub brightness: f32,
                pub aabb: AABB,
                // pub children: Option<Box<[RawLightNode; 2]>>,
                pub children: Option<[u32; 2]>,
            }
            let mut nodes: Vec<RawLightNode> = Vec::new();
            // Find all light emitting blocks in the world, add as base nodes.
            for (chunk_xz, raw_chunk) in raw_chunks {
                for (subchunk_yi_usize, raw_subchunk) in raw_chunk.sections.iter().enumerate() {
                    let subchunk_yi: i32 = subchunk_yi_usize.try_into().unwrap();
                    if raw_subchunk.block_count == 0 {
                        continue;
                    }
                    let subchunk_start_x = chunk_xz[0] * SUBCHUNK_AXIS_LEN_I32;
                    let subchunk_start_y = (subchunk_yi * SUBCHUNK_AXIS_LEN_I32) + MIN_HEIGHT_I32;
                    let subchunk_start_z = chunk_xz[1] * SUBCHUNK_AXIS_LEN_I32;
                    let block_registry = &self.resources.block_registry;
                    let subchunk_light_nodes = raw_subchunk
                        .block_states
                        .get_all_of_types(block_registry.light_emitting_blockstates())
                        .into_iter()
                        .map(|([x_u8, y_u8, z_u8], blockstate_idx)| {
                            use resource::block::blockstate::ModelData;
                            use resource::block::model::ModelType;
                            let global_x_f32 = (subchunk_start_x + i32::from(x_u8)) as f32;
                            let global_y_f32 = (subchunk_start_y + i32::from(y_u8)) as f32;
                            let global_z_f32 = (subchunk_start_z + i32::from(z_u8)) as f32;
                            let global_pos = Point3::new(
                                global_x_f32,
                                global_y_f32,
                                global_z_f32,
                            );
                            let blockstate = &block_registry[blockstate_idx];
                            let emission_level = blockstate
                                .extra_info
                                .light_info
                                .emission_level;
                            debug_assert_ne!(emission_level, 0);
                            let brightness =
                                (emission_level as f32).powi(2) * EMISSION_TO_BRIGHTNESS_COEF;
                            let sphere_radius = brightness.sqrt() * BRIGHTNESS_TO_MAX_RADIUS_COEF;
                            // TODO: Add custom AABBs for some blocks
                            fn model_to_aabb(model: &ModelType) -> AABB {
                                match model {
                                    ModelType::Other(other_model) => {
                                        let vertices = &other_model.vertices;
                                        let mut corner_1 = vertices[0].local_pos.coords;
                                        let mut corner_2 = vertices[0].local_pos.coords;
                                        for vertex in vertices {
                                            corner_1 = corner_1.inf(&vertex.local_pos.coords);
                                            corner_2 = corner_2.sup(&vertex.local_pos.coords);
                                        }
                                        let offset = Vector3::repeat(0.5);
                                        AABB {
                                            corner_1: Point3::from(corner_1) + offset,
                                            corner_2: Point3::from(corner_2) + offset,
                                        }
                                    }
                                    _ => AABB {
                                        corner_1: Point3::origin(),
                                        corner_2: Point3::new(1.0, 1.0, 1.0),
                                    },
                                }
                            }
                            let local_aabb = match &blockstate.model_data {
                                ModelData::Single(model) => model_to_aabb(model.as_ref()),
                                ModelData::RandomChoice(models) => models
                                    .iter()
                                    .map(|model| model_to_aabb(model.model.as_ref()))
                                    .reduce(|acc, aabb| acc.max(&aabb))
                                    .unwrap(),
                            };
                            let aabb = local_aabb + global_pos.coords;
                            RawLightNode {
                                sphere_centre: global_pos + Vector3::repeat(0.5),
                                sphere_radius,
                                brightness,
                                aabb,
                                children: None,
                            }
                        });
                    nodes.extend(subchunk_light_nodes);
                }
            }
            // TODO
            // - Go through all pairs in list, test for sphere combining.
            // - If we find a valid pair, add to back of list, and restart pair search.
            // - (Note that we'll definitely need a faster method later for pair search, but we can
            //    do that once we've found a good distance for chunking.)
            // - If a full run of pair search ends, then we're done, and we're only left with root
            //   nodes.
            // NOTES
            // - We'll definitely need a faster method later for pair search, but we can do that
            //   once we've found a good distance for chunking.
            // - It's probably best to find the pair with the smallest valid combined sphere, so
            //   that we end up with more balanced trees.
            // XXX: DEBUG
            dbg!(nodes.len());
            let start_time = std::time::Instant::now();
            if !nodes.is_empty() {
                fn process_sublist(
                    current_depth: usize,
                    current_sublist_start_i: usize,
                    sublist: &mut [RawLightNode],
                    branch_nodes_start_i: usize,
                    branch_nodes: &mut [RawLightNode],
                    current_branch_node_i: &mut usize,
                    positive_diff_sum: &mut f32,
                    negative_diff_sum: &mut f32,
                ) -> (usize, RawLightNode) {
                    let key_function: fn(&RawLightNode) -> NotNan<f32> = match current_depth % 3 {
                        0 => |node| NotNan::new(node.sphere_centre.x).unwrap(),
                        1 => |node| NotNan::new(node.sphere_centre.y).unwrap(),
                        2 => |node| NotNan::new(node.sphere_centre.z).unwrap(),
                        _ => unreachable!(),
                    };
                    sublist.sort_unstable_by_key(key_function);
                    if sublist.len() == 1 {
                        (current_sublist_start_i, sublist[0])
                    } else {
                        let pivot_i = sublist.len() / 2;
                        let (left_sublist, right_sublist) = sublist.split_at_mut(pivot_i);
                        let (left_child_i, left_child) = process_sublist(
                            current_depth + 1,
                            current_sublist_start_i,
                            left_sublist,
                            branch_nodes_start_i,
                            branch_nodes,
                            current_branch_node_i,
                            positive_diff_sum,
                            negative_diff_sum,
                        );
                        let (right_child_i, right_child) = process_sublist(
                            current_depth + 1,
                            current_sublist_start_i + pivot_i,
                            right_sublist,
                            branch_nodes_start_i,
                            branch_nodes,
                            current_branch_node_i,
                            positive_diff_sum,
                            negative_diff_sum,
                        );
                        let tree_node_i = *current_branch_node_i;
                        *current_branch_node_i += 1;
                        let tree_node = &mut branch_nodes[tree_node_i];
                        {
                            let node_dist =
                                (left_child.sphere_centre - right_child.sphere_centre).magnitude();
                            let threshold_radius =
                                (node_dist + left_child.sphere_radius + right_child.sphere_radius)
                                    / 2.0;
                            let brightness = left_child.brightness + right_child.brightness;
                            let radius = brightness.sqrt() * BRIGHTNESS_TO_MAX_RADIUS_COEF;
                            if radius >= threshold_radius {
                                *positive_diff_sum += radius - threshold_radius;
                            } else {
                                *negative_diff_sum += threshold_radius - radius;
                            }
                            *tree_node = RawLightNode {
                                sphere_centre: Point3::from(
                                    (left_child.sphere_centre.coords
                                        + right_child.sphere_centre.coords)
                                        / 2.0,
                                ),
                                sphere_radius: f32::max(radius, threshold_radius),
                                brightness,
                                aabb: AABB::max(&left_child.aabb, &right_child.aabb),
                                children: Some([
                                    left_child_i.try_into().unwrap(),
                                    right_child_i.try_into().unwrap(),
                                ]),
                            };
                        }
                        (branch_nodes_start_i + tree_node_i, *tree_node)
                    }
                }
                // Add dummy nodes to nodes list, which will be replaced with branch nodes.
                let branch_nodes_start_i = nodes.len();
                // XXX: DEBUG
                {
                    let num_branch_layers =
                        (usize::BITS - nodes.len().saturating_sub(1).leading_zeros()) as usize;
                    let max_depth = num_branch_layers + 1;
                    dbg!(max_depth);
                }
                let num_branch_nodes = if nodes.len() == 1 {
                    0
                } else {
                    let mut num_current_layer_nodes = nodes.len().div_ceil(2);
                    let mut sum = 0;
                    while num_current_layer_nodes > 1 {
                        sum += num_current_layer_nodes;
                        num_current_layer_nodes = num_current_layer_nodes.div_ceil(2);
                    }
                    sum + 1
                };
                nodes.extend(std::iter::repeat_n(
                    RawLightNode::default(),
                    num_branch_nodes,
                ));
                let (leaf_nodes, branch_nodes) = nodes.split_at_mut(branch_nodes_start_i);
                let mut positive_diff_sum = 0.0;
                let mut negative_diff_sum = 0.0;
                let mut current_branch_node_i = 0;
                process_sublist(
                    0,
                    0,
                    leaf_nodes,
                    branch_nodes_start_i,
                    branch_nodes,
                    // &mut 0,
                    &mut current_branch_node_i,
                    &mut positive_diff_sum,
                    &mut negative_diff_sum,
                );
                dbg!(positive_diff_sum, negative_diff_sum);
                // HACK: The number of branch nodes we allocate seems to be a bit too big?
                nodes.drain(branch_nodes_start_i + current_branch_node_i..);
                dbg!(nodes.len(), branch_nodes_start_i, current_branch_node_i);
            }
            // XXX: DEBUG
            println!(
                "Tree construction took {:?}",
                std::time::Instant::now() - start_time
            );
            // Convert nodes
            let converted_light_nodes: Vec<LightNode> = nodes
                .into_iter()
                .map(|node| LightNode {
                    sphere_centre: node.sphere_centre.into(),
                    sphere_radius: node.sphere_radius,
                    aabb_corner_1: node.aabb.corner_1.into(),
                    aabb_corner_2: node.aabb.corner_2.into(),
                    children: node.children.unwrap_or([u32::MAX; 2]),
                })
                .collect();
            dbg!(converted_light_nodes[converted_light_nodes.len() - 1]);
            // XXX: DEBUG
            {
                // Write tree representation to file
                use std::fmt::Write;
                let mut tree_string = String::new();
                let mut current_indent: usize = 0;
                let mut node_stack: Vec<(usize, LightNode)> = Vec::new();
                let mut max_node_stack_len = 0;
                let mut current_node: LightNode =
                    converted_light_nodes[converted_light_nodes.len() - 1];
                loop {
                    if current_node.children[0] == u32::MAX {
                        // Leaf node
                        writeln!(
                            &mut tree_string,
                            "{:current_indent$}Leaf(sph: ({:.2?}, {:.2}), aabb: [{:.2?}, {:.2?}])",
                            "",
                            current_node.sphere_centre,
                            current_node.sphere_radius,
                            current_node.aabb_corner_1,
                            current_node.aabb_corner_2,
                        )
                        .unwrap();
                        if let Some((new_indent, parent_node)) = node_stack.pop() {
                            current_node = parent_node;
                            current_indent = new_indent;
                        } else {
                            break;
                        }
                    } else {
                        // Branch node
                        writeln!(
                            &mut tree_string,
                            "{:current_indent$}Branch(sph: ({:.2?}, {:.2}), aabb: [{:.2?}, {:.2?}]):",
                            "",
                            current_node.sphere_centre,
                            current_node.sphere_radius,
                            current_node.aabb_corner_1,
                            current_node.aabb_corner_2,
                        )
                        .unwrap();
                        // Push right child to stack, switch to left child
                        current_indent += 1;
                        let left_child_i = current_node.children[0] as usize;
                        let right_child_i = current_node.children[1] as usize;
                        let left_child = converted_light_nodes[left_child_i];
                        let right_child = converted_light_nodes[right_child_i];
                        node_stack.push((current_indent, right_child));
                        max_node_stack_len = max_node_stack_len.max(node_stack.len());
                        current_node = left_child;
                    }
                }
                std::fs::write("temp/lt_nodes.txt", &tree_string).unwrap();
                dbg!(max_node_stack_len);
            }
            let light_tree_buffer = VulkanBuffer::from_iter(
                &self.resources.memory_allocator,
                &VulkanBufferCreateInfo {
                    usage: VulkanBufferUsage::STORAGE_BUFFER | VulkanBufferUsage::TRANSFER_DST,
                    ..Default::default()
                },
                &VulkanAllocationCreateInfo {
                    memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                        | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                converted_light_nodes.clone(),
            )
            .context("Error while creating light node buffer")
            .unwrap();
            self.radiance_cascades.debug_light_tree = Some(converted_light_nodes);
            light_tree_buffer
        };
        let radiance_probe_info_descriptor_set = VulkanDescriptorSet::new(
            self.resources.descriptor_set_allocator.clone(),
            self.radiance_cascades
                .probe_info_descriptor_set_layout
                .clone(),
            [
                VulkanWriteDescriptorSet::image_view(0, self.block_item_atlas.view.clone()),
                VulkanWriteDescriptorSet::image_view(1, self.block_item_atlas.luma_view.clone()),
                VulkanWriteDescriptorSet::acceleration_structure(2, tlas_info.world_tlas),
                VulkanWriteDescriptorSet::buffer(3, tlas_info.instance_info_buffer),
                VulkanWriteDescriptorSet::buffer(4, tlas_info.quads_info_buffer),
                VulkanWriteDescriptorSet::buffer(5, update_info_buffer.clone()),
                VulkanWriteDescriptorSet::buffer(
                    6,
                    self.buffer_managers.block_face_instance.get_buffer(),
                ),
                VulkanWriteDescriptorSet::buffer(7, light_tree_buffer.clone()),
            ],
            [],
        )
        .context("Error while creating radiance probe info descriptor set")
        .unwrap();
        // let lightmap_buffers = self.buffer_managers.block_face_instance.get_lightmap_buffers();
        let lightmap_buffer = self
            .buffer_managers
            .block_face_instance
            .get_lightmap_compute_buffer();
        let cascade_0_lightmaps_descriptor_set = VulkanDescriptorSet::new(
            self.resources.descriptor_set_allocator.clone(),
            self.radiance_cascades
                .lightmaps_descriptor_set_layout
                .clone(),
            [
                VulkanWriteDescriptorSet::buffer(0, lightmap_buffer.clone()),
                // VulkanWriteDescriptorSet::buffer(0, lightmap_buffers[0].clone()),
                // VulkanWriteDescriptorSet::buffer(1, lightmap_buffers[1].clone()),
            ],
            [],
        )
        .context("Error while creating cascade 0 lightmaps descriptor set")
        .unwrap();
        // let cascade_1_lightmaps_descriptor_set = VulkanDescriptorSet::new(
        //     self.resources.descriptor_set_allocator.clone(),
        //     self.radiance_cascades
        //         .lightmaps_descriptor_set_layout
        //         .clone(),
        //     [
        //         VulkanWriteDescriptorSet::buffer(0, lightmap_buffers[1].clone()),
        //         VulkanWriteDescriptorSet::buffer(1, lightmap_buffers[0].clone()),
        //     ],
        //     [],
        // )
        // .context("Error while creating cascade 1 lightmaps descriptor set")
        // .unwrap();
        // let queue = &self.resources.queues[0];
        // let mut command_buffer = VulkanRecordingCommandBuffer::new(
        //     self.resources.command_buffer_allocator.clone(),
        //     queue.queue_family_index(),
        //     VulkanCommandBufferLevel::Primary,
        //     VulkanCommandBufferBeginInfo {
        //         usage: VulkanCommandBufferUsage::OneTimeSubmit,
        //         ..Default::default()
        //     },
        // )
        // .unwrap();
        // let built_command_buffer = unsafe {
        //     command_buffer
        //         .bind_pipeline_compute(&self.radiance_cascades.update_pipelines[0])
        //         .unwrap()
        //         .bind_descriptor_sets(
        //             VulkanPipelineBindPoint::Compute,
        //             &self.radiance_cascades.update_pipeline_layout,
        //             0,
        //             &[
        //                 radiance_probe_info_descriptor_set.as_raw(),
        //                 cascade_0_lightmaps_descriptor_set.as_raw(),
        //                 self.matrices_descriptor_set.as_raw(),
        //             ],
        //             &[],
        //         )
        //         .unwrap()
        //         .dispatch([max_dispatch_width, num_updates, 1])
        //         .unwrap();
        //     command_buffer.end().unwrap()
        // };
        // XXX: DEBUG
        // - Dispatch initial copy commands, before we do individual updates.
        // - Doing this for testing, because currently updates can take so long we trigger TDR.
        {
            let built_command_buffer = command_buffer.build().unwrap();
            let device = &self.resources.device;
            let fence = Arc::new(VulkanFence::from_pool(device).unwrap());
            queue.with(|mut queue_guard| unsafe {
                queue_guard
                    .submit(
                        &[VulkanSubmitInfo {
                            command_buffers: vec![VulkanCommandBufferSubmitInfo::new(
                                built_command_buffer.clone(),
                            )],
                            ..Default::default()
                        }],
                        Some(&fence),
                    )
                    .unwrap();
                });
            fence.wait(None).unwrap();
        }
        let mut copy_command_buffer = VulkanAutoCommandBufferBuilder::primary(
            self.resources.command_buffer_allocator.clone(),
            queue.queue_family_index(),
            VulkanCommandBufferUsage::OneTimeSubmit,
        )
        .context("Error while creating radiance probe copy command buffer builder")
        .unwrap();
        copy_command_buffer
            // .copy_buffer(VulkanCopyBufferInfo {
            //     src_buffer: lightmap_buffer.clone(),
            //     dst_buffer: self.buffer_managers.block_face_instance.get_lightmap_render_buffer().clone(),
            //     regions: buffer_copy_regions,
            //     _ne: vulkano_non_exhaustive(),
            // })
            .copy_buffer(VulkanCopyBufferInfo::new(
                lightmap_buffer.clone(),
                self.buffer_managers
                    .block_face_instance
                    .get_lightmap_render_buffer()
                    .clone(),
            ))
            .unwrap();
        let built_copy_command_buffer = copy_command_buffer.build().unwrap();
        {
            let device = self.resources.device.clone();
            let command_buffer_allocator = self.resources.command_buffer_allocator.clone();
            let update_pipeline = self.radiance_cascades.update_pipelines[0].clone();
            let update_pipeline_layout = self.radiance_cascades.update_pipeline_layout.clone();
            let matrices_descriptor_set = self.matrices_descriptor_set.clone();
            thread_pool.execute(move || {
                for update_i in 0..num_updates {
                    let fence = Arc::new(VulkanFence::from_pool(&device).unwrap());
                    let mut command_buffer = VulkanAutoCommandBufferBuilder::primary(
                        command_buffer_allocator.clone(),
                        queue.queue_family_index(),
                        VulkanCommandBufferUsage::OneTimeSubmit,
                    )
                    .context("Error while creating radiance probe update command buffer builder")
                    .unwrap();
                    unsafe {
                        command_buffer
                            .bind_pipeline_compute(update_pipeline.clone())
                            .unwrap()
                            .bind_descriptor_sets(
                                VulkanPipelineBindPoint::Compute,
                                update_pipeline_layout.clone(),
                                0,
                                (
                                    radiance_probe_info_descriptor_set.clone(),
                                    cascade_0_lightmaps_descriptor_set.clone(),
                                    matrices_descriptor_set.clone(),
                                ),
                            )
                            .unwrap()
                            .push_constants(update_pipeline_layout.clone(), 0, update_i)
                            .unwrap()
                            .dispatch([update_lengths[update_i as usize], 1, 1])
                            .unwrap();
                    }
                    let built_command_buffer = command_buffer.build().unwrap();
                    queue.with(|mut queue_guard| unsafe {
                        queue_guard
                            .submit(
                                &[VulkanSubmitInfo {
                                    command_buffers: vec![VulkanCommandBufferSubmitInfo::new(
                                        built_command_buffer.clone(),
                                    )],
                                    ..Default::default()
                                }],
                                Some(&fence),
                            )
                            .unwrap();
                    });
                    fence.wait(None).unwrap();
                    drop(built_command_buffer);
                }
                let fence = Arc::new(VulkanFence::from_pool(&device).unwrap());
                queue.with(|mut queue_guard| unsafe {
                    queue_guard
                        .submit(
                            &[VulkanSubmitInfo {
                                command_buffers: vec![VulkanCommandBufferSubmitInfo::new(
                                    built_copy_command_buffer,
                                )],
                                ..Default::default()
                            }],
                            Some(&fence),
                        )
                        .unwrap();
                });
                drop(radiance_probe_info_descriptor_set);
                drop(cascade_0_lightmaps_descriptor_set);
                drop(update_info_buffer);
            });
        }
    }
}

#[derive(Debug)]
pub struct TextureAtlas {
    pub image: Arc<VulkanImage>,
    pub luma_image: Arc<VulkanImage>,
    pub view: Arc<VulkanImageView>,
    pub luma_view: Arc<VulkanImageView>,
    pub sampler: Arc<VulkanSampler>,
}

impl TextureAtlas {
    pub fn from_builder(
        builder: crate::resource::texture::AtlasBuilder,
        device: &Arc<VulkanDevice>,
        queue: &Arc<VulkanQueue>,
        memory_allocator: &Arc<dyn VulkanMemoryAllocator>,
        command_buffer_allocator: Arc<dyn VulkanCommandBufferAllocator>,
    ) -> anyhow::Result<Self> {
        let (width, height) = (builder.texture.width(), builder.texture.height());
        let bytes = builder.texture.into_vec();
        let luma_bytes = builder.luma_texture.into_vec();
        let image = VulkanImage::new(
            &memory_allocator,
            &VulkanImageCreateInfo {
                image_type: VulkanImageType::Dim2d,
                format: VulkanFormat::R8G8B8A8_SRGB,
                extent: [width, height, 1],
                usage: VulkanImageUsage::SAMPLED | VulkanImageUsage::TRANSFER_DST,
                ..Default::default()
            },
            &VulkanAllocationCreateInfo {
                memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE,
                allocate_preference: VulkanMemoryAllocatePreference::AlwaysAllocate,
                ..Default::default()
            },
        )
        .context("Failed while creating atlas image")?;
        let luma_image = VulkanImage::new(
            &memory_allocator,
            &VulkanImageCreateInfo {
                image_type: VulkanImageType::Dim2d,
                format: VulkanFormat::R8_UNORM,
                extent: [width, height, 1],
                usage: VulkanImageUsage::SAMPLED | VulkanImageUsage::TRANSFER_DST,
                ..Default::default()
            },
            &VulkanAllocationCreateInfo {
                memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE,
                allocate_preference: VulkanMemoryAllocatePreference::AlwaysAllocate,
                ..Default::default()
            },
        )
        .context("Failed while creating atlas luma image")?;
        let staging_buffer = VulkanBuffer::from_iter(
            &memory_allocator,
            &VulkanBufferCreateInfo {
                usage: VulkanBufferUsage::TRANSFER_SRC,
                ..Default::default()
            },
            &VulkanAllocationCreateInfo {
                memory_type_filter: VulkanMemoryTypeFilter::PREFER_HOST
                    | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            bytes,
        )
        .context("Error while creating atlas pixel staging buffer")?;
        let luma_staging_buffer = VulkanBuffer::from_iter(
            &memory_allocator,
            &VulkanBufferCreateInfo {
                usage: VulkanBufferUsage::TRANSFER_SRC,
                ..Default::default()
            },
            &VulkanAllocationCreateInfo {
                memory_type_filter: VulkanMemoryTypeFilter::PREFER_HOST
                    | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            luma_bytes,
        )
        .context("Error while creating luma pixel staging buffer")?;
        let mut command_buffer = VulkanAutoCommandBufferBuilder::primary(
            command_buffer_allocator,
            queue.queue_family_index(),
            VulkanCommandBufferUsage::OneTimeSubmit,
        )
        .context("Error while creating command buffer builder")?;
        command_buffer
            .copy_buffer_to_image(VulkanCopyBufferToImageInfo::new(
                staging_buffer,
                image.clone(),
            ))
            .unwrap();
        command_buffer
            .copy_buffer_to_image(VulkanCopyBufferToImageInfo::new(
                luma_staging_buffer,
                luma_image.clone(),
            ))
            .unwrap();
        let command_buffer = command_buffer.build().unwrap();
        vulkano::sync::now(device.clone())
            .then_execute(queue.clone(), command_buffer)
            .unwrap()
            .flush()
            .unwrap();
        let view = VulkanImageView::new_default(&image)
            .context("Failed while creating Vulkan atlas image view")?;
        let luma_view = VulkanImageView::new_default(&luma_image)
            .context("Failed while creating Vulkan atlas luma image view")?;
        let sampler = VulkanSampler::new(device, &VulkanSamplerCreateInfo::default())
            .context("Failed while creating Vulkan atlas sampler")?;
        Ok(Self {
            image,
            luma_image,
            view,
            luma_view,
            sampler,
        })
    }
}
