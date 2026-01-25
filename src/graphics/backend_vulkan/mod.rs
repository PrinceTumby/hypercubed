#![allow(clippy::std_instead_of_alloc)]
#![allow(clippy::std_instead_of_core)]
#[cfg(not(feature = "full_std"))]
compile_error!("The Vulkan backend requires full use of `std`");

pub mod chunk;
pub mod debug;
pub mod egui_renderer;
pub mod shader_exports;

use crate::graphics::chunk::{HasSubchunkData, SubchunkData};
use crate::graphics::debug::{Line as DebugLine, Point as DebugPoint, Triangle as DebugTriangle};
use crate::graphics::{Camera, DebugOutput, DebugState, GraphicsBackend, GraphicsOptions};
use crate::{MIN_HEIGHT_I32, SUBCHUNK_AXIS_LEN_I32};
use anyhow::{Context, anyhow};
use chunk::{
    block_face::{BlockFaceInstanceBufferManager, BlockFaceVertexBufferManager},
    custom_block::CustomBlockInstanceBufferManager,
    tinted_block_face::{TintedBlockFaceInstanceBufferManager, TintedBlockFaceVertexBufferManager},
};
use portable_std::{FastHashMap, FastHashMapEntry, FastHashSet};
use resources::block::ResourceData;
use resources::block::model::{ModelRegistry, Tint};
use shader_exports::RawViewInfo;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};
use threadpool::ThreadPool;
use vulkan_prelude::*;
use winit::window::Window;

#[derive(Clone)]
pub struct GraphicsResources {
    pub block_registry: Arc<resources::block::Registry>,
    pub model_registry: Arc<ModelRegistry>,
    pub device: Arc<vulkano::device::Device>,
    pub render_queue: Arc<vulkano::device::Queue>,
    pub compute_queue: Arc<vulkano::device::Queue>,
    pub memory_allocator: Arc<VulkanStandardMemoryAllocator>,
    pub command_buffer_allocator: Arc<VulkanStandardCommandBufferAllocator>,
    pub descriptor_set_allocator: Arc<VulkanStandardDescriptorSetAllocator>,
    pub render_pass: Arc<VulkanRenderPass>,
}

impl GraphicsOptions {
    pub fn get_vulkan_present_mode(&self) -> VulkanPresentMode {
        match self.vsync {
            true => VulkanPresentMode::Fifo,
            false => VulkanPresentMode::Immediate,
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

pub struct GraphicsState {
    pub size: winit::dpi::PhysicalSize<u32>,
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
    pub view_info_buffer: VulkanSubbuffer<RawViewInfo>,
    pub camera_descriptor_set: Arc<VulkanDescriptorSet>,
    pub matrices_descriptor_set: Arc<VulkanDescriptorSet>,
    pub block_item_atlas: TextureAtlas,
    pub block_item_atlas_descriptor_set: Arc<VulkanDescriptorSet>,
    pub subchunk_data_storage: SubchunkDataStorage,
    pub pending_subchunk_tx: Sender<Option<chunk::RawSubchunk>>,
    pub pending_subchunk_rx: Receiver<Option<chunk::RawSubchunk>>,
    pub current_dispatch_id_counter: u64,
    pub num_pending_subchunks: usize,
    // Debug state
    pub debug_point_pipeline: Arc<VulkanGraphicsPipeline>,
    pub debug_line_pipeline: Arc<VulkanGraphicsPipeline>,
    pub debug_triangle_pipeline: Arc<VulkanGraphicsPipeline>,
}

impl GraphicsBackend for GraphicsState {
    #[tracing::instrument(skip_all)]
    fn new(window: Arc<Window>, resource_data: ResourceData) -> anyhow::Result<Box<Self>> {
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
        let surface =
            VulkanSurface::from_window(&instance, &window).context("Failed to create surface")?;
        // Find a suitable physical device
        let required_extensions = VulkanDeviceExtensions {
            khr_swapchain: true,
            ..VulkanDeviceExtensions::empty()
        };
        let required_features = VulkanDeviceFeatures {
            vulkan_memory_model: true,
            multi_draw_indirect: true,
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
                present_mode: graphics_options.get_vulkan_present_mode(),
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
            // Load game resources.
            let ResourceData {
                block_registry,
                model_registry,
                atlas,
            } = resource_data;
            let atlas_texture = TextureAtlas::new(
                &atlas,
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
                model_registry.custom_block_faces.iter().map(|face| {
                    face.vertices.map(|v| {
                        chunk::custom_block::Vertex::new(
                            *v.local_pos.coords.as_ref(),
                            v.uvs,
                            *face.normal.as_ref(),
                            matches!(face.tint, Some(Tint::Biome)),
                        )
                    })
                }),
            )
            .context("Error while creating custom block faces buffer")?;
            (
                atlas_texture,
                [atlas.width, atlas.height],
                custom_block_faces_buffer,
                block_registry,
                model_registry,
            )
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
        let view_info_buffer: VulkanSubbuffer<RawViewInfo> = VulkanBuffer::new_sized(
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
        .context("Error while creating view info buffer")?;
        let view_info_descriptor_set = VulkanDescriptorSet::new(
            descriptor_set_allocator.clone(),
            camera_descriptor_set_layout.clone(),
            [VulkanWriteDescriptorSet::buffer(
                0,
                view_info_buffer.clone(),
            )],
            [],
        )
        .context("Error while creating view info descriptor set")?;
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
        let (pending_subchunk_tx, pending_subchunk_rx) = std::sync::mpsc::channel();
        Ok(Box::new(Self {
            size,
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
            view_info_buffer,
            camera_descriptor_set: view_info_descriptor_set,
            matrices_descriptor_set,
            block_item_atlas: block_item_texture_atlas,
            block_item_atlas_descriptor_set,
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
            debug_point_pipeline,
            debug_line_pipeline,
            debug_triangle_pipeline,
        }))
    }

    fn get_block_registry(&self) -> &resources::block::Registry {
        self.resources.block_registry.as_ref()
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
            (self.swapchain, self.swapchain_images) = self
                .swapchain
                .recreate(&VulkanSwapchainCreateInfo {
                    image_extent: new_size.into(),
                    present_mode: self.graphics_options.get_vulkan_present_mode(),
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
        }
    }

    fn get_graphics_options(&self) -> GraphicsOptions {
        self.graphics_options
    }

    #[tracing::instrument(skip_all)]
    fn apply_new_graphics_options(&mut self, new_options: GraphicsOptions) {
        let old_options = std::mem::replace(&mut self.graphics_options, new_options);
        // Swapchain update in `resize` also updates VSync
        if new_options.vsync != old_options.vsync {
            self.resize(self.size);
        }
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
            let block_registry = self.resources.block_registry.clone();
            let model_registry = self.resources.model_registry.clone();
            let raw_chunks = raw_chunks.clone();
            let pending_subchunk_tx = self.pending_subchunk_tx.clone();
            thread_pool.execute(move || {
                chunk::process_subchunk(
                    &block_registry,
                    &model_registry,
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
        camera: &Camera,
        egui_ctx: &egui::Context,
        egui_full_output: egui::output::FullOutput,
        debug_state: &DebugState,
        debug_points: &[DebugPoint],
        debug_lines: &[DebugLine],
        debug_triangles: &[DebugTriangle],
    ) -> anyhow::Result<Option<DebugOutput>> {
        // Upload pending subchunks.
        let mut subchunk_upload_semaphore: Option<VulkanSemaphore> = None;
        let mut _subchunk_upload_command_buffer: Option<Arc<_>> = None;
        if self.num_pending_subchunks > 0 {
            let span = tracing::trace_span!("upload_pending_subchunks");
            let _enter = span.enter();
            let mut command_buffer = VulkanAutoCommandBufferBuilder::primary(
                self.resources.command_buffer_allocator.clone(),
                self.resources.render_queue.queue_family_index(),
                VulkanCommandBufferUsage::OneTimeSubmit,
            )
            .context("Error while creating subchunk upload command buffer builder")
            .unwrap();
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
                    &mut command_buffer,
                    raw_subchunk,
                );
                subchunks_processed_this_frame += 1;
                if subchunks_processed_this_frame >= 16 {
                    break;
                }
            }
            let built_command_buffer = command_buffer
                .build()
                .context("Error while building subchunk upload command buffer")?;
            let semaphore = VulkanSemaphore::from_pool(&self.resources.device)
                .context("Error while creating subchunk upload semaphore")?;
            self.resources.render_queue.with(|mut queue_guard| unsafe {
                queue_guard
                    .submit(
                        &[VulkanSubmitInfo {
                            command_buffers: &[VulkanCommandBufferSubmitInfo::new(
                                built_command_buffer.as_raw(),
                            )],
                            wait_semaphores: &[],
                            signal_semaphores: &[VulkanSemaphoreSubmitInfo::new(&semaphore)],
                            ..Default::default()
                        }],
                        None,
                    )
                    .unwrap();
            });
            subchunk_upload_semaphore = Some(semaphore);
            _subchunk_upload_command_buffer = Some(built_command_buffer);
        }
        let subchunk_data_storage = &mut self.subchunk_data_storage;
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
                self.view_info_buffer.clone(),
                Box::new(RawViewInfo {
                    view_matrix: camera.generate_reversed_depth_view_matrix_slice(),
                    screen_size: self.size.into(),
                }),
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
                // Reconfigure the swapchain if it's out of date.
                Err(VulkanError::OutOfDate) => {
                    self.resize(self.size);
                    return Ok(None);
                }
                Err(VulkanError::Timeout) => return Ok(None),
                Err(err) => return Err(anyhow!("Error while acquiring swapchain image - {err}")),
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
        let mut block_face_draw_commands_buffer = None;
        let mut tinted_block_face_draw_commands_buffer = None;
        let mut custom_block_draw_commands_buffer = None;
        let debug_output;
        {
            let camera_chunk_coords = {
                // let camera_pos = debug_state.cull_camera.pos;
                let camera_x = (camera.pos.x.floor() as i32).div_euclid(SUBCHUNK_AXIS_LEN_I32);
                let camera_y = (camera.pos.y.floor() as i32 - MIN_HEIGHT_I32)
                    .div_euclid(SUBCHUNK_AXIS_LEN_I32);
                let camera_z = (camera.pos.z.floor() as i32).div_euclid(SUBCHUNK_AXIS_LEN_I32);
                [camera_x, camera_y, camera_z]
            };
            let mut block_face_draw_commands: Vec<VulkanDrawIndirectCommand> = Vec::new();
            let mut tinted_block_face_draw_commands: Vec<VulkanDrawIndirectCommand> = Vec::new();
            let mut custom_block_draw_commands: Vec<VulkanDrawIndirectCommand> = Vec::new();
            debug_output = super::for_each_visible_subchunk(
                camera,
                &subchunk_data_storage.subchunks,
                &subchunk_data_storage.loaded_chunks,
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
                },
            );
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
                            subchunk_data_storage.block_face_vertex.get_buffer(),
                            subchunk_data_storage.block_face_instance.get_buffer(),
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
                            subchunk_data_storage.tinted_block_face_vertex.get_buffer(),
                            subchunk_data_storage
                                .tinted_block_face_instance
                                .get_buffer(),
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
                        (subchunk_data_storage.custom_block_instance.get_buffer(),),
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
                    // Shader converts lines to quads, so we need 4 vertices per instance.
                    .draw(4, debug_lines.len().try_into().unwrap(), 0, 0)
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
        // Submit command buffer to GPU.
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
            let span = tracing::trace_span!("submit_render_command_buffer_and_present");
            let _enter = span.enter();
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
                let mut wait_semaphores = SmallVec::<[_; 2]>::new();
                wait_semaphores.push(VulkanSemaphoreSubmitInfo::new(&swapchain_semaphore));
                if let Some(upload_semaphore) = &subchunk_upload_semaphore {
                    wait_semaphores.push(VulkanSemaphoreSubmitInfo::new(upload_semaphore));
                };
                queue_guard
                    .submit(
                        &[VulkanSubmitInfo {
                            command_buffers: &[VulkanCommandBufferSubmitInfo::new(
                                built_command_buffer.as_raw(),
                            )],
                            wait_semaphores: &wait_semaphores,
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
                tracing::trace_span!("wait_for_render_fence").in_scope(|| {
                    finish_fence.wait(None).unwrap();
                });
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
        Ok(Some(debug_output))
    }
}

#[derive(Debug)]
pub struct TextureAtlas {
    pub image: Arc<VulkanImage>,
    pub view: Arc<VulkanImageView>,
    pub sampler: Arc<VulkanSampler>,
}

impl TextureAtlas {
    pub fn new(
        atlas: &resources::texture::Atlas,
        device: &Arc<VulkanDevice>,
        queue: &Arc<VulkanQueue>,
        memory_allocator: &Arc<dyn VulkanMemoryAllocator>,
        command_buffer_allocator: Arc<dyn VulkanCommandBufferAllocator>,
    ) -> anyhow::Result<Self> {
        let image = VulkanImage::new(
            memory_allocator,
            &VulkanImageCreateInfo {
                image_type: VulkanImageType::Dim2d,
                format: VulkanFormat::R8G8B8A8_UNORM,
                extent: [atlas.width, atlas.height, 1],
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
            atlas.texture_bytes.iter().copied(),
        )
        .context("Error while creating atlas pixel staging buffer")?;
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
        let command_buffer = command_buffer.build().unwrap();
        vulkano::sync::now(device.clone())
            .then_execute(queue.clone(), command_buffer)
            .unwrap()
            .flush()
            .unwrap();
        let view = VulkanImageView::new_default(&image)
            .context("Failed while creating Vulkan atlas image view")?;
        let sampler = VulkanSampler::new(device, &VulkanSamplerCreateInfo::default())
            .context("Failed while creating Vulkan atlas sampler")?;
        Ok(Self {
            image,
            view,
            sampler,
        })
    }
}
