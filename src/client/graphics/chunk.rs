use super::Texture;
use crate::basic_types::AxisDirection;
use crate::resource::block::RightAngleRotation;
use nalgebra::{Matrix3, Rotation3};
use std::marker::PhantomData;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
    include_wgsl, vertex_attr_array, Buffer, BufferSlice, Device, PipelineLayout, RenderPipeline,
    RenderPipelineDescriptor, SurfaceConfiguration, VertexAttribute,
};

pub struct Subchunk {
    pub start_coords: [i32; 3],
    /// Equal to `u32::MAX` if the direction group contains no instances.
    pub block_face_start_vertices: [u32; 6],
    pub block_face_instance_groups: [(u32, u32); 6],
    /// Equal to `u32::MAX` if the direction group contains no instances.
    pub tinted_block_face_start_vertices: [u32; 6],
    pub tinted_block_face_instance_groups: [(u32, u32); 6],
    pub custom_block_groups: Vec<CustomBlockGroup>,
    pub connected_faces: SubchunkConnectivity,
}

pub struct CustomBlockGroup {
    pub start_vertex: u32,
    pub start_index_and_len: (u32, u32),
    pub start_instance_and_len: (u32, u32),
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

#[derive(Clone, Copy, Debug)]
struct BufferArea {
    pub usage: BufferAreaUsage,
    pub num_chunks: u64,
}

impl BufferArea {
    pub fn belongs_to(&self, subchunk_coords: [i32; 3]) -> bool {
        matches!(self.usage, BufferAreaUsage::Used(coords) if coords == subchunk_coords)
    }

    pub fn is_free(&self) -> bool {
        matches!(self.usage, BufferAreaUsage::Free)
    }
}

#[derive(Clone, Copy, Debug)]
enum BufferAreaUsage {
    Free,
    Used([i32; 3]),
}

// TODO:
// - Convert generics to `CHUNK_SIZE` and `INITIAL_NUM_CHUNKS`
// - When the buffer fills up, allocate a new buffer increased by `INITIAL_NUM_CHUNKS`
// - Copy all old buffer contents over to new buffer
// - Expand `usage_map` with the new free space
// - Retry allocation
#[derive(Debug)]
pub struct BufferManager<T: bytemuck::Pod, const BUFFER_SIZE: usize, const CHUNK_SIZE: usize> {
    buffer: Buffer,
    usage_map: Vec<BufferArea>,
    phantom: PhantomData<([T; BUFFER_SIZE], [T; CHUNK_SIZE])>,
}

impl<T: bytemuck::Pod, const BUFFER_SIZE: usize, const CHUNK_SIZE: usize>
    BufferManager<T, BUFFER_SIZE, CHUNK_SIZE>
{
    // Assert buffer contains a whole number of chunks
    const _ASSERT1: () = assert!(BUFFER_SIZE % CHUNK_SIZE == 0);
    // Assert vertices fit nicely into chunks
    const _ASSERT2: () = assert!(CHUNK_SIZE % std::mem::size_of::<T>() == 0);

    pub fn new(device: &wgpu::Device, usages: wgpu::BufferUsages) -> Self {
        let num_chunks = (BUFFER_SIZE / CHUNK_SIZE) as u64;
        Self {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: BUFFER_SIZE as u64,
                usage: usages | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            usage_map: vec![BufferArea {
                usage: BufferAreaUsage::Free,
                num_chunks,
            }],
            phantom: PhantomData,
        }
    }

    pub fn alloc_area(
        &mut self,
        queue: &wgpu::Queue,
        subchunk_coords: [i32; 3],
        items: &[T],
    ) -> u32 {
        debug_assert!(!items.is_empty());
        let items_byte_slice: &[u8] = bytemuck::cast_slice(items);
        let byte_len = items_byte_slice.len();
        let num_chunks_needed = byte_len.div_ceil(CHUNK_SIZE) as u64;
        // Find free area large enough to hold items
        let mut current_start_chunk: u64 = 0;
        for i in 0..self.usage_map.len() {
            let area = &mut self.usage_map[i];
            if area.is_free() {
                use std::cmp::Ordering;
                match area.num_chunks.cmp(&num_chunks_needed) {
                    Ordering::Greater => {
                        // Split area into used portion and leftover free protion
                        let new_free_area = BufferArea {
                            usage: BufferAreaUsage::Free,
                            num_chunks: area.num_chunks - num_chunks_needed,
                        };
                        area.num_chunks = num_chunks_needed;
                        area.usage = BufferAreaUsage::Used(subchunk_coords);
                        self.usage_map.insert(i + 1, new_free_area);
                    }
                    Ordering::Equal => {
                        // Mark entire area as used
                        area.usage = BufferAreaUsage::Used(subchunk_coords);
                    }
                    Ordering::Less => {
                        current_start_chunk += area.num_chunks;
                        continue;
                    }
                }
                // Write items to buffer
                let buffer_offset = current_start_chunk * CHUNK_SIZE as u64;
                let mut buffer_window = queue
                    .write_buffer_with(
                        &self.buffer,
                        buffer_offset,
                        (byte_len as u64).try_into().unwrap(),
                    )
                    .unwrap();
                buffer_window.copy_from_slice(items_byte_slice);
                return (buffer_offset / std::mem::size_of::<T>() as u64)
                    .try_into()
                    .unwrap();
            } else {
                current_start_chunk += area.num_chunks;
            }
        }
        unimplemented!("Buffer pool growing");
    }

    pub fn free_subchunk_areas(&mut self, subchunk_coords: [i32; 3]) {
        // Mark all subchunk owned areas as free
        for area in &mut self.usage_map {
            if area.belongs_to(subchunk_coords) {
                area.usage = BufferAreaUsage::Free;
            }
        }
        // Merge free areas
        let mut current_area_i: usize = 1;
        while current_area_i < self.usage_map.len() {
            if self.usage_map[current_area_i - 1].is_free()
                && self.usage_map[current_area_i].is_free()
            {
                self.usage_map[current_area_i - 1].num_chunks +=
                    self.usage_map[current_area_i].num_chunks;
                self.usage_map.remove(current_area_i);
            } else {
                current_area_i += 1;
            }
        }
    }

    pub fn get_slice(&self) -> BufferSlice {
        self.buffer.slice(..)
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct VertexBufferManager<V: bytemuck::Pod, const BUFFER_SIZE: usize, const CHUNK_SIZE: usize>(
    BufferManager<V, BUFFER_SIZE, CHUNK_SIZE>,
);

impl<V: bytemuck::Pod, const BUFFER_SIZE: usize, const CHUNK_SIZE: usize>
    VertexBufferManager<V, BUFFER_SIZE, CHUNK_SIZE>
{
    pub fn new(device: &wgpu::Device) -> Self {
        Self(BufferManager::new(device, wgpu::BufferUsages::VERTEX))
    }

    /// Returns the `first_vertex` indirect draw argument.
    pub fn alloc_area(
        &mut self,
        queue: &wgpu::Queue,
        subchunk_coords: [i32; 3],
        base_quad: [V; 4],
    ) -> u32 {
        self.0.alloc_area(queue, subchunk_coords, &base_quad)
    }

    pub fn free_subchunk_areas(&mut self, subchunk_coords: [i32; 3]) {
        self.0.free_subchunk_areas(subchunk_coords)
    }

    pub fn get_slice(&self) -> wgpu::BufferSlice {
        self.0.get_slice()
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct InstanceBufferManager<
    I: bytemuck::Pod,
    const BUFFER_SIZE: usize,
    const CHUNK_SIZE: usize,
>(BufferManager<I, BUFFER_SIZE, CHUNK_SIZE>);

impl<I: bytemuck::Pod, const BUFFER_SIZE: usize, const CHUNK_SIZE: usize>
    InstanceBufferManager<I, BUFFER_SIZE, CHUNK_SIZE>
{
    pub fn new(device: &wgpu::Device) -> Self {
        Self(BufferManager::new(device, wgpu::BufferUsages::VERTEX))
    }

    /// Returns the `first_instance` indirect draw argument.
    pub fn alloc_area(
        &mut self,
        queue: &wgpu::Queue,
        subchunk_coords: [i32; 3],
        instances: &[I],
    ) -> u32 {
        self.0.alloc_area(queue, subchunk_coords, instances)
    }

    pub fn free_subchunk_areas(&mut self, subchunk_coords: [i32; 3]) {
        self.0.free_subchunk_areas(subchunk_coords)
    }

    pub fn get_slice(&self) -> wgpu::BufferSlice {
        self.0.get_slice()
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
            cache: None,
        })
    }

    pub mod face_matrices {
        use super::*;

        #[inline]
        pub fn rotations() -> [Rotation3<f32>; 6] {
            [
                // Top
                Rotation3::identity(),
                // Bottom
                Rotation3::from_euler_angles(std::f32::consts::PI, 0.0, 0.0),
                // North
                Rotation3::from_euler_angles(
                    -std::f32::consts::FRAC_PI_2,
                    0.0,
                    std::f32::consts::PI,
                ),
                // South
                Rotation3::from_euler_angles(std::f32::consts::FRAC_PI_2, 0.0, 0.0),
                // East
                Rotation3::from_euler_angles(
                    0.0,
                    std::f32::consts::FRAC_PI_2,
                    -std::f32::consts::FRAC_PI_2,
                ),
                // West
                Rotation3::from_euler_angles(
                    0.0,
                    -std::f32::consts::FRAC_PI_2,
                    std::f32::consts::FRAC_PI_2,
                ),
            ]
        }

        pub fn generate_array() -> [[[f32; 4]; 3]; 6] {
            // Alignment of each row in a mat3x3 is same as vec4, so we pad up to size
            rotations()
                .map(Matrix3::from)
                .map(|matrix| matrix.into())
                .map(|matrix: [[f32; 3]; 3]| matrix.map(|[x, y, z]| [x, y, z, 0.0]))
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

    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct Vertex {
        subchunk_start_coords: [f32; 3],
        face_matrix_index: u32,
    }

    impl Vertex {
        const ATTRIBUTES: &'static [VertexAttribute] = &vertex_attr_array![
            // subchunk_start_coords
            0 => Float32x3,
            // face_matrix_index
            1 => Uint32,
        ];

        pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: Self::ATTRIBUTES,
            }
        }

        pub fn generate_base_quad(
            subchunk_start_coords: [i32; 3],
            face_matrix_index: usize,
        ) -> [Self; 4] {
            let subchunk_start_coords = subchunk_start_coords.map(|n| n as f32);
            let face_matrix_index = face_matrix_index as u32;
            [Self {
                subchunk_start_coords,
                face_matrix_index,
            }; 4]
        }
    }

    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct Instance {
        uvs: [u16; 4],
        /// 0-3: X offset
        /// 4-7: Y offset
        /// 8-11: Z offset
        /// 12-15: Unused
        packed_xyz: u16,
        /// 8-11: Sky light level
        /// 12-15: Block light level
        uv_rotation_and_light_levels: [u8; 2],
    }

    impl Instance {
        const ATTRIBUTES: &'static [VertexAttribute] = &vertex_attr_array![
            // uvs
            10 => Uint16x4,
            // packed_xyz
            11 => Uint8x2,
            // uv_rotation_and_light_levels
            12 => Uint8x2,
        ];

        pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: Self::ATTRIBUTES,
            }
        }

        pub fn new(
            subchunk_xyz: [u8; 3],
            uvs: [u16; 4],
            uv_rotation: RightAngleRotation,
            light_levels: [u8; 2],
        ) -> Self {
            debug_assert!(subchunk_xyz[0] < 16);
            debug_assert!(subchunk_xyz[1] < 16);
            debug_assert!(subchunk_xyz[2] < 16);
            debug_assert!(light_levels[0] < 16);
            debug_assert!(light_levels[1] < 16);
            Self {
                uvs,
                packed_xyz: (subchunk_xyz[0] as u16)
                    | ((subchunk_xyz[1] as u16) << 4)
                    | ((subchunk_xyz[2] as u16) << 8),
                uv_rotation_and_light_levels: [
                    match uv_rotation {
                        RightAngleRotation::Zero => 0,
                        RightAngleRotation::Ninety => 1,
                        RightAngleRotation::OneEighty => 2,
                        RightAngleRotation::TwoSeventy => 3,
                    },
                    light_levels[0] | (light_levels[1] << 4),
                ],
            }
        }
    }

    pub type BlockFaceVertexBufferManager = VertexBufferManager<
        Vertex,
        { std::mem::size_of::<[[Vertex; 4]; 1 << 20]>() },
        { std::mem::size_of::<[Vertex; 4]>() },
    >;

    pub type BlockFaceInstanceBufferManager = InstanceBufferManager<
        Instance,
        { std::mem::size_of::<[[Instance; 4]; 1 << 20]>() },
        { std::mem::size_of::<[Instance; 4]>() },
    >;
}

pub mod tinted_block_face {
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
            cache: None,
        })
    }

    pub use super::block_face::Vertex;

    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct Instance {
        uvs: [u16; 4],
        tint_color: [u8; 4],
        /// 0-3: X offset
        /// 4-7: Y offset
        /// 8-11: Z offset
        /// 12-15: Unused
        packed_xyz: u16,
        /// 0-3: UV rotation
        /// 4-7: Unused
        /// 8-11: Sky light level
        /// 12-15: Block light level
        uv_rotation_and_light_levels: [u8; 2],
    }

    impl Instance {
        const ATTRIBUTES: &'static [VertexAttribute] = &vertex_attr_array![
            // uvs
            10 => Uint16x4,
            // tint_color
            11 => Unorm8x4,
            // packed_xyz
            12 => Uint8x2,
            // uv_rotation_and_light_levels
            13 => Uint8x2,
        ];

        pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: Self::ATTRIBUTES,
            }
        }

        pub fn new(
            subchunk_xyz: [u8; 3],
            uvs: [u16; 4],
            uv_rotation: RightAngleRotation,
            light_levels: [u8; 2],
            tint_color: [u8; 4],
        ) -> Self {
            debug_assert!(subchunk_xyz[0] < 16);
            debug_assert!(subchunk_xyz[1] < 16);
            debug_assert!(subchunk_xyz[2] < 16);
            debug_assert!(light_levels[0] < 16);
            debug_assert!(light_levels[1] < 16);
            Self {
                uvs,
                tint_color,
                packed_xyz: (subchunk_xyz[0] as u16)
                    | ((subchunk_xyz[1] as u16) << 4)
                    | ((subchunk_xyz[2] as u16) << 8),
                uv_rotation_and_light_levels: [
                    match uv_rotation {
                        RightAngleRotation::Zero => 0,
                        RightAngleRotation::Ninety => 1,
                        RightAngleRotation::OneEighty => 2,
                        RightAngleRotation::TwoSeventy => 3,
                    },
                    light_levels[0] | (light_levels[1] << 4),
                ],
            }
        }
    }

    pub type TintedBlockFaceVertexBufferManager = VertexBufferManager<
        Vertex,
        { std::mem::size_of::<[[Vertex; 4]; 1 << 20]>() },
        { std::mem::size_of::<[Vertex; 4]>() },
    >;

    pub type TintedBlockFaceInstanceBufferManager = InstanceBufferManager<
        Instance,
        { std::mem::size_of::<[[Instance; 4]; 1 << 20]>() },
        { std::mem::size_of::<[Instance; 4]>() },
    >;
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
            cache: None,
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
        pos: [f32; 3],
        tint_color: [u8; 4],
        /// Light levels for surrounding blocks in order:
        /// 1: Centre
        /// 2: Above
        /// 3: Below
        /// 4: North
        /// 5: South
        /// 6: East
        /// 7: West
        /// 8: Unused
        light_level_pairs: [u8; 8],
    }

    impl Instance {
        const ATTRIBUTES: &'static [VertexAttribute] = &vertex_attr_array![
            // pos
            10 => Float32x3,
            // tint_color
            11 => Unorm8x4,
            // light_level_pairs (first half)
            12 => Uint8x4,
            // light_level_pairs (second half)
            13 => Uint8x4,
        ];

        pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: Self::ATTRIBUTES,
            }
        }

        pub fn new(
            pos: [f32; 3],
            tint_color: [u8; 4],
            centre_light_levels: [u8; 2],
            neighbour_light_levels: [[u8; 2]; 6],
        ) -> Self {
            debug_assert!(centre_light_levels[0] < 16);
            debug_assert!(centre_light_levels[1] < 16);
            for pair in neighbour_light_levels {
                debug_assert!(pair[0] < 16);
                debug_assert!(pair[1] < 16);
            }
            let mut converted_light_level_pairs = [0u8; 8];
            converted_light_level_pairs[0] = centre_light_levels[0] | (centre_light_levels[1] << 4);
            for (i, pair) in neighbour_light_levels.into_iter().enumerate() {
                converted_light_level_pairs[i + 1] = pair[0] | (pair[1] << 4);
            }
            Self {
                pos,
                tint_color,
                light_level_pairs: converted_light_level_pairs,
            }
        }
    }

    pub type CustomBlockInstanceBufferManager = InstanceBufferManager<
        Instance,
        { std::mem::size_of::<[[Instance; 4]; 1 << 20]>() },
        { std::mem::size_of::<[Instance; 4]>() },
    >;
}
