#![allow(clippy::std_instead_of_alloc)]
#![allow(clippy::std_instead_of_core)]
#[cfg(not(feature = "full_std"))]
compile_error!("The Vulkan backend requires use of `std`");

pub mod chunk;
pub mod debug;
pub mod egui_renderer;
pub mod shader_exports;

use crate::basic_types::AxisDirection;
use crate::client::{MIN_HEIGHT_I32, SUBCHUNK_AXIS_LEN_I32};
use ahash::{AHashMap, AHashSet};
use anyhow::{Context, anyhow};
use chunk::{
    block_face::{BlockFaceInstanceBufferManager, BlockFaceVertexBufferManager},
    custom_block::CustomBlockInstanceBufferManager,
    tinted_block_face::{TintedBlockFaceInstanceBufferManager, TintedBlockFaceVertexBufferManager},
};
use debug::line::Instance as DebugLineInstance;
use debug::point::Vertex as DebugPointVertex;
use debug::triangle::Instance as DebugTriangleInstance;
use nalgebra::{Perspective3, Point3, Vector3};
use resources::block::model::{ModelRegistry, Tint};
use std::collections::VecDeque;
use std::sync::Arc;
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

#[derive(Clone)]
pub struct GraphicsResources {
    pub device: Arc<vulkano::device::Device>,
    pub render_queue: Arc<vulkano::device::Queue>,
    pub compute_queue: Arc<vulkano::device::Queue>,
    pub memory_allocator: Arc<VulkanStandardMemoryAllocator>,
    pub command_buffer_allocator: Arc<VulkanStandardCommandBufferAllocator>,
    pub descriptor_set_allocator: Arc<VulkanStandardDescriptorSetAllocator>,
    pub render_pass: Arc<VulkanRenderPass>,
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

pub struct GraphicsState {
    pub resources: GraphicsResources,
    pub swapchain: Arc<vulkano::swapchain::Swapchain>,
    pub swapchain_images: Vec<Arc<VulkanImage>>,
    pub depth_image: Arc<VulkanImage>,
    pub egui_renderer: egui_renderer::Renderer,
    pub graphics_options: GraphicsOptions,
    pub block_graphics_pipeline: Arc<VulkanGraphicsPipeline>,
    pub generic_block_graphics_pipeline_layout: Arc<VulkanPipelineLayout>,
    pub tinted_block_graphics_pipeline: Arc<VulkanGraphicsPipeline>,
    pub custom_block_graphics_pipeline: Arc<VulkanGraphicsPipeline>,
    pub custom_block_faces_buffer: VulkanSubbuffer<[[chunk::custom_block::Vertex; 4]]>,
    pub custom_block_faces_descriptor_set: Arc<VulkanDescriptorSet>,
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
    pub fn new<F>(window: Arc<Window>, register_blocks: F) -> anyhow::Result<Self>
    where
        F: FnOnce(
            &mut resources::block::Registry,
            &mut resources::block::model::ModelRegistryBuilder,
            &mut resources::texture::AtlasBuilder,
        ) -> anyhow::Result<()>,
    {
        let graphics_options = GraphicsOptions::default();
        // Initialise Vulkan state
        let library = VulkanLibrary::new().context("Failed to load Vulkan library")?;
        let surface_required_extensions = VulkanSurface::required_extensions(&window)
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
            VulkanSurface::from_window_ref(&instance, &window)
                .context("Failed to create surface")?
        };
        // Find a suitable physical device
        let required_extensions = VulkanDeviceExtensions {
            khr_swapchain: true,
            ..VulkanDeviceExtensions::empty()
        };
        let required_features = VulkanDeviceFeatures {
            vulkan_memory_model: true,
            multi_draw_indirect: true,
            shader_int8: true,
            shader_int16: true,
            storage_buffer8_bit_access: true,
            storage_buffer16_bit_access: true,
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
                let render_queue_family_usize = device
                    .queue_family_properties()
                    .iter()
                    .enumerate()
                    .position(|(i, queue_family)| {
                        queue_family
                            .queue_flags
                            .contains(VulkanQueueFlags::GRAPHICS | VulkanQueueFlags::COMPUTE)
                            && device.surface_support(i as u32, &surface).unwrap_or(false)
                    })?;
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
        let image_format = physical_device
            .surface_formats(&surface, &Default::default())
            .unwrap()[0]
            .0;
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
        let egui_renderer = egui_renderer::Renderer::new(
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
            custom_block_faces_buffer,
            block_registry,
            model_registry,
        ) = {
            let size = [1024; 2];
            let square_length = 16;
            let mut atlas_builder =
                resources::texture::AtlasBuilder::new(size[0], size[1], square_length);
            let mut model_cache = resources::block::model::ModelRegistryBuilder::new();
            let mut block_registry = resources::block::Registry::new();
            register_blocks(&mut block_registry, &mut model_cache, &mut atlas_builder)?;
            let atlas = TextureAtlas::from_builder(
                atlas_builder.finish(),
                &device,
                &render_queue,
                &(memory_allocator.clone() as Arc<_>),
                command_buffer_allocator.clone(),
            )
            .context("Failed while building block and item atlas")?;
            let custom_block_faces_buffer = VulkanBuffer::from_iter(
                &memory_allocator,
                &VulkanBufferCreateInfo {
                    usage: VulkanBufferUsage::STORAGE_BUFFER,
                    ..Default::default()
                },
                &VulkanAllocationCreateInfo {
                    memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                        | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    allocate_preference: VulkanMemoryAllocatePreference::AlwaysAllocate,
                    ..Default::default()
                },
                model_cache.custom_block_faces.iter().map(|face| {
                    face.map(|v| {
                        chunk::custom_block::Vertex::new(
                            *v.local_pos.coords.as_ref(),
                            v.uvs,
                            *v.normal.as_ref(),
                            matches!(v.tint, Some(Tint::Biome)),
                        )
                    })
                }),
            )
            .context("Error while creating custom block faces buffer")?;
            (
                atlas,
                size,
                custom_block_faces_buffer,
                block_registry,
                model_cache.finish(),
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
        let custom_block_faces_descriptor_set_layout = VulkanDescriptorSetLayout::new(
            &device,
            &VulkanDescriptorSetLayoutCreateInfo {
                bindings: &[VulkanDescriptorSetLayoutBinding {
                    binding: 0,
                    binding_flags: VulkanDescriptorBindingFlags::empty(),
                    descriptor_type: VulkanDescriptorType::StorageBuffer,
                    descriptor_count: 1,
                    stages: VulkanShaderStages::VERTEX | VulkanShaderStages::COMPUTE,
                    immutable_samplers: &[],
                    _ne: vulkano_non_exhaustive(),
                }],
                ..Default::default()
            },
        )
        .context("Error while creating custom block faces descriptor set layout")?;
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
            chunk::block_face::face_matrices::generate_array(),
        )
        .context("Error while creating face matrices buffer")?;
        let matrices_descriptor_set = VulkanDescriptorSet::new(
            descriptor_set_allocator.clone(),
            matrices_descriptor_set_layout.clone(),
            [VulkanWriteDescriptorSet::buffer(0, matrices_buffer.clone())],
            [],
        )
        .context("Error while creating matrices descriptor set")?;
        let custom_block_faces_descriptor_set = VulkanDescriptorSet::new(
            descriptor_set_allocator.clone(),
            custom_block_faces_descriptor_set_layout.clone(),
            [VulkanWriteDescriptorSet::buffer(
                0,
                custom_block_faces_buffer.clone(),
            )],
            [],
        )
        .context("Error while creating custom block faces descriptor set")?;
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
        // Block graphics pipelines
        let generic_block_graphics_pipeline_layout = VulkanPipelineLayout::new(
            &device,
            &VulkanPipelineLayoutCreateInfo {
                set_layouts: &[
                    &camera_descriptor_set_layout,
                    &block_item_atlas_descriptor_set_layout,
                    &matrices_descriptor_set_layout,
                    &custom_block_faces_descriptor_set_layout,
                ],
                ..Default::default()
            },
        )
        .context("Error while creating block graphics pipeline layout")?;
        let block_graphics_pipeline = chunk::block_face::create_graphics_pipeline(
            &device,
            &generic_block_graphics_pipeline_layout,
            &render_pass.first_subpass(),
        )
        .context("Error while creating block graphics pipeline")?;
        let tinted_block_graphics_pipeline = chunk::tinted_block_face::create_graphics_pipeline(
            &device,
            &generic_block_graphics_pipeline_layout,
            &render_pass.first_subpass(),
        )
        .context("Error while creating tinted block graphics pipeline")?;
        let custom_block_graphics_pipeline = chunk::custom_block::create_graphics_pipeline(
            &device,
            &generic_block_graphics_pipeline_layout,
            &render_pass.first_subpass(),
        )
        .context("Error while creating custom block graphics pipeline")?;
        // Buffer managers
        let block_face_vertex_buffer_manager = BlockFaceVertexBufferManager::new(&device)
            .context("Error while creating block face vertex buffer manager")?;
        let block_face_instance_buffer_manager = BlockFaceInstanceBufferManager::new(&device)
            .context("Error while creating block face instance buffer manager")?;
        let tinted_block_face_vertex_buffer_manager =
            TintedBlockFaceVertexBufferManager::new(&device)
                .context("Error while creating tinted block face vertex buffer manager")?;
        let tinted_block_face_instance_buffer_manager =
            TintedBlockFaceInstanceBufferManager::new(&device)
                .context("Error while creating tinted block face instance buffer manager")?;
        let custom_block_instance_buffer_manager =
            CustomBlockInstanceBufferManager::new(&device)
                .context("Error while creating custom block instance buffer manager")?;
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
                model_registry: Arc::new(model_registry),
            },
            swapchain,
            swapchain_images,
            depth_image,
            egui_renderer,
            graphics_options,
            block_graphics_pipeline,
            generic_block_graphics_pipeline_layout,
            tinted_block_graphics_pipeline,
            custom_block_graphics_pipeline,
            custom_block_faces_buffer,
            custom_block_faces_descriptor_set,
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

    #[expect(clippy::too_many_arguments)]
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
            let mut custom_block_draw_commands: Vec<VulkanDrawIndirectCommand> = Vec::new();
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
                            if debug_state.cave_cull_check_connectivity
                                && let Some(subchunk) = subchunk_maybe
                                && !subchunk.connected_faces.connects(&from_dir, &to_dir)
                            {
                                continue;
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
                    custom_block_draw_commands.push(VulkanDrawIndirectCommand {
                        vertex_count: group.start_face_and_len[1] * 6,
                        instance_count: group.start_instance_and_len[1],
                        first_vertex: group.start_face_and_len[0] * 6,
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
                    self.custom_block_faces_descriptor_set.clone(),
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
                        (self.buffer_managers.custom_block_instance.get_buffer(),),
                    )
                    .unwrap()
                    .draw_indirect(draw_commands_buffer)
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
                            command_buffers: &[VulkanCommandBufferSubmitInfo::new(
                                built_command_buffer.as_raw(),
                            )],
                            wait_semaphores: &[VulkanSemaphoreSubmitInfo::new(
                                &swapchain_semaphore,
                            )],
                            signal_semaphores: &[
                                VulkanSemaphoreSubmitInfo::new(&render_semaphore),
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
        atlas: resources::texture::Atlas,
        device: &Arc<VulkanDevice>,
        queue: &Arc<VulkanQueue>,
        memory_allocator: &Arc<dyn VulkanMemoryAllocator>,
        command_buffer_allocator: Arc<dyn VulkanCommandBufferAllocator>,
    ) -> anyhow::Result<Self> {
        let (width, height) = (atlas.texture.width(), atlas.texture.height());
        let bytes = atlas.texture.into_vec();
        let luma_bytes = atlas.luma_texture.into_vec();
        let image = VulkanImage::new(
            memory_allocator,
            &VulkanImageCreateInfo {
                image_type: VulkanImageType::Dim2d,
                format: VulkanFormat::R8G8B8A8_UNORM,
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
            memory_allocator,
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
            memory_allocator,
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
            memory_allocator,
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
