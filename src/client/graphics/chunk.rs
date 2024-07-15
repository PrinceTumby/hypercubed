use super::Texture;
use crate::basic_types::AxisDirection;
use nalgebra::{Matrix3, Rotation3, Vector3};
use std::marker::PhantomData;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
    include_wgsl, vertex_attr_array, Buffer, BufferSlice, Device, PipelineLayout, RenderPipeline,
    RenderPipelineDescriptor, SurfaceConfiguration, VertexAttribute,
};

pub struct Subchunk {
    pub start_coords: [i32; 3],
    pub block_faces: block_face::InstanceList,
    pub tinted_block_faces: tinted_block_face::InstanceList,
    pub custom_block_info: Option<SubchunkCustomBlockInfo>,
    pub connected_faces: SubchunkConnectivity,
}

pub struct SubchunkCustomBlockInfo {
    // TODO: Pre-generate big buffers for all custom block vertices and indices
    pub vertices: custom_block::VertexList,
    pub indices: custom_block::IndexList,
    pub instances: custom_block::InstanceList,
    pub draw_args: custom_block::DrawArgsList,
}

// Bits (least to most significant) store if each of these pairs of faces are connected:
// 0: Down-Up
// 1: Down-North
// 2: Down-South
// 3: Down-West
// 4: Down-East
// 5: Up-North
// 6: Up-South
// 7: Up-West
// 8: Up-East
// 9: North-South
// 10: North-West
// 11: North-East
// 12: South-West
// 13: South-East
// 14: West-East
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubchunkConnectivity(u16);

impl SubchunkConnectivity {
    pub fn empty() -> Self {
        Self(0)
    }

    pub fn full() -> Self {
        Self(0x7FFF)
    }

    pub fn add_connection(&mut self, face_1: &AxisDirection, face_2: &AxisDirection) {
        use AxisDirection::*;
        match (face_1, face_2) {
            (&Down, &Down) | (&Up, &Up) => {}
            (&North, &North) | (&South, &South) => {}
            (&West, &West) | (&East, &East) => {}
            (&Down, &Up) | (&Up, &Down) => self.0 |= 0x1,
            (&Down, &North) | (&North, &Down) => self.0 |= 0x2,
            (&Down, &South) | (&South, &Down) => self.0 |= 0x4,
            (&Down, &West) | (&West, &Down) => self.0 |= 0x8,
            (&Down, &East) | (&East, &Down) => self.0 |= 0x10,
            (&Up, &North) | (&North, &Up) => self.0 |= 0x20,
            (&Up, &South) | (&South, &Up) => self.0 |= 0x40,
            (&Up, &West) | (&West, &Up) => self.0 |= 0x80,
            (&Up, &East) | (&East, &Up) => self.0 |= 0x100,
            (&North, &South) | (&South, &North) => self.0 |= 0x200,
            (&North, &West) | (&West, &North) => self.0 |= 0x400,
            (&North, &East) | (&East, &North) => self.0 |= 0x800,
            (&South, &West) | (&West, &South) => self.0 |= 0x1000,
            (&South, &East) | (&East, &South) => self.0 |= 0x2000,
            (&West, &East) | (&East, &West) => self.0 |= 0x4000,
        }
    }

    pub fn connects(&self, face_1: &AxisDirection, face_2: &AxisDirection) -> bool {
        use AxisDirection::*;
        match (face_1, face_2) {
            (&Down, &Down) | (&Up, &Up) => true,
            (&North, &North) | (&South, &South) => true,
            (&West, &West) | (&East, &East) => true,
            (&Down, &Up) | (&Up, &Down) => self.0 & 0x1 != 0,
            (&Down, &North) | (&North, &Down) => self.0 & 0x2 != 0,
            (&Down, &South) | (&South, &Down) => self.0 & 0x4 != 0,
            (&Down, &West) | (&West, &Down) => self.0 & 0x8 != 0,
            (&Down, &East) | (&East, &Down) => self.0 & 0x10 != 0,
            (&Up, &North) | (&North, &Up) => self.0 & 0x20 != 0,
            (&Up, &South) | (&South, &Up) => self.0 & 0x40 != 0,
            (&Up, &West) | (&West, &Up) => self.0 & 0x80 != 0,
            (&Up, &East) | (&East, &Up) => self.0 & 0x100 != 0,
            (&North, &South) | (&South, &North) => self.0 & 0x200 != 0,
            (&North, &West) | (&West, &North) => self.0 & 0x400 != 0,
            (&North, &East) | (&East, &North) => self.0 & 0x800 != 0,
            (&South, &West) | (&West, &South) => self.0 & 0x1000 != 0,
            (&South, &East) | (&East, &South) => self.0 & 0x2000 != 0,
            (&West, &East) | (&East, &West) => self.0 & 0x4000 != 0,
        }
    }

    pub fn get_pairs(&self) -> [([AxisDirection; 2], bool); 15] {
        let fields = [
            ([AxisDirection::Down, AxisDirection::Up], 0x1),
            ([AxisDirection::Down, AxisDirection::North], 0x2),
            ([AxisDirection::Down, AxisDirection::South], 0x4),
            ([AxisDirection::Down, AxisDirection::West], 0x8),
            ([AxisDirection::Down, AxisDirection::East], 0x10),
            ([AxisDirection::Up, AxisDirection::North], 0x20),
            ([AxisDirection::Up, AxisDirection::South], 0x40),
            ([AxisDirection::Up, AxisDirection::West], 0x80),
            ([AxisDirection::Up, AxisDirection::East], 0x100),
            ([AxisDirection::North, AxisDirection::South], 0x200),
            ([AxisDirection::North, AxisDirection::West], 0x400),
            ([AxisDirection::North, AxisDirection::East], 0x800),
            ([AxisDirection::South, AxisDirection::West], 0x1000),
            ([AxisDirection::South, AxisDirection::East], 0x2000),
            ([AxisDirection::West, AxisDirection::East], 0x4000),
        ];
        fields.map(|(dirs, mask)| (dirs, self.0 & mask != 0))
    }
}

impl std::fmt::Debug for SubchunkConnectivity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let mut debug_set = f.debug_set();
        let fields = [
            ("down_up", 0x1),
            ("down_north", 0x2),
            ("down_south", 0x4),
            ("down_west", 0x8),
            ("down_east", 0x10),
            ("up_north", 0x20),
            ("up_south", 0x40),
            ("up_west", 0x80),
            ("up_east", 0x100),
            ("north_south", 0x200),
            ("north_west", 0x400),
            ("north_east", 0x800),
            ("south_west", 0x1000),
            ("south_east", 0x2000),
            ("west_east", 0x4000),
        ];
        for (field_name, field_mask) in fields {
            if self.0 & field_mask != 0 {
                debug_set.entry(&field_name);
            }
        }
        debug_set.finish()
    }
}

#[derive(Debug)]
pub struct VertexListBuffer<T: bytemuck::Pod> {
    buffer: Buffer,
    num_items: u32,
    phantom: PhantomData<T>,
}

impl<T: bytemuck::Pod> VertexListBuffer<T> {
    /// Panics if `items.len() > u32::MAX`.
    pub fn new(device: &Device, items: &[T]) -> Self {
        Self {
            buffer: device.create_buffer_init(&BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(items),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            num_items: items.len().try_into().unwrap(),
            phantom: PhantomData,
        }
    }

    pub fn get_slice(&self) -> BufferSlice {
        self.buffer.slice(..)
    }

    pub fn num_items(&self) -> u32 {
        self.num_items
    }

    pub fn size(&self) -> u64 {
        self.buffer.size()
    }
}

#[derive(Debug)]
pub struct IndexListBuffer<T: bytemuck::Pod> {
    buffer: Buffer,
    num_items: u32,
    phantom: PhantomData<T>,
}

impl<T: bytemuck::Pod> IndexListBuffer<T> {
    /// Panics if `items.len() > u32::MAX`.
    pub fn new(device: &Device, items: &[T]) -> Self {
        Self {
            buffer: device.create_buffer_init(&BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(items),
                usage: wgpu::BufferUsages::INDEX,
            }),
            num_items: items.len().try_into().unwrap(),
            phantom: PhantomData,
        }
    }

    pub fn get_slice(&self) -> BufferSlice {
        self.buffer.slice(..)
    }

    pub fn num_items(&self) -> u32 {
        self.num_items
    }

    pub fn size(&self) -> u64 {
        self.buffer.size()
    }
}

#[derive(Debug)]
pub struct DrawArgsBuffer<T: bytemuck::Pod> {
    buffer: Buffer,
    num_items: u32,
    phantom: PhantomData<T>,
}

impl<T: bytemuck::Pod> DrawArgsBuffer<T> {
    /// Panics if `items.len() > u32::MAX`.
    pub fn new(device: &Device, items: &[T]) -> Self {
        Self {
            buffer: device.create_buffer_init(&BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(items),
                usage: wgpu::BufferUsages::INDIRECT,
            }),
            num_items: items.len().try_into().unwrap(),
            phantom: PhantomData,
        }
    }

    pub fn get_buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn num_items(&self) -> u32 {
        self.num_items
    }

    pub fn size(&self) -> u64 {
        self.buffer.size()
    }
}

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
                compilation_options: Default::default(),
                buffers: &[Vertex::desc(), Instance::desc()],
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
                topology: wgpu::PrimitiveTopology::TriangleStrip,
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
                    -std::f32::consts::FRAC_PI_2,
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
                    std::f32::consts::FRAC_PI_2,
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

    pub type InstanceList = VertexListBuffer<Instance>;

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
            // uvs
            11 => Uint16x4,
            // matrix_indices
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

    //pub const INDICES: &[[u16; 3]] = &[
    //    // Front
    //    [0, 1, 2],
    //    [2, 1, 3],
    //    // // Back
    //    // [4, 5, 6],
    //    // [6, 5, 7],
    //    // // Left
    //    // [8, 9, 10],
    //    // [10, 9, 11],
    //    // // Right
    //    // [12, 13, 14],
    //    // [14, 13, 15],
    //    // // Top
    //    // [16, 17, 18],
    //    // [18, 17, 19],
    //    // // Bottom
    //    // [20, 21, 22],
    //    // [22, 21, 23],
    //];
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
                compilation_options: Default::default(),
                buffers: &[Vertex::desc(), Instance::desc()],
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
                topology: wgpu::PrimitiveTopology::TriangleStrip,
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
        })
    }

    pub type InstanceList = VertexListBuffer<Instance>;

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

pub mod custom_block {
    use super::*;

    pub fn create_render_pipeline(
        device: &Device,
        config: &SurfaceConfiguration,
        layout: &PipelineLayout,
    ) -> RenderPipeline {
        let shader = device.create_shader_module(include_wgsl!("shaders/custom_block.wgsl"));
        device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Custom Block Render Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                compilation_options: Default::default(),
                buffers: &[Vertex::desc(), Instance::desc()],
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
        })
    }

    pub type VertexList = VertexListBuffer<Vertex>;
    pub type IndexList = IndexListBuffer<u32>;

    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct Vertex {
        pub pos: [f32; 3],
        pub uvs: [u16; 2],
        pub normal: [f32; 3],
        pub tint_percentage: f32,
    }

    impl Vertex {
        const ATTRIBUTES: &'static [VertexAttribute] = &vertex_attr_array![
            // pos
            0 => Float32x3,
            // uvs
            1 => Uint16x2,
            // normal
            2 => Float32x3,
            // tint_percentage
            3 => Float32,
        ];

        pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: Self::ATTRIBUTES,
            }
        }
    }

    pub type InstanceList = VertexListBuffer<Instance>;

    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct Instance {
        pub pos: [f32; 3],
        /// In order of: X rotation matrix, Y rotation matrix, unused, unused
        pub matrix_indices: [u8; 4],
        pub tint_color: [u8; 4],
    }

    impl Instance {
        const ATTRIBUTES: &'static [VertexAttribute] = &vertex_attr_array![
            // pos
            10 => Float32x3,
            // matrix_indices
            11 => Uint8x4,
            // tint_color
            12 => Unorm8x4,
        ];

        pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: Self::ATTRIBUTES,
            }
        }
    }

    pub type DrawArgsList = DrawArgsBuffer<DrawIndexedIndirectArgs>;

    #[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    #[repr(C)]
    pub struct DrawIndexedIndirectArgs {
        pub num_indices: u32,
        pub num_instances: u32,
        pub start_index: u32,
        pub start_vertex: u32,
        pub start_instance: u32,
    }
}
