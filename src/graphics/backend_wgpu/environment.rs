use super::{Texture, include_wesl_module};

pub mod sky {
    use super::*;

    pub fn create_sun_render_pipeline(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        layout: &wgpu::PipelineLayout,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(include_wesl_module!("sun"));
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Sun Render Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: None,
                compilation_options: Default::default(),
                buffers: &[],
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
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
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

    pub fn create_moon_render_pipeline(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        layout: &wgpu::PipelineLayout,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(include_wesl_module!("moon"));
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Moon Render Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: None,
                compilation_options: Default::default(),
                buffers: &[],
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
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
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

    pub fn create_star_render_pipeline(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        layout: &wgpu::PipelineLayout,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(include_wesl_module!("star"));
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Star Render Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: None,
                compilation_options: Default::default(),
                buffers: &[Some(StarInstance::desc())],
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
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
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

    #[repr(transparent)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct StarVertex(pub [f32; 3]);

    #[repr(transparent)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct StarInstance(pub [StarVertex; 4]);

    impl StarInstance {
        const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
            // `[0]`
            0 => Float32x3,
            // `[1]`
            1 => Float32x3,
            // `[2]`
            2 => Float32x3,
            // `[3]`
            3 => Float32x3,
        ];

        pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
            wgpu::VertexBufferLayout {
                array_stride: core::mem::size_of::<Self>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: Self::ATTRIBUTES,
            }
        }
    }
}
