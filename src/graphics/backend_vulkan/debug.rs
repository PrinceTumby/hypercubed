use super::shader_exports::debug::types::{self as shader_debug_types, vertex_input_state};
use super::shader_exports::shader_stage_from_entry_point;
use anyhow::Context;
use std::sync::Arc;
use vulkan_prelude::*;

pub use shader_debug_types::PackedFlags;

pub mod point {
    use super::*;

    pub fn create_graphics_pipeline(
        device: &Arc<VulkanDevice>,
        layout: &Arc<VulkanPipelineLayout>,
        subpass: &VulkanSubpass,
    ) -> anyhow::Result<Arc<VulkanGraphicsPipeline>> {
        VulkanGraphicsPipeline::new(
            device,
            None, // No pipeline cache
            &VulkanGraphicsPipelineCreateInfo {
                flags: VulkanPipelineCreateFlags::default(),
                stages: &[
                    shader_stage_from_entry_point(
                        &mut None,
                        device,
                        "shader::debug::point::vertex",
                    ),
                    shader_stage_from_entry_point(
                        &mut None,
                        device,
                        "shader::debug::point::fragment",
                    ),
                ],
                vertex_input_state: Some(&vertex_input_state::point()),
                input_assembly_state: Some(&VulkanInputAssemblyState {
                    topology: VulkanPrimitiveTopology::PointList,
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
                        // We're just presorting these to get the order right, doesn't matter too
                        // much.
                        write_enable: false,
                        compare_op: VulkanCompareOp::GreaterOrEqual,
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
                dynamic_state: &[VulkanDynamicState::Viewport],
                layout,
                subpass: Some(VulkanPipelineSubpassType::BeginRenderPass(subpass)),
                base_pipeline: None,
                discard_rectangle_state: None,
                fragment_shading_rate_state: None,
                _ne: vulkano_non_exhaustive(),
            },
        )
        .context("Error while creating debug point graphics pipeline")
    }

    pub use shader_debug_types::PointVertex as Vertex;
}

pub mod line {
    use super::*;

    pub fn create_graphics_pipeline(
        device: &Arc<VulkanDevice>,
        layout: &Arc<VulkanPipelineLayout>,
        subpass: &VulkanSubpass,
    ) -> anyhow::Result<Arc<VulkanGraphicsPipeline>> {
        VulkanGraphicsPipeline::new(
            device,
            None, // No pipeline cache
            &VulkanGraphicsPipelineCreateInfo {
                flags: VulkanPipelineCreateFlags::default(),
                stages: &[
                    shader_stage_from_entry_point(&mut None, device, "shader::debug::line::vertex"),
                    shader_stage_from_entry_point(
                        &mut None,
                        device,
                        "shader::debug::line::fragment",
                    ),
                ],
                vertex_input_state: Some(&vertex_input_state::line()),
                input_assembly_state: Some(&VulkanInputAssemblyState {
                    // Vulkan doesn't support line widths, so we internally convert to quads in the
                    // shader.
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
                        // We're just presorting these to get the order right, doesn't matter too
                        // much.
                        write_enable: false,
                        compare_op: VulkanCompareOp::GreaterOrEqual,
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
                dynamic_state: &[VulkanDynamicState::Viewport],
                layout,
                subpass: Some(VulkanPipelineSubpassType::BeginRenderPass(subpass)),
                base_pipeline: None,
                discard_rectangle_state: None,
                fragment_shading_rate_state: None,
                _ne: vulkano_non_exhaustive(),
            },
        )
        .context("Error while creating debug line graphics pipeline")
    }

    pub use shader_debug_types::LineInstance as Instance;
}

pub mod triangle {
    use super::*;

    pub fn create_graphics_pipeline(
        device: &Arc<VulkanDevice>,
        layout: &Arc<VulkanPipelineLayout>,
        subpass: &VulkanSubpass,
    ) -> anyhow::Result<Arc<VulkanGraphicsPipeline>> {
        VulkanGraphicsPipeline::new(
            device,
            None, // No pipeline cache
            &VulkanGraphicsPipelineCreateInfo {
                flags: VulkanPipelineCreateFlags::default(),
                stages: &[
                    shader_stage_from_entry_point(
                        &mut None,
                        device,
                        "shader::debug::triangle::vertex",
                    ),
                    shader_stage_from_entry_point(
                        &mut None,
                        device,
                        "shader::debug::triangle::fragment",
                    ),
                ],
                vertex_input_state: Some(&vertex_input_state::triangle()),
                input_assembly_state: Some(&VulkanInputAssemblyState {
                    topology: VulkanPrimitiveTopology::TriangleList,
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
                        // We're just presorting these to get the order right, doesn't matter too
                        // much.
                        write_enable: false,
                        compare_op: VulkanCompareOp::GreaterOrEqual,
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
                dynamic_state: &[VulkanDynamicState::Viewport],
                layout,
                subpass: Some(VulkanPipelineSubpassType::BeginRenderPass(subpass)),
                base_pipeline: None,
                discard_rectangle_state: None,
                fragment_shading_rate_state: None,
                _ne: vulkano_non_exhaustive(),
            },
        )
        .context("Error while creating debug triangle graphics pipeline")
    }

    pub use shader_debug_types::TriangleInstance as Instance;
}
