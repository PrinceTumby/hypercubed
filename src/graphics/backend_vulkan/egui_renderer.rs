use super::GraphicsResources;
use super::shader_exports::shader_stage_from_entry_point;
use anyhow::Context;
use foldhash::{HashMap, HashMapExt};
use std::sync::Arc;
use vulkano_prelude::*;

pub struct Renderer {
    pipeline_layout: Arc<VulkanPipelineLayout>,
    pipeline: Arc<VulkanGraphicsPipeline>,
    images: HashMap<egui::TextureId, ImageData>,
    image_descriptor_set_layout: Arc<VulkanDescriptorSetLayout>,
    sampler_cache: HashMap<egui::TextureOptions, Arc<VulkanSampler>>,
    next_user_texture_id: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ScreenSize {
    width: f32,
    height: f32,
}

struct ImageData {
    pub image: Arc<VulkanImage>,
    pub descriptor_set: Arc<VulkanDescriptorSet>,
}

pub struct RenderData {
    meshes: Vec<RenderMeshInfo>,
    vertex_buffer: VulkanSubbuffer<[Vertex]>,
    index_buffer: VulkanSubbuffer<[u32]>,
}

struct RenderMeshInfo {
    scissor_rect: VulkanScissor,
    base_vertex: i32,
    index_slice: std::ops::Range<u32>,
    texture_id: egui::TextureId,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pub pos: [f32; 2],
    pub uvs: [f32; 2],
    pub color: [u8; 4],
}

impl Renderer {
    pub fn new(
        device: &Arc<VulkanDevice>,
        common_descriptor_set_layout: &Arc<VulkanDescriptorSetLayout>,
        render_pass: &Arc<VulkanRenderPass>,
    ) -> anyhow::Result<Self> {
        let image_descriptor_set_layout = VulkanDescriptorSetLayout::new(
            device,
            &VulkanDescriptorSetLayoutCreateInfo {
                bindings: &[VulkanDescriptorSetLayoutBinding {
                    binding: 0,
                    binding_flags: VulkanDescriptorBindingFlags::empty(),
                    descriptor_type: VulkanDescriptorType::CombinedImageSampler,
                    descriptor_count: 1,
                    stages: VulkanShaderStages::FRAGMENT,
                    immutable_samplers: &[],
                    // HACK: See prelude.
                    _ne: vulkano_non_exhaustive(),
                }],
                ..Default::default()
            },
        )
        .context("Error while creating image descriptor set layout")?;
        let pipeline_layout = VulkanPipelineLayout::new(
            device,
            &VulkanPipelineLayoutCreateInfo {
                set_layouts: &[common_descriptor_set_layout, &image_descriptor_set_layout],
                ..Default::default()
            },
        )
        .context("Error while creating egui graphics pipeline layout")?;
        let pipeline = VulkanGraphicsPipeline::new(
            device,
            None, // No pipeline cache
            &VulkanGraphicsPipelineCreateInfo {
                flags: VulkanPipelineCreateFlags::default(),
                stages: &[
                    shader_stage_from_entry_point(&mut None, device, "egui", "vertex"),
                    shader_stage_from_entry_point(&mut None, device, "egui", "fragment"),
                ],
                vertex_input_state: Some(&VulkanVertexInputState {
                    bindings: vulkan_vertex_bindings![
                        0 => (Vertex, Vertex),
                    ],
                    attributes: vulkan_vertex_attributes!(1, [
                        // pos
                        [0 <- 0] => R32G32_SFLOAT,
                        // uvs
                        [1 <- 0] => R32G32_SFLOAT,
                        // color
                        [2 <- 0] => R8G8B8A8_UNORM,
                    ]),
                    ..Default::default()
                }),
                input_assembly_state: Some(&VulkanInputAssemblyState {
                    topology: VulkanPrimitiveTopology::TriangleList,
                    primitive_restart_enable: false,
                    ..Default::default()
                }),
                tessellation_state: None,
                // We leave the viewport state as a single default viewport, as we use dynamic
                // state to set the viewport at render time.
                viewport_state: Some(&Default::default()),
                rasterization_state: Some(&Default::default()),
                multisample_state: Some(&Default::default()),
                depth_stencil_state: Some(&VulkanDepthStencilState {
                    depth: Some(VulkanDepthState {
                        write_enable: false,
                        compare_op: VulkanCompareOp::Always,
                    }),
                    ..Default::default()
                }),
                color_blend_state: Some(&VulkanColorBlendState {
                    attachments: &[VulkanColorBlendAttachmentState {
                        blend: Some(VulkanAttachmentBlend {
                            src_color_blend_factor: VulkanBlendFactor::One,
                            dst_color_blend_factor: VulkanBlendFactor::OneMinusSrcAlpha,
                            color_blend_op: VulkanBlendOp::Add,
                            src_alpha_blend_factor: VulkanBlendFactor::OneMinusSrcAlpha,
                            dst_alpha_blend_factor: VulkanBlendFactor::One,
                            alpha_blend_op: VulkanBlendOp::Add,
                        }),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                dynamic_state: &[VulkanDynamicState::Viewport, VulkanDynamicState::Scissor],
                layout: &pipeline_layout,
                subpass: Some(VulkanPipelineSubpassType::BeginRenderPass(
                    &render_pass.first_subpass(),
                )),
                base_pipeline: None,
                discard_rectangle_state: None,
                fragment_shading_rate_state: None,
                _ne: vulkano_non_exhaustive(),
            },
        )
        .context("Error while creating egui graphics pipeline")?;
        Ok(Self {
            pipeline_layout,
            pipeline,
            images: HashMap::new(),
            image_descriptor_set_layout,
            sampler_cache: HashMap::new(),
            next_user_texture_id: 0,
        })
    }

    pub fn free_textures(&mut self, texture_ids: &[egui::TextureId]) {
        for id in texture_ids {
            self.images.remove(id);
        }
    }

    fn update_textures(
        &mut self,
        device: &Arc<VulkanDevice>,
        memory_allocator: &Arc<dyn VulkanMemoryAllocator>,
        descriptor_set_allocator: &Arc<dyn VulkanDescriptorSetAllocator>,
        command_buffer: &mut VulkanAutoCommandBufferBuilder<VulkanPrimaryAutoCommandBuffer>,
        textures: Vec<(egui::TextureId, egui::epaint::image::ImageDelta)>,
    ) -> anyhow::Result<()> {
        for (texture_id, texture_data) in textures {
            let [width, height] = texture_data.image.size();
            let size = texture_data.image.size().map(|n| n as u32);
            let rgba_pixels = match &texture_data.image {
                egui::ImageData::Color(image) => {
                    assert_eq!(width * height, image.pixels.len());
                    std::borrow::Cow::Borrowed(&image.pixels)
                }
            };
            let rgba_bytes: &[u8] = bytemuck::cast_slice(rgba_pixels.as_slice());
            if let Some(pos) = texture_data.pos {
                // Update existing image with new data
                let current_image = &self.images[&texture_id];
                let origin: [u32; 2] = pos.map(|n| n as u32);
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
                    rgba_bytes.iter().copied(),
                )
                .context("Error while creating egui pixel staging buffer")?;
                command_buffer
                    .copy_buffer_to_image(VulkanCopyBufferToImageInfo {
                        src_buffer: staging_buffer,
                        dst_image: current_image.image.clone(),
                        dst_image_layout: VulkanImageLayout::TransferDstOptimal,
                        regions: [VulkanBufferImageCopy {
                            buffer_row_length: size[0],
                            image_subresource: VulkanImageSubresourceLayers {
                                aspects: VulkanImageAspects::COLOR,
                                mip_level: 0,
                                base_array_layer: 0,
                                layer_count: 1,
                            },
                            image_offset: [origin[0], origin[1], 0],
                            image_extent: [size[0], size[1], 1],
                            ..Default::default()
                        }]
                        .into(),
                        // HACK: See prelude.
                        _ne: vulkano_non_exhaustive(),
                    })
                    .unwrap();
            } else {
                // Register new image
                let image = VulkanImage::new(
                    memory_allocator,
                    &VulkanImageCreateInfo {
                        image_type: VulkanImageType::Dim2d,
                        format: VulkanFormat::R8G8B8A8_UNORM,
                        extent: [size[0], size[1], 1],
                        usage: VulkanImageUsage::SAMPLED | VulkanImageUsage::TRANSFER_DST,
                        ..Default::default()
                    },
                    &VulkanAllocationCreateInfo {
                        memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE,
                        ..Default::default()
                    },
                )
                .context("Error while creating egui image")?;
                let sampler = self
                    .sampler_cache
                    .entry(texture_data.options)
                    .or_insert_with(|| {
                        use egui::{TextureFilter, TextureWrapMode};
                        fn convert_filter(filter: TextureFilter) -> VulkanFilter {
                            match filter {
                                TextureFilter::Nearest => VulkanFilter::Nearest,
                                TextureFilter::Linear => VulkanFilter::Linear,
                            }
                        }
                        let address_mode = match texture_data.options.wrap_mode {
                            TextureWrapMode::ClampToEdge => VulkanSamplerAddressMode::ClampToEdge,
                            TextureWrapMode::Repeat => VulkanSamplerAddressMode::Repeat,
                            TextureWrapMode::MirroredRepeat => {
                                VulkanSamplerAddressMode::MirroredRepeat
                            }
                        };
                        VulkanSampler::new(
                            device,
                            &VulkanSamplerCreateInfo {
                                mag_filter: convert_filter(texture_data.options.magnification),
                                min_filter: convert_filter(texture_data.options.minification),
                                address_mode: [address_mode; 3],
                                ..Default::default()
                            },
                        )
                        .context("Error while creating egui image sampler")
                        .unwrap()
                    })
                    .clone();
                let view = VulkanImageView::new_default(&image)
                    .context("Error while creating egui image view")?;
                let descriptor_set = VulkanDescriptorSet::new(
                    descriptor_set_allocator.clone(),
                    self.image_descriptor_set_layout.clone(),
                    [VulkanWriteDescriptorSet::image(
                        0,
                        VulkanDescriptorImageInfo {
                            sampler: Some(sampler.clone()),
                            image_view: Some(view.clone()),
                            ..Default::default()
                        },
                    )],
                    [],
                )
                .context("Error while creating egui image descriptor set")?;
                // Write image data
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
                    rgba_bytes.iter().copied(),
                )
                .context("Error while creating egui pixel staging buffer")?;
                command_buffer
                    .copy_buffer_to_image(VulkanCopyBufferToImageInfo::new(
                        staging_buffer,
                        image.clone(),
                    ))
                    .unwrap();
                self.images.insert(
                    texture_id,
                    ImageData {
                        image,
                        descriptor_set,
                    },
                );
            }
        }
        Ok(())
    }

    pub fn prepare(
        &mut self,
        graphics_resources: &GraphicsResources,
        command_buffer: &mut VulkanAutoCommandBufferBuilder<VulkanPrimaryAutoCommandBuffer>,
        physical_size: &winit::dpi::PhysicalSize<u32>,
        texture_updates: Vec<(egui::TextureId, egui::epaint::image::ImageDelta)>,
        primitives: Vec<egui::ClippedPrimitive>,
        pixels_per_point: f32,
    ) -> anyhow::Result<Option<RenderData>> {
        let device = &graphics_resources.device;
        let memory_allocator = &graphics_resources.memory_allocator;
        let descriptor_set_allocator = &graphics_resources.descriptor_set_allocator;
        let width = physical_size.width as f32;
        let height = physical_size.height as f32;
        // Update textures.
        self.update_textures(
            device,
            &(memory_allocator.clone() as Arc<_>),
            &(descriptor_set_allocator.clone() as Arc<_>),
            command_buffer,
            texture_updates,
        )
        .context("Error while updating textures")?;
        // Generate mesh data.
        let mut vertices: Vec<Vertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        let mut meshes: Vec<RenderMeshInfo> = Vec::with_capacity(primitives.len());
        for egui::ClippedPrimitive {
            clip_rect,
            primitive,
        } in primitives
        {
            if clip_rect.area() == 0.0 {
                continue;
            }
            let egui::epaint::Primitive::Mesh(mesh) = primitive else {
                continue;
            };
            let base_vertex = vertices.len() as i32;
            let indices_start = indices.len() as u32;
            vertices.extend(mesh.vertices.into_iter().map(|v| Vertex {
                pos: [v.pos.x * pixels_per_point, v.pos.y * pixels_per_point],
                uvs: [v.uv.x, v.uv.y],
                color: v.color.to_array(),
            }));
            indices.extend(mesh.indices);
            let indices_end = indices.len() as u32;
            meshes.push(RenderMeshInfo {
                scissor_rect: VulkanScissor {
                    offset: [
                        (clip_rect.min.x * pixels_per_point).min(width) as u32,
                        (clip_rect.min.y * pixels_per_point).min(height) as u32,
                    ],
                    extent: [
                        ((clip_rect.max.x - clip_rect.min.x) * pixels_per_point).min(width) as u32,
                        ((clip_rect.max.y - clip_rect.min.y) * pixels_per_point).min(height) as u32,
                    ],
                },
                base_vertex,
                index_slice: indices_start..indices_end,
                texture_id: mesh.texture_id,
            });
        }
        // Vulkano doesn't allow empty buffers, so just indicate no rendering needs doing.
        if vertices.is_empty() {
            return Ok(None);
        }
        let vertex_buffer = VulkanBuffer::from_iter(
            memory_allocator,
            &VulkanBufferCreateInfo {
                usage: VulkanBufferUsage::VERTEX_BUFFER | VulkanBufferUsage::STORAGE_BUFFER,
                ..Default::default()
            },
            &VulkanAllocationCreateInfo {
                memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                    | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            vertices,
        )
        .context("Error while creating egui vertex buffer")?;
        if indices.is_empty() {
            return Ok(None);
        }
        let index_buffer = VulkanBuffer::from_iter(
            memory_allocator,
            &VulkanBufferCreateInfo {
                usage: VulkanBufferUsage::INDEX_BUFFER | VulkanBufferUsage::STORAGE_BUFFER,
                ..Default::default()
            },
            &VulkanAllocationCreateInfo {
                memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                    | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            indices,
        )
        .context("Error while creating egui index buffer")?;
        Ok(Some(RenderData {
            meshes,
            vertex_buffer,
            index_buffer,
        }))
    }

    /// Requires a render subpass to have been started.
    /// Clobbers the render subpass scissor rect.
    pub fn render(
        &mut self,
        command_buffer: &mut VulkanAutoCommandBufferBuilder<VulkanPrimaryAutoCommandBuffer>,
        common_descriptor_set: Arc<VulkanDescriptorSet>,
        render_data: RenderData,
    ) {
        command_buffer
            .bind_pipeline_graphics(self.pipeline.clone())
            .unwrap()
            .bind_descriptor_sets(
                VulkanPipelineBindPoint::Graphics,
                self.pipeline_layout.clone(),
                0,
                common_descriptor_set,
            )
            .unwrap()
            .bind_vertex_buffers(0, [render_data.vertex_buffer])
            .unwrap()
            .bind_index_buffer(render_data.index_buffer)
            .unwrap();
        let mut last_texture_id: Option<egui::TextureId> = None;
        for mesh in &render_data.meshes {
            if last_texture_id != Some(mesh.texture_id) {
                last_texture_id = Some(mesh.texture_id);
                command_buffer
                    .bind_descriptor_sets(
                        VulkanPipelineBindPoint::Graphics,
                        self.pipeline_layout.clone(),
                        1,
                        vec![self.images[&mesh.texture_id].descriptor_set.clone()],
                    )
                    .unwrap();
            }
            command_buffer
                .set_scissor(0, SmallVec::from(&[mesh.scissor_rect] as &[_]))
                .unwrap();
            unsafe {
                command_buffer
                    .draw_indexed(
                        mesh.index_slice.end - mesh.index_slice.start,
                        1, // Instance count
                        mesh.index_slice.start,
                        mesh.base_vertex,
                        0, // Instance start
                    )
                    .unwrap();
            }
        }
    }

    pub fn register_user_image(
        &mut self,
        device: &Arc<VulkanDevice>,
        descriptor_set_allocator: &Arc<dyn VulkanDescriptorSetAllocator>,
        image: Arc<VulkanImage>,
        options: egui::TextureOptions,
    ) -> anyhow::Result<egui::TextureId> {
        let texture_id = egui::TextureId::User(self.next_user_texture_id);
        self.next_user_texture_id += 1;
        let sampler = self
            .sampler_cache
            .entry(options)
            .or_insert_with(|| {
                use egui::{TextureFilter, TextureWrapMode};
                fn convert_filter(filter: TextureFilter) -> VulkanFilter {
                    match filter {
                        TextureFilter::Nearest => VulkanFilter::Nearest,
                        TextureFilter::Linear => VulkanFilter::Linear,
                    }
                }
                let address_mode = match options.wrap_mode {
                    TextureWrapMode::ClampToEdge => VulkanSamplerAddressMode::ClampToEdge,
                    TextureWrapMode::Repeat => VulkanSamplerAddressMode::Repeat,
                    TextureWrapMode::MirroredRepeat => VulkanSamplerAddressMode::MirroredRepeat,
                };
                VulkanSampler::new(
                    device,
                    &VulkanSamplerCreateInfo {
                        mag_filter: convert_filter(options.magnification),
                        min_filter: convert_filter(options.minification),
                        address_mode: [address_mode; 3],
                        ..Default::default()
                    },
                )
                .context("Error while creating egui image sampler")
                .unwrap()
            })
            .clone();
        let view =
            VulkanImageView::new_default(&image).context("Error while creating egui image view")?;
        let descriptor_set = VulkanDescriptorSet::new(
            descriptor_set_allocator.clone(),
            self.image_descriptor_set_layout.clone(),
            [VulkanWriteDescriptorSet::image(
                0,
                VulkanDescriptorImageInfo {
                    sampler: Some(sampler),
                    image_view: Some(view),
                    ..Default::default()
                },
            )],
            [],
        )
        .context("Error while creating egui user image descriptor set")?;
        self.images.insert(
            texture_id,
            ImageData {
                image,
                descriptor_set,
            },
        );
        Ok(texture_id)
    }
}
