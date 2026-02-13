use super::shader_exports::shader_stage_from_entry_point;
use anyhow::Context;
use std::sync::Arc;
use vulkan_prelude::*;

pub mod sky {
    use super::*;

    pub fn create_sun_graphics_pipeline(
        device: &Arc<VulkanDevice>,
        layout: &Arc<VulkanPipelineLayout>,
        subpass: &VulkanSubpass,
    ) -> anyhow::Result<Arc<VulkanGraphicsPipeline>> {
        VulkanGraphicsPipeline::new(
            device,
            None, // No pipeline cache.
            &VulkanGraphicsPipelineCreateInfo {
                flags: VulkanPipelineCreateFlags::default(),
                stages: &[
                    shader_stage_from_entry_point(&mut None, device, "shader::sky::sun::vertex"),
                    shader_stage_from_entry_point(&mut None, device, "shader::sky::sun::fragment"),
                ],
                // The shader generates its own vertex information, so nothing here.
                vertex_input_state: Some(&VulkanVertexInputState::new()),
                input_assembly_state: Some(&VulkanInputAssemblyState {
                    topology: VulkanPrimitiveTopology::TriangleStrip,
                    primitive_restart_enable: false,
                    ..Default::default()
                }),
                tessellation_state: None,
                // We leave the viewport state as a single default viewport, as we use dynamic
                // state to set the viewport at render time.
                viewport_state: Some(&Default::default()),
                rasterization_state: Some(&VulkanRasterizationState {
                    cull_mode: VulkanCullMode::None,
                    ..Default::default()
                }),
                multisample_state: Some(&Default::default()),
                depth_stencil_state: Some(&VulkanDepthStencilState {
                    depth: Some(VulkanDepthState {
                        write_enable: false,
                        compare_op: VulkanCompareOp::GreaterOrEqual,
                    }),
                    ..Default::default()
                }),
                color_blend_state: Some(&VulkanColorBlendState {
                    attachments: &[VulkanColorBlendAttachmentState {
                        blend: Some(VulkanAttachmentBlend {
                            src_color_blend_factor: VulkanBlendFactor::One,
                            dst_color_blend_factor: VulkanBlendFactor::One,
                            color_blend_op: VulkanBlendOp::Add,
                            src_alpha_blend_factor: VulkanBlendFactor::One,
                            dst_alpha_blend_factor: VulkanBlendFactor::One,
                            alpha_blend_op: VulkanBlendOp::Add,
                        }),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                dynamic_state: &[VulkanDynamicState::Viewport],
                layout,
                subpass: Some(VulkanPipelineSubpassType::BeginRenderPass(subpass)),
                base_pipeline: None,
                discard_rectangle_state: None,
                fragment_shading_rate_state: None,
                _ne: vulkano_non_exhaustive(),
            },
        )
        .context("Error while creating pipeline")
    }

    pub fn create_moon_graphics_pipeline(
        device: &Arc<VulkanDevice>,
        layout: &Arc<VulkanPipelineLayout>,
        subpass: &VulkanSubpass,
    ) -> anyhow::Result<Arc<VulkanGraphicsPipeline>> {
        VulkanGraphicsPipeline::new(
            device,
            None, // No pipeline cache.
            &VulkanGraphicsPipelineCreateInfo {
                flags: VulkanPipelineCreateFlags::default(),
                stages: &[
                    shader_stage_from_entry_point(&mut None, device, "shader::sky::moon::vertex"),
                    shader_stage_from_entry_point(&mut None, device, "shader::sky::moon::fragment"),
                ],
                // The shader generates its own vertex information, so nothing here.
                vertex_input_state: Some(&VulkanVertexInputState::new()),
                input_assembly_state: Some(&VulkanInputAssemblyState {
                    topology: VulkanPrimitiveTopology::TriangleStrip,
                    primitive_restart_enable: false,
                    ..Default::default()
                }),
                tessellation_state: None,
                // We leave the viewport state as a single default viewport, as we use dynamic
                // state to set the viewport at render time.
                viewport_state: Some(&Default::default()),
                rasterization_state: Some(&VulkanRasterizationState {
                    cull_mode: VulkanCullMode::None,
                    ..Default::default()
                }),
                multisample_state: Some(&Default::default()),
                depth_stencil_state: Some(&VulkanDepthStencilState {
                    depth: Some(VulkanDepthState {
                        write_enable: false,
                        compare_op: VulkanCompareOp::GreaterOrEqual,
                    }),
                    ..Default::default()
                }),
                color_blend_state: Some(&VulkanColorBlendState {
                    attachments: &[VulkanColorBlendAttachmentState {
                        blend: Some(VulkanAttachmentBlend {
                            src_color_blend_factor: VulkanBlendFactor::One,
                            dst_color_blend_factor: VulkanBlendFactor::One,
                            color_blend_op: VulkanBlendOp::Add,
                            src_alpha_blend_factor: VulkanBlendFactor::One,
                            dst_alpha_blend_factor: VulkanBlendFactor::One,
                            alpha_blend_op: VulkanBlendOp::Add,
                        }),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                dynamic_state: &[VulkanDynamicState::Viewport],
                layout,
                subpass: Some(VulkanPipelineSubpassType::BeginRenderPass(subpass)),
                base_pipeline: None,
                discard_rectangle_state: None,
                fragment_shading_rate_state: None,
                _ne: vulkano_non_exhaustive(),
            },
        )
        .context("Error while creating pipeline")
    }

    pub fn create_star_graphics_pipeline(
        device: &Arc<VulkanDevice>,
        layout: &Arc<VulkanPipelineLayout>,
        subpass: &VulkanSubpass,
    ) -> anyhow::Result<Arc<VulkanGraphicsPipeline>> {
        VulkanGraphicsPipeline::new(
            device,
            None, // No pipeline cache.
            &VulkanGraphicsPipelineCreateInfo {
                flags: VulkanPipelineCreateFlags::default(),
                stages: &[
                    shader_stage_from_entry_point(&mut None, device, "shader::sky::star::vertex"),
                    shader_stage_from_entry_point(&mut None, device, "shader::sky::star::fragment"),
                ],
                vertex_input_state: Some(&VulkanVertexInputState {
                    bindings: vulkan_vertex_bindings![
                        0 => ([StarVertex; 4], Instance),
                    ],
                    attributes: vulkan_vertex_attributes!(1, [
                        // `instance[0]`
                        [0 <- 0] => R32G32B32_SFLOAT,
                        // `instance[1]`
                        [1 <- 0] => R32G32B32_SFLOAT,
                        // `instance[2]`
                        [2 <- 0] => R32G32B32_SFLOAT,
                        // `instance[3]`
                        [3 <- 0] => R32G32B32_SFLOAT,
                    ]),
                    ..Default::default()
                }),
                input_assembly_state: Some(&VulkanInputAssemblyState {
                    topology: VulkanPrimitiveTopology::TriangleStrip,
                    primitive_restart_enable: false,
                    ..Default::default()
                }),
                tessellation_state: None,
                // We leave the viewport state as a single default viewport, as we use dynamic
                // state to set the viewport at render time.
                viewport_state: Some(&Default::default()),
                rasterization_state: Some(&VulkanRasterizationState {
                    cull_mode: VulkanCullMode::None,
                    ..Default::default()
                }),
                multisample_state: Some(&Default::default()),
                depth_stencil_state: Some(&VulkanDepthStencilState {
                    depth: Some(VulkanDepthState {
                        write_enable: false,
                        compare_op: VulkanCompareOp::GreaterOrEqual,
                    }),
                    ..Default::default()
                }),
                color_blend_state: Some(&VulkanColorBlendState {
                    attachments: &[VulkanColorBlendAttachmentState {
                        blend: Some(VulkanAttachmentBlend {
                            src_color_blend_factor: VulkanBlendFactor::One,
                            dst_color_blend_factor: VulkanBlendFactor::One,
                            color_blend_op: VulkanBlendOp::Add,
                            src_alpha_blend_factor: VulkanBlendFactor::One,
                            dst_alpha_blend_factor: VulkanBlendFactor::One,
                            alpha_blend_op: VulkanBlendOp::Add,
                        }),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                dynamic_state: &[VulkanDynamicState::Viewport],
                layout,
                subpass: Some(VulkanPipelineSubpassType::BeginRenderPass(subpass)),
                base_pipeline: None,
                discard_rectangle_state: None,
                fragment_shading_rate_state: None,
                _ne: vulkano_non_exhaustive(),
            },
        )
        .context("Error while creating pipeline")
    }

    #[repr(transparent)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct StarVertex(pub [f32; 3]);
}
