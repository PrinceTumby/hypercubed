use super::Texture;
use wgpu::{
    Device, PipelineLayout, RenderPipeline, RenderPipelineDescriptor, SurfaceConfiguration,
    VertexAttribute, include_wgsl, vertex_attr_array,
};

pub mod crosshair {
    use super::*;

    pub fn create_render_pipeline(
        device: &Device,
        config: &SurfaceConfiguration,
        layout: &PipelineLayout,
    ) -> RenderPipeline {
        let shader = device.create_shader_module(include_wgsl!("shaders/debug_crosshair.wgsl"));
        device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Debug Crosshair Render Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                compilation_options: Default::default(),
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
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
        pub color: [u8; 4],
    }

    impl Vertex {
        const ATTRIBUTES: &'static [VertexAttribute] = &vertex_attr_array![
            // pos
            0 => Float32x3,
            // color
            1 => Unorm8x4,
        ];

        pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: Self::ATTRIBUTES,
            }
        }
    }

    pub const VERTICES: &[Vertex] = &[
        // +X
        Vertex {
            pos: [0.0, 0.0, 0.0],
            color: [0xFF, 0x00, 0x00, 0xFF],
        },
        Vertex {
            pos: [1.0, 0.0, 0.0],
            color: [0xFF, 0x00, 0x00, 0xFF],
        },
        // +Y
        Vertex {
            pos: [0.0, 0.0, 0.0],
            color: [0x00, 0xFF, 0x00, 0xFF],
        },
        Vertex {
            pos: [0.0, 1.0, 0.0],
            color: [0x00, 0xFF, 0x00, 0xFF],
        },
        // +Z
        Vertex {
            pos: [0.0, 0.0, 0.0],
            color: [0x00, 0x00, 0xFF, 0xFF],
        },
        Vertex {
            pos: [0.0, 0.0, 1.0],
            color: [0x00, 0x00, 0xFF, 0xFF],
        },
    ];
}
