use super::Texture;
use crate::graphics::debug::{Line as DebugLine, Point as DebugPoint, Triangle as DebugTriangle};
use wgpu::{include_wgsl, vertex_attr_array};

pub mod point {
    use super::*;

    pub fn create_render_pipeline(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        layout: &wgpu::PipelineLayout,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(include_wgsl!("shaders/debug_point.wgsl"));
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Debug Point Render Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: None,
                compilation_options: Default::default(),
                buffers: &[DebugPoint::wgpu_desc()],
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
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::GreaterEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        })
    }

    impl DebugPoint {
        const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &vertex_attr_array![
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
        let shader = device.create_shader_module(include_wgsl!("shaders/debug_line.wgsl"));
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Debug Line Render Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: None,
                compilation_options: Default::default(),
                buffers: &[DebugLine::wgpu_desc()],
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
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::GreaterEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        })
    }

    impl DebugLine {
        const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &vertex_attr_array![
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
        let shader = device.create_shader_module(include_wgsl!("shaders/debug_triangle.wgsl"));
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Debug Triangle Render Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: None,
                compilation_options: Default::default(),
                buffers: &[DebugTriangle::wgpu_desc()],
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
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::GreaterEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        })
    }

    impl DebugTriangle {
        const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &vertex_attr_array![
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

pub mod crosshair {
    use super::*;

    pub fn create_render_pipeline(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        layout: &wgpu::PipelineLayout,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(include_wgsl!("shaders/debug_crosshair.wgsl"));
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Debug Crosshair Render Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: None,
                compilation_options: Default::default(),
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: None,
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        })
    }

    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct Vertex {
        pub pos: [f32; 3],
        pub colour: [u8; 4],
    }

    impl Vertex {
        const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &vertex_attr_array![
            // pos
            0 => Float32x3,
            // colour
            1 => Unorm8x4,
        ];

        pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
            wgpu::VertexBufferLayout {
                array_stride: core::mem::size_of::<Self>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: Self::ATTRIBUTES,
            }
        }
    }

    pub const VERTICES: &[Vertex] = &[
        // +X
        Vertex {
            pos: [0.0, 0.0, 0.0],
            colour: [0xFF, 0x00, 0x00, 0xFF],
        },
        Vertex {
            pos: [1.0, 0.0, 0.0],
            colour: [0xFF, 0x00, 0x00, 0xFF],
        },
        // +Y
        Vertex {
            pos: [0.0, 0.0, 0.0],
            colour: [0x00, 0xFF, 0x00, 0xFF],
        },
        Vertex {
            pos: [0.0, 1.0, 0.0],
            colour: [0x00, 0xFF, 0x00, 0xFF],
        },
        // +Z
        Vertex {
            pos: [0.0, 0.0, 0.0],
            colour: [0x00, 0x00, 0xFF, 0xFF],
        },
        Vertex {
            pos: [0.0, 0.0, 1.0],
            colour: [0x00, 0x00, 0xFF, 0xFF],
        },
    ];
}
