#![allow(clippy::std_instead_of_alloc)]
#![allow(clippy::std_instead_of_core)]
#[cfg(not(feature = "full_std"))]
compile_error!("The Vulkan backend requires full use of `std`");

// TODO: Switch to using `gl_DrawID` and per-draw-call chunk information buffers, so we can stop
//       using vertex buffer managers and save some VRAM.
// - Maybe, depending on how big chunk info is, we still have persistent buffers to store chunk
//   info?
//   - Can just be a simple slotted map, as each subchunk face group only has a single bit of draw
//     info, meaning we don't need complicated area allocation.
//   - Rendering just pushes a buffer of indices that's used as indirection for `gl_DrawID`.
// - Other alternative (which we can also use in wgpu) is that we just vertex pull using
//   `vertex_index / 4`, so we don't have to duplicate info 4 times.

pub mod chunk;
pub mod debug;
pub mod egui_renderer;
pub mod environment;
pub mod shader_exports;

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

use anyhow::{Context, anyhow};
use portable_std::{FastHashMap, FastHashMapEntry, FastHashSet};
use resources::GameResourceData;
use resources::block::model::Tint;
use shader_exports::chunk::SubchunkFaceGroupInfo;
use shader_exports::{CommonDescriptorSetIdxs, RawRenderInfo};
use threadpool::ThreadPool;
use vulkan_prelude::*;
use winit::event_loop::OwnedDisplayHandle;
use winit::window::Window;

use crate::graphics::chunk::{HasSubchunkData, SubchunkData};
use crate::graphics::debug::{Line as DebugLine, Point as DebugPoint, Triangle as DebugTriangle};
use crate::graphics::environment::sky::{STAR_QUADS, SkyExtrapolationState};
use crate::graphics::lightmap::{RawLightmapTexture, generate_lightmap_texture};
use crate::graphics::{DebugOutput, DebugState, GraphicsBackend, GraphicsOptions};
use crate::{ClientPlayState, MIN_HEIGHT_I32, SUBCHUNK_AXIS_LEN_I32};

#[derive(Clone)]
pub struct GraphicsResources {
    pub block_registry: Arc<resources::block::Registry>,
    pub model_registry: Arc<resources::block::model::ModelRegistry>,
    pub device: Arc<VulkanDevice>,
    pub render_queue: Arc<VulkanQueue>,
    pub compute_queue: Arc<VulkanQueue>,
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
}

impl SubchunkDataStorage {
    pub fn remove_subchunk(&mut self, subchunk_coords: [i32; 3]) {
        let FastHashMapEntry::Occupied(subchunk_entry) = self.subchunks.entry(subchunk_coords)
        else {
            return;
        };
        subchunk_entry.remove();
    }
}

pub struct EnvironmentGraphicsState {
    pub sun_graphics_pipeline: Arc<VulkanGraphicsPipeline>,
    pub moon_graphics_pipeline: Arc<VulkanGraphicsPipeline>,
    pub star_graphics_pipeline: Arc<VulkanGraphicsPipeline>,
    pub star_quads_buffer: VulkanSubbuffer<[[environment::sky::StarVertex; 4]]>,
    pub moon_phases_image: HypercubedVkImage,
    pub sun_image: HypercubedVkImage,
}

pub struct GraphicsState {
    pub size: winit::dpi::PhysicalSize<u32>,
    pub resources: GraphicsResources,
    pub swapchain: Arc<VulkanSwapchain>,
    pub swapchain_images: Vec<Arc<VulkanImage>>,
    pub depth_image: Arc<VulkanImage>,
    pub egui_renderer: egui_renderer::Renderer,
    pub graphics_options: GraphicsOptions,
    pub block_graphics_pipeline: Arc<VulkanGraphicsPipeline>,
    pub generic_pipeline_layout: Arc<VulkanPipelineLayout>,
    pub tinted_block_graphics_pipeline: Arc<VulkanGraphicsPipeline>,
    pub custom_block_graphics_pipeline: Arc<VulkanGraphicsPipeline>,
    pub custom_block_faces_buffer: VulkanSubbuffer<[[chunk::custom_block::Vertex; 4]]>,
    pub common_descriptor_set: Arc<VulkanDescriptorSet>,
    /// Contains rendering information that is constant between frames.
    pub base_render_info: RawRenderInfo,
    pub render_info_buffer: VulkanSubbuffer<RawRenderInfo>,
    pub block_item_atlas: HypercubedVkImage,
    pub lightmap_buffer: VulkanSubbuffer<RawLightmapTexture>,
    pub environment_state: EnvironmentGraphicsState,
    pub subchunk_data_storage: SubchunkDataStorage,
    pub pending_subchunk_tx: Sender<Option<([i32; 3], chunk::Subchunk)>>,
    pub pending_subchunk_rx: Receiver<Option<([i32; 3], chunk::Subchunk)>>,
    pub current_dispatch_id_counter: u64,
    pub num_pending_subchunks: usize,
    pub sky_extrapolation_state: SkyExtrapolationState,
    // Debug state
    pub debug_point_pipeline: Arc<VulkanGraphicsPipeline>,
    pub debug_line_pipeline: Arc<VulkanGraphicsPipeline>,
    pub debug_triangle_pipeline: Arc<VulkanGraphicsPipeline>,
}

impl GraphicsBackend for GraphicsState {
    #[tracing::instrument(skip_all)]
    fn new(
        window: Arc<Window>,
        _display: OwnedDisplayHandle,
        game_data: GameResourceData,
    ) -> anyhow::Result<Box<Self>> {
        let graphics_options = GraphicsOptions::default();
        // Initialise Vulkan state.
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
        // Find a suitable physical device.
        let required_extensions = VulkanDeviceExtensions {
            khr_swapchain: true,
            ..VulkanDeviceExtensions::empty()
        };
        let required_features = VulkanDeviceFeatures {
            buffer_device_address: true,
            multi_draw_indirect: true,
            shader_draw_parameters: true,
            vulkan_memory_model: true,
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
        log::debug!("Vulkan render queue family index: {render_queue_family_index}");
        log::debug!("Vulkan compute queue family index: {compute_queue_family_index}");
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
        // Generate dummy lightmap buffer and view, updated during frame render.
        let (lightmap_buffer, lightmap_buffer_view) = {
            let lightmap_buffer: VulkanSubbuffer<RawLightmapTexture> = VulkanBuffer::new_sized(
                &memory_allocator,
                &VulkanBufferCreateInfo {
                    usage: VulkanBufferUsage::UNIFORM_TEXEL_BUFFER
                        | VulkanBufferUsage::TRANSFER_DST,
                    ..Default::default()
                },
                &VulkanAllocationCreateInfo {
                    memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE,
                    ..Default::default()
                },
            )
            .context("Error while creating lightmap buffer")?;
            let lightmap_buffer_view = VulkanBufferView::new(
                &lightmap_buffer,
                &VulkanBufferViewCreateInfo {
                    format: VulkanFormat::R8G8B8A8_UNORM,
                    ..Default::default()
                },
            )
            .context("Error while creating lightmap buffer view")?;
            (lightmap_buffer, lightmap_buffer_view)
        };
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
        let (block_item_texture_atlas, block_item_atlas_size, custom_block_faces_buffer) = {
            let atlas_texture = HypercubedVkImage::create_from_resource_atlas(
                &atlas,
                &device,
                &render_queue,
                &(memory_allocator.clone() as Arc<_>),
                command_buffer_allocator.clone(),
            )
            .context("Error while creating block and item atlas image")?;
            let custom_block_faces_buffer = vulkan_buffer_from_iter_staged(
                render_queue.clone(),
                command_buffer_allocator.clone(),
                &(memory_allocator.clone() as Arc<_>),
                &VulkanBufferCreateInfo {
                    usage: VulkanBufferUsage::STORAGE_BUFFER,
                    ..Default::default()
                },
                &VulkanAllocationCreateInfo {
                    memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE,
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
            )
        };
        // Load sun and moon textures.
        let sun_image = HypercubedVkImage::create_from_resource_texture(
            &sun_texture,
            &device,
            &render_queue,
            &(memory_allocator.clone() as Arc<_>),
            command_buffer_allocator.clone(),
        )
        .context("Error while creating sun image")?;
        let moon_phases_image = HypercubedVkImage::create_from_resource_texture(
            &moon_phases_texture,
            &device,
            &render_queue,
            &(memory_allocator.clone() as Arc<_>),
            command_buffer_allocator.clone(),
        )
        .context("Error while creating moon phases image")?;
        // Common descriptor set, used for all rendering.
        let common_descriptor_set_layout = VulkanDescriptorSetLayout::new(
            &device.clone(),
            &VulkanDescriptorSetLayoutCreateInfo {
                bindings: &[
                    // Render info.
                    VulkanDescriptorSetLayoutBinding {
                        binding: CommonDescriptorSetIdxs::RenderInfo as u32,
                        binding_flags: VulkanDescriptorBindingFlags::empty(),
                        descriptor_type: VulkanDescriptorType::UniformBuffer,
                        descriptor_count: 1,
                        stages: VulkanShaderStages::VERTEX,
                        immutable_samplers: &[],
                        _ne: vulkano_non_exhaustive(),
                    },
                    // Lightmap uniform texel buffer.
                    VulkanDescriptorSetLayoutBinding {
                        binding: CommonDescriptorSetIdxs::Lightmap as u32,
                        binding_flags: VulkanDescriptorBindingFlags::empty(),
                        descriptor_type: VulkanDescriptorType::UniformTexelBuffer,
                        descriptor_count: 1,
                        stages: VulkanShaderStages::VERTEX,
                        immutable_samplers: &[],
                        _ne: vulkano_non_exhaustive(),
                    },
                    // Block and item atlas image.
                    VulkanDescriptorSetLayoutBinding {
                        binding: CommonDescriptorSetIdxs::BlockItemAtlasCombinedImageSampler as u32,
                        binding_flags: VulkanDescriptorBindingFlags::empty(),
                        descriptor_type: VulkanDescriptorType::CombinedImageSampler,
                        descriptor_count: 1,
                        stages: VulkanShaderStages::FRAGMENT,
                        immutable_samplers: &[&VulkanSampler::new(
                            &device,
                            &VulkanSamplerCreateInfo::default(),
                        )
                        .context("Error while creating block and item atlas sampler")?],
                        _ne: vulkano_non_exhaustive(),
                    },
                    // Custom block faces.
                    VulkanDescriptorSetLayoutBinding {
                        binding: CommonDescriptorSetIdxs::CustomBlockFaces as u32,
                        binding_flags: VulkanDescriptorBindingFlags::empty(),
                        descriptor_type: VulkanDescriptorType::StorageBuffer,
                        descriptor_count: 1,
                        stages: VulkanShaderStages::VERTEX,
                        immutable_samplers: &[],
                        _ne: vulkano_non_exhaustive(),
                    },
                    // Sun image.
                    VulkanDescriptorSetLayoutBinding {
                        binding: CommonDescriptorSetIdxs::SunCombinedImageSampler as u32,
                        binding_flags: VulkanDescriptorBindingFlags::empty(),
                        descriptor_type: VulkanDescriptorType::CombinedImageSampler,
                        descriptor_count: 1,
                        stages: VulkanShaderStages::FRAGMENT,
                        immutable_samplers: &[&VulkanSampler::new(
                            &device,
                            &VulkanSamplerCreateInfo::default(),
                        )
                        .context("Error while creating sun image sampler")?],
                        _ne: vulkano_non_exhaustive(),
                    },
                    // Moon phases image.
                    VulkanDescriptorSetLayoutBinding {
                        binding: CommonDescriptorSetIdxs::MoonPhasesCombinedImageSampler as u32,
                        binding_flags: VulkanDescriptorBindingFlags::empty(),
                        descriptor_type: VulkanDescriptorType::CombinedImageSampler,
                        descriptor_count: 1,
                        stages: VulkanShaderStages::FRAGMENT,
                        immutable_samplers: &[&VulkanSampler::new(
                            &device,
                            &VulkanSamplerCreateInfo::default(),
                        )
                        .context("Error while creating moon phases image sampler")?],
                        _ne: vulkano_non_exhaustive(),
                    },
                ],
                ..Default::default()
            },
        )
        .context("Error while creating common descriptor set layout")?;
        let render_info_buffer: VulkanSubbuffer<RawRenderInfo> = VulkanBuffer::new_sized(
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
        .context("Error while creating render info buffer")?;
        let common_descriptor_set = VulkanDescriptorSet::new(
            descriptor_set_allocator.clone(),
            common_descriptor_set_layout.clone(),
            [
                // Render info uniform buffer.
                VulkanWriteDescriptorSet::buffer(
                    CommonDescriptorSetIdxs::RenderInfo as u32,
                    render_info_buffer.clone(),
                ),
                // Lightmap uniform texel buffer.
                VulkanWriteDescriptorSet::buffer_view(
                    CommonDescriptorSetIdxs::Lightmap as u32,
                    lightmap_buffer_view.clone(),
                ),
                // Block and item atlas image (immutable sampler already embedded in layout).
                VulkanWriteDescriptorSet::image_view(
                    CommonDescriptorSetIdxs::BlockItemAtlasCombinedImageSampler as u32,
                    block_item_texture_atlas.view.clone(),
                ),
                // Custom block faces buffer.
                VulkanWriteDescriptorSet::buffer(
                    CommonDescriptorSetIdxs::CustomBlockFaces as u32,
                    custom_block_faces_buffer.clone(),
                ),
                // Sun image (immutable sampler already embedded in layout).
                VulkanWriteDescriptorSet::image_view(
                    CommonDescriptorSetIdxs::SunCombinedImageSampler as u32,
                    sun_image.view.clone(),
                ),
                // Moon phases image (immutable sampler already embedded in layout).
                VulkanWriteDescriptorSet::image_view(
                    CommonDescriptorSetIdxs::MoonPhasesCombinedImageSampler as u32,
                    moon_phases_image.view.clone(),
                ),
            ],
            [],
        )
        .context("Error while creating view info descriptor set")?;
        // Create base rendering info, constant between frames.
        let base_render_info = RawRenderInfo {
            view_matrix: Default::default(),
            recip_screen_size: Default::default(),
            recip_block_item_atlas_size: block_item_atlas_size.map(|n| (n as f32).recip()),
            face_matrices: chunk::block_face::face_matrices::generate_array(),
            sky_matrix: Default::default(),
            time_of_day: Default::default(),
            star_brightness: Default::default(),
        };
        // Block graphics pipelines.
        let generic_pipeline_layout = VulkanPipelineLayout::new(
            &device,
            &VulkanPipelineLayoutCreateInfo {
                set_layouts: &[&common_descriptor_set_layout],
                // Chunk rendering pipelines want a draw call info buffer address pushed in.
                push_constant_ranges: &[VulkanPushConstantRange {
                    stages: VulkanShaderStages::VERTEX,
                    offset: 0,
                    size: 8,
                }],
                ..Default::default()
            },
        )
        .context("Error while creating generic pipeline layout")?;
        let block_graphics_pipeline = chunk::block_face::create_graphics_pipeline(
            &device,
            &generic_pipeline_layout,
            &render_pass.first_subpass(),
        )
        .context("Error while creating block graphics pipeline")?;
        let tinted_block_graphics_pipeline = chunk::tinted_block_face::create_graphics_pipeline(
            &device,
            &generic_pipeline_layout,
            &render_pass.first_subpass(),
        )
        .context("Error while creating tinted block graphics pipeline")?;
        let custom_block_graphics_pipeline = chunk::custom_block::create_graphics_pipeline(
            &device,
            &generic_pipeline_layout,
            &render_pass.first_subpass(),
        )
        .context("Error while creating custom block graphics pipeline")?;
        // Environment graphics pipelines.
        let sun_graphics_pipeline = environment::sky::create_sun_graphics_pipeline(
            &device,
            &generic_pipeline_layout,
            &render_pass.first_subpass(),
        )
        .context("Error while creating sun graphics pipeline")?;
        let moon_graphics_pipeline = environment::sky::create_moon_graphics_pipeline(
            &device,
            &generic_pipeline_layout,
            &render_pass.first_subpass(),
        )
        .context("Error while creating moon graphics pipeline")?;
        let star_graphics_pipeline = environment::sky::create_star_graphics_pipeline(
            &device,
            &generic_pipeline_layout,
            &render_pass.first_subpass(),
        )
        .context("Error while creating star graphics pipeline")?;
        // Star quads buffer.
        let star_quads_buffer = vulkan_buffer_from_iter_staged(
            render_queue.clone(),
            command_buffer_allocator.clone(),
            &(memory_allocator.clone() as Arc<_>),
            &VulkanBufferCreateInfo {
                usage: VulkanBufferUsage::VERTEX_BUFFER,
                ..Default::default()
            },
            &VulkanAllocationCreateInfo {
                memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE,
                ..Default::default()
            },
            STAR_QUADS
                .iter()
                .map(|&[p1, p2, p3, p4]| [p1, p2, p4, p3].map(environment::sky::StarVertex)),
        )
        .context("Error while creating star quads buffer")?;
        // Debug pipelines.
        let debug_point_pipeline = debug::point::create_graphics_pipeline(
            &device,
            &generic_pipeline_layout,
            &render_pass.first_subpass(),
        )?;
        let debug_line_pipeline = debug::line::create_graphics_pipeline(
            &device,
            &generic_pipeline_layout,
            &render_pass.first_subpass(),
        )?;
        let debug_triangle_pipeline = debug::triangle::create_graphics_pipeline(
            &device,
            &generic_pipeline_layout,
            &render_pass.first_subpass(),
        )?;
        // Initialise egui renderer, for debug UI.
        let egui_renderer =
            egui_renderer::Renderer::new(&device, &common_descriptor_set_layout, &render_pass)
                .context("Error while creating egui renderer")?;
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
            generic_pipeline_layout,
            tinted_block_graphics_pipeline,
            custom_block_graphics_pipeline,
            common_descriptor_set,
            base_render_info,
            render_info_buffer,
            custom_block_faces_buffer,
            block_item_atlas: block_item_texture_atlas,
            lightmap_buffer,
            environment_state: EnvironmentGraphicsState {
                sun_graphics_pipeline,
                moon_graphics_pipeline,
                star_graphics_pipeline,
                star_quads_buffer,
                moon_phases_image,
                sun_image,
            },
            subchunk_data_storage: SubchunkDataStorage {
                subchunks: FastHashMap::new(),
                loaded_chunks: FastHashSet::new(),
            },
            pending_subchunk_tx,
            pending_subchunk_rx,
            current_dispatch_id_counter: 0,
            num_pending_subchunks: 0,
            sky_extrapolation_state: SkyExtrapolationState::new(),
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
            .context("Error while creating new depth image")
            .unwrap();
        }
    }

    fn get_graphics_options(&self) -> GraphicsOptions {
        self.graphics_options
    }

    #[tracing::instrument(skip_all)]
    fn apply_new_graphics_options(&mut self, new_options: GraphicsOptions) {
        let old_options = std::mem::replace(&mut self.graphics_options, new_options);
        // Swapchain update in `resize` also updates VSync.
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
            let memory_allocator = self.resources.memory_allocator.clone();
            thread_pool.execute(move || {
                chunk::process_subchunk(
                    &block_registry,
                    &model_registry,
                    &raw_chunks,
                    &pending_subchunk_tx,
                    &(memory_allocator as Arc<_>),
                    subchunk_coords,
                    dispatch_id,
                )
                .expect("Error while processing subchunk");
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
        // Add any subchunks that have now finished processing.
        if self.num_pending_subchunks > 0 {
            let span = tracing::trace_span!("add_pending_subchunks");
            let _enter = span.enter();
            for maybe_subchunk in self
                .pending_subchunk_rx
                .try_iter()
                .take(self.num_pending_subchunks)
            {
                self.num_pending_subchunks -= 1;
                let Some((subchunk_coords, subchunk)) = maybe_subchunk else {
                    continue;
                };
                // Check that the subchunk is newer than the subchunk it's replacing, or that it's
                // a brand new subchunk.
                // If it's not, then skip it.
                if self
                    .subchunk_data_storage
                    .subchunks
                    .get(&subchunk_coords)
                    .map(|old_subchunk| old_subchunk.dispatch_id > subchunk.dispatch_id)
                    .unwrap_or(false)
                {
                    continue;
                }
                // Remove old subchunk.
                self.subchunk_data_storage.remove_subchunk(subchunk_coords);
                // Add new subchunk.
                self.subchunk_data_storage
                    .subchunks
                    .insert(subchunk_coords, subchunk);
            }
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
        command_buffer
            .update_buffer(
                self.render_info_buffer.clone(),
                Box::new(RawRenderInfo {
                    view_matrix: camera.generate_reversed_depth_view_matrix_slice(),
                    sky_matrix,
                    recip_screen_size: <[u32; 2]>::from(self.size).map(|n| (n as f32).recip()),
                    face_matrices: self.base_render_info.face_matrices,
                    recip_block_item_atlas_size: self.base_render_info.recip_block_item_atlas_size,
                    time_of_day: time_of_day.rem_euclid(192_000.0) as f32,
                    star_brightness,
                }),
            )
            .unwrap();
        // Update lightmap.
        command_buffer
            .update_buffer(
                self.lightmap_buffer.clone(),
                Box::new(generate_lightmap_texture(
                    self.graphics_options.lightmap_gamma_setting,
                    time_of_day,
                )),
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
        // Main render subpass.
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
                    clear_values: vec![Some([sky_r, sky_g, sky_b, 1.0].into()), Some(0.0.into())],
                    ..VulkanRenderPassBeginInfo::framebuffer(framebuffer.clone())
                },
                VulkanSubpassBeginInfo {
                    contents: VulkanSubpassContents::Inline,
                    ..Default::default()
                },
            )
            .unwrap();
        // Block rendering.
        let mut block_face_draw_commands_buffer = None;
        let mut block_face_subchunk_face_groups_buffer = None;
        let mut tinted_block_face_draw_commands_buffer = None;
        let mut tinted_block_face_subchunk_face_groups_buffer = None;
        let mut custom_block_draw_commands_buffer = None;
        let mut custom_block_subchunk_instance_groups_buffer = None;
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
            let mut block_face_subchunk_face_groups: Vec<SubchunkFaceGroupInfo> = Vec::new();
            let mut tinted_block_face_draw_commands: Vec<VulkanDrawIndirectCommand> = Vec::new();
            let mut tinted_block_face_subchunk_face_groups: Vec<SubchunkFaceGroupInfo> = Vec::new();
            let mut custom_block_draw_commands: Vec<VulkanDrawIndirectCommand> = Vec::new();
            let mut custom_block_subchunk_instance_groups: Vec<u64> = Vec::new();
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
                        // Base block faces.
                        if subchunk.block_face_instance_groups[i].1 != 0 {
                            let buffer = subchunk.block_face_instances_buffer.as_ref().unwrap();
                            let buffer_offset = subchunk.block_face_instance_groups[i].0 as u64
                                * core::mem::size_of::<chunk::block_face::Instance>() as u64;
                            block_face_draw_commands.push(VulkanDrawIndirectCommand {
                                vertex_count: 4,
                                instance_count: subchunk.block_face_instance_groups[i].1,
                                first_vertex: 0,
                                first_instance: 0,
                            });
                            block_face_subchunk_face_groups.push(SubchunkFaceGroupInfo {
                                buffer_address: buffer.address.get() + buffer_offset,
                                subchunk_start_coords: subchunk.start_coords,
                                face_matrix_index: i as u32,
                            });
                        }
                        // Tinted block faces.
                        if subchunk.tinted_block_face_instance_groups[i].1 != 0 {
                            let buffer = subchunk
                                .tinted_block_face_instances_buffer
                                .as_ref()
                                .unwrap();
                            let buffer_offset = subchunk.tinted_block_face_instance_groups[i].0
                                as u64
                                * core::mem::size_of::<chunk::tinted_block_face::Instance>() as u64;
                            tinted_block_face_draw_commands.push(VulkanDrawIndirectCommand {
                                vertex_count: 4,
                                instance_count: subchunk.tinted_block_face_instance_groups[i].1,
                                first_vertex: 0,
                                first_instance: 0,
                            });
                            tinted_block_face_subchunk_face_groups.push(SubchunkFaceGroupInfo {
                                buffer_address: buffer.address.get() + buffer_offset,
                                subchunk_start_coords: subchunk.start_coords,
                                face_matrix_index: i as u32,
                            });
                        }
                    }
                    // Custom blocks.
                    if !subchunk.custom_block_groups.is_empty() {
                        let buffer = subchunk.custom_block_instances_buffer.as_ref().unwrap();
                        for group in &subchunk.custom_block_groups {
                            let buffer_offset = group.start_instance_and_len[0] as u64
                                * core::mem::size_of::<chunk::custom_block::Instance>() as u64;
                            custom_block_draw_commands.push(VulkanDrawIndirectCommand {
                                vertex_count: group.start_face_and_len[1] * 6,
                                instance_count: group.start_instance_and_len[1],
                                first_vertex: group.start_face_and_len[0] * 6,
                                first_instance: 0,
                            });
                            custom_block_subchunk_instance_groups
                                .push(buffer.address.get() + buffer_offset);
                        }
                    }
                },
            );
            if !block_face_draw_commands.is_empty() {
                assert_eq!(
                    block_face_draw_commands.len(),
                    block_face_subchunk_face_groups.len()
                );
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
                block_face_subchunk_face_groups_buffer = Some(
                    VulkanBuffer::from_iter(
                        &self.resources.memory_allocator,
                        &VulkanBufferCreateInfo {
                            usage: VulkanBufferUsage::SHADER_DEVICE_ADDRESS,
                            ..Default::default()
                        },
                        &VulkanAllocationCreateInfo {
                            memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                                | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                            ..Default::default()
                        },
                        block_face_subchunk_face_groups,
                    )
                    .context("Error while creating block face subchunk face groups buffer")?,
                );
            }
            if !tinted_block_face_draw_commands.is_empty() {
                assert_eq!(
                    tinted_block_face_draw_commands.len(),
                    tinted_block_face_subchunk_face_groups.len()
                );
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
                tinted_block_face_subchunk_face_groups_buffer = Some(
                    VulkanBuffer::from_iter(
                        &self.resources.memory_allocator,
                        &VulkanBufferCreateInfo {
                            usage: VulkanBufferUsage::SHADER_DEVICE_ADDRESS,
                            ..Default::default()
                        },
                        &VulkanAllocationCreateInfo {
                            memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                                | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                            ..Default::default()
                        },
                        tinted_block_face_subchunk_face_groups,
                    )
                    .context(
                        "Error while creating tinted block face subchunk face groups buffer",
                    )?,
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
                custom_block_subchunk_instance_groups_buffer = Some(
                    VulkanBuffer::from_iter(
                        &self.resources.memory_allocator,
                        &VulkanBufferCreateInfo {
                            usage: VulkanBufferUsage::SHADER_DEVICE_ADDRESS,
                            ..Default::default()
                        },
                        &VulkanAllocationCreateInfo {
                            memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                                | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                            ..Default::default()
                        },
                        custom_block_subchunk_instance_groups,
                    )
                    .context("Error while creating custom block subchunk instance groups buffer")?,
                );
            }
        }
        // Render subchunks.
        command_buffer
            // We need to bind a pipeline with a compatible layout for binding descriptor sets, so
            // bind the block graphics pipeline here.
            .bind_pipeline_graphics(self.block_graphics_pipeline.clone())
            .unwrap()
            .bind_descriptor_sets(
                VulkanPipelineBindPoint::Graphics,
                self.generic_pipeline_layout.clone(),
                0,
                self.common_descriptor_set.clone(),
            )
            .unwrap()
            // We need to set push constants even if nothing uses them, so just zero out.
            .push_constants(self.generic_pipeline_layout.clone(), 0, 0u64)
            .unwrap()
            .set_viewport(
                0,
                SmallVec::from(&[VulkanViewport {
                    extent: [self.size.width as f32, self.size.height as f32],
                    ..Default::default()
                }] as &[_]),
            )
            .unwrap();
        // Draw basic block faces.
        if let Some(draw_commands_buffer) = block_face_draw_commands_buffer {
            let subchunk_face_groups_buffer_device_address = block_face_subchunk_face_groups_buffer
                .as_ref()
                .unwrap()
                .device_address()
                .context("Error while getting basic face groups buffer device address")?;
            unsafe {
                // Block graphics pipeline already bound, see above.
                command_buffer
                    .push_constants(
                        self.generic_pipeline_layout.clone(),
                        0,
                        subchunk_face_groups_buffer_device_address.get(),
                    )
                    .unwrap()
                    .draw_indirect(draw_commands_buffer)
                    .unwrap();
            }
        }
        // Draw tinted block faces.
        if let Some(draw_commands_buffer) = tinted_block_face_draw_commands_buffer {
            let subchunk_face_groups_buffer_device_address =
                tinted_block_face_subchunk_face_groups_buffer
                    .as_ref()
                    .unwrap()
                    .device_address()
                    .context("Error while getting tinted face groups buffer device address")?;
            unsafe {
                command_buffer
                    .bind_pipeline_graphics(self.tinted_block_graphics_pipeline.clone())
                    .unwrap()
                    .push_constants(
                        self.generic_pipeline_layout.clone(),
                        0,
                        subchunk_face_groups_buffer_device_address.get(),
                    )
                    .unwrap()
                    .draw_indirect(draw_commands_buffer)
                    .unwrap();
            }
        }
        // Draw custom blocks.
        if let Some(draw_commands_buffer) = custom_block_draw_commands_buffer {
            let subchunk_instance_buffer_device_address =
                custom_block_subchunk_instance_groups_buffer
                    .as_ref()
                    .unwrap()
                    .device_address()
                    .context("Error while getting tinted face groups buffer device address")?;
            unsafe {
                command_buffer
                    .bind_pipeline_graphics(self.custom_block_graphics_pipeline.clone())
                    .unwrap()
                    .push_constants(
                        self.generic_pipeline_layout.clone(),
                        0,
                        subchunk_instance_buffer_device_address.get(),
                    )
                    .unwrap()
                    .draw_indirect(draw_commands_buffer)
                    .unwrap();
            }
        }
        // Render sky.
        {
            // Draw sun.
            unsafe {
                command_buffer
                    .bind_pipeline_graphics(self.environment_state.sun_graphics_pipeline.clone())
                    .unwrap()
                    .draw(4, 1, 0, 0)
                    .unwrap();
            }
            // Draw moon.
            unsafe {
                command_buffer
                    .bind_pipeline_graphics(self.environment_state.moon_graphics_pipeline.clone())
                    .unwrap()
                    .draw(4, 1, 0, 0)
                    .unwrap();
            }
            // Draw stars.
            if star_brightness > 0.0 {
                unsafe {
                    command_buffer
                        .bind_pipeline_graphics(
                            self.environment_state.star_graphics_pipeline.clone(),
                        )
                        .unwrap()
                        .bind_vertex_buffers(0, self.environment_state.star_quads_buffer.clone())
                        .unwrap()
                        .draw(4, STAR_QUADS.len() as u32, 0, 0)
                        .unwrap();
                }
            }
        }
        // Render debug graphics.
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
                    .bind_vertex_buffers(0, buffer)
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
                    .bind_vertex_buffers(0, buffer)
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
                    .bind_vertex_buffers(0, buffer)
                    .unwrap()
                    .draw(3, debug_triangles.len().try_into().unwrap(), 0, 0)
                    .unwrap();
            }
        }
        // Render egui UI.
        if let Some(egui_render_data) = egui_render_data {
            self.egui_renderer.render(
                &mut command_buffer,
                self.common_descriptor_set.clone(),
                egui_render_data,
            );
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
pub struct HypercubedVkImage {
    pub image: Arc<VulkanImage>,
    pub view: Arc<VulkanImageView>,
}

impl HypercubedVkImage {
    pub fn create_from_resource_atlas(
        atlas: &resources::texture::Atlas,
        device: &Arc<VulkanDevice>,
        queue: &Arc<VulkanQueue>,
        memory_allocator: &Arc<dyn VulkanMemoryAllocator>,
        command_buffer_allocator: Arc<dyn VulkanCommandBufferAllocator>,
    ) -> anyhow::Result<Self> {
        Self::create_from_raw(
            &atlas.texture_bytes,
            atlas.width,
            atlas.height,
            VulkanFormat::R8G8B8A8_UNORM,
            device,
            queue,
            memory_allocator,
            command_buffer_allocator,
        )
    }

    pub fn create_from_resource_texture(
        texture: &resources::texture::RawTexture,
        device: &Arc<VulkanDevice>,
        queue: &Arc<VulkanQueue>,
        memory_allocator: &Arc<dyn VulkanMemoryAllocator>,
        command_buffer_allocator: Arc<dyn VulkanCommandBufferAllocator>,
    ) -> anyhow::Result<Self> {
        Self::create_from_raw(
            &texture.texture_bytes,
            texture.width,
            texture.height,
            VulkanFormat::R8G8B8A8_UNORM,
            device,
            queue,
            memory_allocator,
            command_buffer_allocator,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_from_raw(
        texture_bytes: &[u8],
        width: u32,
        height: u32,
        image_format: VulkanFormat,
        device: &Arc<VulkanDevice>,
        queue: &Arc<VulkanQueue>,
        memory_allocator: &Arc<dyn VulkanMemoryAllocator>,
        command_buffer_allocator: Arc<dyn VulkanCommandBufferAllocator>,
    ) -> anyhow::Result<Self> {
        let image = VulkanImage::new(
            memory_allocator,
            &VulkanImageCreateInfo {
                image_type: VulkanImageType::Dim2d,
                format: image_format,
                extent: [width, height, 1],
                usage: VulkanImageUsage::SAMPLED | VulkanImageUsage::TRANSFER_DST,
                ..Default::default()
            },
            &VulkanAllocationCreateInfo {
                memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE,
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
            texture_bytes.iter().copied(),
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
        Ok(Self { image, view })
    }
}
