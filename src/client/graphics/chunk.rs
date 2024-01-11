use super::Texture;
use nalgebra::{Matrix3, Rotation3, Vector3};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
    include_wgsl, vertex_attr_array, Buffer, BufferSlice, Device, PipelineLayout, RenderPipeline,
    RenderPipelineDescriptor, SurfaceConfiguration, VertexAttribute,
};

pub mod block_face {
    use super::*;

    pub fn create_render_pipeline(
        device: &Device,
        config: &SurfaceConfiguration,
        layout: &PipelineLayout,
    ) -> RenderPipeline {
        let shader = device.create_shader_module(include_wgsl!("shaders/block_face.wgsl"));
        device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Block Face Render Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc(), Instance::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        })
    }

    pub mod face_matrices {
        use super::*;

        pub fn generate_array() -> [[[f32; 4]; 3]; 6] {
            let matrix_arrays: [[[f32; 3]; 3]; 6] = [
                // Top
                Matrix3::identity().into(),
                // Bottom
                Matrix3::from(Rotation3::from_euler_angles(std::f32::consts::PI, 0.0, 0.0)).into(),
                // North
                Matrix3::from(Rotation3::from_euler_angles(
                    -std::f32::consts::FRAC_PI_2,
                    0.0,
                    std::f32::consts::PI,
                ))
                .into(),
                // South
                Matrix3::from(Rotation3::from_euler_angles(
                    std::f32::consts::FRAC_PI_2,
                    0.0,
                    0.0,
                ))
                .into(),
                // East
                Matrix3::from(Rotation3::from_euler_angles(
                    0.0,
                    std::f32::consts::FRAC_PI_2,
                    -std::f32::consts::FRAC_PI_2,
                ))
                .into(),
                // West
                Matrix3::from(Rotation3::from_euler_angles(
                    0.0,
                    -std::f32::consts::FRAC_PI_2,
                    std::f32::consts::FRAC_PI_2,
                ))
                .into(),
            ];
            // Alignment of each row in a mat3x3 is same as vec4, so we pad up to size
            matrix_arrays.map(|matrix| matrix.map(|[x, y, z]| [x, y, z, 0.0]))
        }

        pub mod indices {
            pub const TOP: u8 = 0;
            pub const BOTTOM: u8 = 1;
            pub const NORTH: u8 = 2;
            pub const SOUTH: u8 = 3;
            pub const EAST: u8 = 4;
            pub const WEST: u8 = 5;
        }
    }

    pub mod rotation_matrices {
        use super::*;

        pub fn generate_x_rotation_array() -> [[[f32; 4]; 3]; 4] {
            let matrix_arrays: [[[f32; 3]; 3]; 4] = [
                // Zero
                Matrix3::identity().into(),
                // Ninety
                Matrix3::from(Rotation3::from_axis_angle(
                    &Vector3::x_axis(),
                    std::f32::consts::FRAC_PI_2,
                ))
                .into(),
                // One-Eighty
                Matrix3::from(Rotation3::from_axis_angle(
                    &Vector3::x_axis(),
                    std::f32::consts::PI,
                ))
                .into(),
                // Two-Seventy
                Matrix3::from(Rotation3::from_axis_angle(
                    &Vector3::x_axis(),
                    -std::f32::consts::FRAC_PI_2,
                ))
                .into(),
            ];
            // Alignment of each row in a mat3x3 is same as vec4, so we pad up to size
            matrix_arrays.map(|matrix| matrix.map(|[x, y, z]| [x, y, z, 0.0]))
        }

        pub fn generate_y_rotation_array() -> [[[f32; 4]; 3]; 4] {
            let matrix_arrays: [[[f32; 3]; 3]; 4] = [
                // Zero
                Matrix3::identity().into(),
                // Ninety
                Matrix3::from(Rotation3::from_axis_angle(
                    &Vector3::y_axis(),
                    std::f32::consts::FRAC_PI_2,
                ))
                .into(),
                // One-Eighty
                Matrix3::from(Rotation3::from_axis_angle(
                    &Vector3::y_axis(),
                    std::f32::consts::PI,
                ))
                .into(),
                // Two-Seventy
                Matrix3::from(Rotation3::from_axis_angle(
                    &Vector3::y_axis(),
                    -std::f32::consts::FRAC_PI_2,
                ))
                .into(),
            ];
            // Alignment of each row in a mat3x3 is same as vec4, so we pad up to size
            matrix_arrays.map(|matrix| matrix.map(|[x, y, z]| [x, y, z, 0.0]))
        }

        pub mod indices {
            pub const ZERO: u8 = 0;
            pub const NINETY: u8 = 1;
            pub const ONE_EIGHTY: u8 = 2;
            pub const TWO_SEVENTY: u8 = 3;
        }
    }

    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct Vertex {
        pub pos: [f32; 3],
        pub uvs: [f32; 2],
        pub normal: [f32; 3],
    }

    impl Vertex {
        const ATTRIBUTES: &'static [VertexAttribute] = &vertex_attr_array![
            // pos
            0 => Float32x3,
            // uvs
            1 => Float32x2,
            // normal
            2 => Float32x3,
        ];

        pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: Self::ATTRIBUTES,
            }
        }
    }

    #[derive(Debug)]
    pub struct InstanceList {
        instance_buffer: Buffer,
        num_instances: u32,
    }

    impl InstanceList {
        pub fn new(device: &Device, instances: &[Instance]) -> Self {
            Self {
                instance_buffer: device.create_buffer_init(&BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(instances),
                    usage: wgpu::BufferUsages::VERTEX,
                }),
                num_instances: instances.len().try_into().unwrap(),
            }
        }

        pub fn get_instances_slice(&self) -> BufferSlice {
            self.instance_buffer.slice(..)
        }

        pub fn num_instances(&self) -> u32 {
            self.num_instances
        }
    }

    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct Instance {
        pub pos: [f32; 3],
        pub uvs: [u16; 4],
        /// In order of: face matrix, X rotation matrix, Y rotation matrix, unused
        pub matrix_indices: [u8; 4],
    }

    impl Instance {
        const ATTRIBUTES: &'static [VertexAttribute] = &vertex_attr_array![
            // pos
            10 => Float32x3,
            11 => Uint16x4,
            12 => Uint8x4,
        ];

        pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: Self::ATTRIBUTES,
            }
        }
    }

    // Block top face vertices. Multiplied by face matrices to get other faces.

    pub const VERTICES: &[Vertex] = &[
        Vertex {
            pos: [-0.5, 0.5, 0.5],
            uvs: [0.0, 1.0],
            normal: [0.0, 1.0, -0.0],
        },
        Vertex {
            pos: [0.5, 0.5, 0.5],
            uvs: [1.0, 1.0],
            normal: [0.0, 1.0, -0.0],
        },
        Vertex {
            pos: [-0.5, 0.5, -0.5],
            uvs: [0.0, 0.0],
            normal: [0.0, 1.0, -0.0],
        },
        Vertex {
            pos: [0.5, 0.5, -0.5],
            uvs: [1.0, 0.0],
            normal: [0.0, 1.0, -0.0],
        },
    ];

    pub const INDICES: &[[u16; 3]] = &[
        // Front
        [0, 1, 2],
        [2, 1, 3],
        // Back
        [4, 5, 6],
        [6, 5, 7],
        // Left
        [8, 9, 10],
        [10, 9, 11],
        // Right
        [12, 13, 14],
        [14, 13, 15],
        // Top
        [16, 17, 18],
        [18, 17, 19],
        // Bottom
        [20, 21, 22],
        [22, 21, 23],
    ];
}

pub mod tinted_block_face {
    use super::block_face::Vertex;
    use super::*;

    pub fn create_render_pipeline(
        device: &Device,
        config: &SurfaceConfiguration,
        layout: &PipelineLayout,
    ) -> RenderPipeline {
        let shader = device.create_shader_module(include_wgsl!("shaders/tinted_block_face.wgsl"));
        device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Tinted Block Face Render Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc(), Instance::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        })
    }

    #[derive(Debug)]
    pub struct InstanceList {
        instance_buffer: Buffer,
        num_instances: u32,
    }

    impl InstanceList {
        pub fn new(device: &Device, instances: &[Instance]) -> Self {
            Self {
                instance_buffer: device.create_buffer_init(&BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(instances),
                    usage: wgpu::BufferUsages::VERTEX,
                }),
                num_instances: instances.len().try_into().unwrap(),
            }
        }

        pub fn get_instances_slice(&self) -> BufferSlice {
            self.instance_buffer.slice(..)
        }

        pub fn num_instances(&self) -> u32 {
            self.num_instances
        }
    }

    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct Instance {
        pub pos: [f32; 3],
        pub uvs: [u16; 4],
        /// In order of: face matrix, X rotation matrix, Y rotation matrix, unused
        pub matrix_indices: [u8; 4],
        pub tint_color: [u8; 4],
    }

    impl Instance {
        const ATTRIBUTES: &'static [VertexAttribute] = &vertex_attr_array![
            // pos
            10 => Float32x3,
            11 => Uint16x4,
            12 => Uint8x4,
            13 => Unorm8x4,
        ];

        pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: Self::ATTRIBUTES,
            }
        }
    }
}
