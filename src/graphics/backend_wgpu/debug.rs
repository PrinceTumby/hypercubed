use super::{Texture, include_wesl_module};
use crate::graphics::debug::{Line as DebugLine, Point as DebugPoint, Triangle as DebugTriangle};

pub mod point {
    use super::*;

    pub fn create_render_pipeline(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        layout: &wgpu::PipelineLayout,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(include_wesl_module!("debug_point"));
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Debug Point Render Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: None,
                compilation_options: Default::default(),
                buffers: &[Some(DebugPoint::wgpu_desc())],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: None,
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                // We're just presorting these to get the order right, doesn't matter too much.
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        })
    }

    impl DebugPoint {
        const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
            // pos
            0 => Float32x3,
            // size
            1 => Float32,
            // colour
            2 => Unorm8x4,
            // flags
            3 => Uint32,
        ];

        pub const fn wgpu_desc() -> wgpu::VertexBufferLayout<'static> {
            wgpu::VertexBufferLayout {
                array_stride: core::mem::size_of::<Self>() as wgpu::BufferAddress,
                // WGPU doesn't support point sizes, so we internally convert to quads in the
                // shader.
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: Self::ATTRIBUTES,
            }
        }
    }
}

pub mod line {
    use super::*;

    pub fn create_render_pipeline(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        layout: &wgpu::PipelineLayout,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(include_wesl_module!("debug_line"));
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Debug Line Render Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: None,
                compilation_options: Default::default(),
                buffers: &[Some(DebugLine::wgpu_desc())],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: None,
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                // WGPU doesn't support line widths, so we internally convert to quads in the
                // shader.
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                // We're just presorting these to get the order right, doesn't matter too much.
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        })
    }

    impl DebugLine {
        const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
            // p1
            0 => Float32x3,
            // p2
            1 => Float32x3,
            // size
            2 => Float32,
            // colour
            3 => Unorm8x4,
            // flags
            4 => Uint32,
        ];

        pub const fn wgpu_desc() -> wgpu::VertexBufferLayout<'static> {
            wgpu::VertexBufferLayout {
                array_stride: core::mem::size_of::<Self>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: Self::ATTRIBUTES,
            }
        }
    }
}

pub mod triangle {
    use super::*;

    pub fn create_render_pipeline(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        layout: &wgpu::PipelineLayout,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(include_wesl_module!("debug_triangle"));
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Debug Triangle Render Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: None,
                compilation_options: Default::default(),
                buffers: &[Some(DebugTriangle::wgpu_desc())],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: None,
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                // We're just presorting these to get the order right, doesn't matter too much.
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        })
    }

    impl DebugTriangle {
        const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
            // p1
            0 => Float32x3,
            // p2
            1 => Float32x3,
            // p3
            2 => Float32x3,
            // colour
            3 => Unorm8x4,
            // flags
            4 => Uint32,
        ];

        pub fn wgpu_desc() -> wgpu::VertexBufferLayout<'static> {
            wgpu::VertexBufferLayout {
                array_stride: core::mem::size_of::<Self>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: Self::ATTRIBUTES,
            }
        }
    }
}
