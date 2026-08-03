use super::{SubchunkDataStorage, Texture, include_wesl_module};
use crate::graphics::chunk::{HasSubchunkData, SubchunkConnectivity, SubchunkData};
use crate::{MIN_HEIGHT_I32, SUBCHUNK_AXIS_LEN_I32};
use core::marker::PhantomData;
use nalgebra::{Matrix3, Rotation3};
use portable_std::{Arc, FastHashMap};
use resources::block::RightAngleRotation;
use resources::block::blockstate::BlockOpacity;
use resources::block::model::{ModelIndex, ModelType};
use resources::block::model::{ModelRegistry, Tint};
use std::sync::mpsc::Sender;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
    Buffer, BufferSlice, Device, PipelineLayout, RenderPipeline, RenderPipelineDescriptor,
    SurfaceConfiguration, VertexAttribute,
};

pub struct Subchunk {
    pub dispatch_id: u64,
    pub start_coords: [i32; 3],
    /// Equal to `u32::MAX` if the direction group contains no instances.
    pub block_face_start_vertices: [u32; 6],
    /// Equal to `(0, 0)` if the direction group contains no instances.
    pub block_face_instance_groups: [(u32, u32); 6],
    /// Equal to `u32::MAX` if the direction group contains no instances.
    pub tinted_block_face_start_vertices: [u32; 6],
    /// Equal to `(0, 0)` if the direction group contains no instances.
    pub tinted_block_face_instance_groups: [(u32, u32); 6],
    pub custom_block_groups: Vec<CustomBlockGroup>,
    pub connectivity: SubchunkConnectivity,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CustomBlockGroup {
    pub start_face_and_len: [u32; 2],
    pub start_instance_and_len: [u32; 2],
}

impl HasSubchunkData for Subchunk {
    fn get_data(&self) -> SubchunkData {
        SubchunkData {
            start_coords: self.start_coords,
            connectivity: self.connectivity,
        }
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

    pub fn get_slice(&self) -> BufferSlice<'_> {
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

    pub fn get_slice(&self) -> BufferSlice<'_> {
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
    // Assert buffer contains a whole number of chunks.
    const _ASSERT1: () = assert!(BUFFER_SIZE.is_multiple_of(CHUNK_SIZE));
    // Assert vertices fit nicely into chunks.
    const _ASSERT2: () = assert!(CHUNK_SIZE.is_multiple_of(core::mem::size_of::<T>()));

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
                use core::cmp::Ordering;
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
                return (buffer_offset / core::mem::size_of::<T>() as u64)
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

    pub fn get_slice(&self) -> BufferSlice<'_> {
        self.buffer.slice(..)
    }

    pub fn get_entire_binding(&self) -> wgpu::BindingResource<'_> {
        self.buffer.as_entire_binding()
    }

    pub fn size(&self) -> wgpu::BufferAddress {
        self.buffer.size()
    }

    pub fn free_bytes(&self) -> u64 {
        self.usage_map
            .iter()
            .map(|area| {
                if area.is_free() {
                    area.num_chunks * CHUNK_SIZE as u64
                } else {
                    0
                }
            })
            .sum()
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

    pub fn get_slice(&self) -> wgpu::BufferSlice<'_> {
        self.0.get_slice()
    }

    pub fn size(&self) -> wgpu::BufferAddress {
        self.0.size()
    }

    pub fn free_bytes(&self) -> wgpu::BufferAddress {
        self.0.free_bytes()
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
        instances: impl AsRef<[I]>,
    ) -> u32 {
        self.0
            .alloc_area(queue, subchunk_coords, instances.as_ref())
    }

    pub fn free_subchunk_areas(&mut self, subchunk_coords: [i32; 3]) {
        self.0.free_subchunk_areas(subchunk_coords)
    }

    pub fn get_slice(&self) -> wgpu::BufferSlice<'_> {
        self.0.get_slice()
    }

    pub fn get_entire_binding(&self) -> wgpu::BindingResource<'_> {
        self.0.get_entire_binding()
    }

    pub fn size(&self) -> wgpu::BufferAddress {
        self.0.size()
    }

    pub fn free_bytes(&self) -> wgpu::BufferAddress {
        self.0.free_bytes()
    }
}

pub mod block_face {
    use super::*;

    pub fn create_render_pipeline(
        device: &Device,
        config: &SurfaceConfiguration,
        layout: &PipelineLayout,
    ) -> RenderPipeline {
        let shader = device.create_shader_module(include_wesl_module!("block_face"));
        device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Block Face Render Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: None,
                compilation_options: Default::default(),
                buffers: &[Some(Vertex::desc()), Some(Instance::desc())],
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
                depth_write_enabled: Some(true),
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

    pub mod face_matrices {
        use super::*;

        #[inline]
        pub fn rotations() -> [Rotation3<f32>; 6] {
            [
                // Top
                Rotation3::identity(),
                // Bottom
                Rotation3::from_euler_angles(core::f32::consts::PI, 0.0, 0.0),
                // North
                Rotation3::from_euler_angles(
                    -core::f32::consts::FRAC_PI_2,
                    0.0,
                    core::f32::consts::PI,
                ),
                // South
                Rotation3::from_euler_angles(core::f32::consts::FRAC_PI_2, 0.0, 0.0),
                // East
                Rotation3::from_euler_angles(
                    0.0,
                    core::f32::consts::FRAC_PI_2,
                    -core::f32::consts::FRAC_PI_2,
                ),
                // West
                Rotation3::from_euler_angles(
                    0.0,
                    -core::f32::consts::FRAC_PI_2,
                    core::f32::consts::FRAC_PI_2,
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
        const ATTRIBUTES: &'static [VertexAttribute] = &wgpu::vertex_attr_array![
            // subchunk_start_coords
            0 => Float32x3,
            // face_matrix_index
            1 => Uint32,
        ];

        pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
            wgpu::VertexBufferLayout {
                array_stride: core::mem::size_of::<Self>() as wgpu::BufferAddress,
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
        /// 0-3: UV rotation
        /// 4-7: Unused
        /// 8-11: Sky light level
        /// 12-15: Block light level
        uv_rotation_and_light_levels: [u8; 2],
    }

    impl Instance {
        const ATTRIBUTES: &'static [VertexAttribute] = &wgpu::vertex_attr_array![
            // uvs
            10 => Uint16x4,
            // packed_xyz
            11 => Uint8x2,
            // uv_rotation_and_light_levels
            12 => Uint8x2,
        ];

        pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
            wgpu::VertexBufferLayout {
                array_stride: core::mem::size_of::<Self>() as wgpu::BufferAddress,
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
        { core::mem::size_of::<[[Vertex; 4]; 1 << 20]>() },
        { core::mem::size_of::<[Vertex; 4]>() },
    >;

    pub type BlockFaceInstanceBufferManager = InstanceBufferManager<
        Instance,
        { core::mem::size_of::<[[Instance; 4]; 1 << 20]>() },
        { core::mem::size_of::<[Instance; 4]>() },
    >;
}

pub mod tinted_block_face {
    use super::*;

    pub fn create_render_pipeline(
        device: &Device,
        config: &SurfaceConfiguration,
        layout: &PipelineLayout,
    ) -> RenderPipeline {
        let shader = device.create_shader_module(include_wesl_module!("tinted_block_face"));
        device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Tinted Block Face Render Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: None,
                compilation_options: Default::default(),
                buffers: &[Some(Vertex::desc()), Some(Instance::desc())],
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
                depth_write_enabled: Some(true),
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
        const ATTRIBUTES: &'static [VertexAttribute] = &wgpu::vertex_attr_array![
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
                array_stride: core::mem::size_of::<Self>() as wgpu::BufferAddress,
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
        { core::mem::size_of::<[[Vertex; 4]; 1 << 20]>() },
        { core::mem::size_of::<[Vertex; 4]>() },
    >;

    pub type TintedBlockFaceInstanceBufferManager = InstanceBufferManager<
        Instance,
        { core::mem::size_of::<[[Instance; 4]; 1 << 20]>() },
        { core::mem::size_of::<[Instance; 4]>() },
    >;
}

pub mod custom_block {
    use super::*;

    pub fn create_render_pipeline(
        device: &Device,
        config: &SurfaceConfiguration,
        layout: &PipelineLayout,
    ) -> RenderPipeline {
        let shader = device.create_shader_module(include_wesl_module!("custom_block"));
        device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Custom Block Render Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: None,
                compilation_options: Default::default(),
                buffers: &[Some(Instance::desc())],
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
                depth_write_enabled: Some(true),
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
        const ATTRIBUTES: &'static [VertexAttribute] = &wgpu::vertex_attr_array![
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
                array_stride: core::mem::size_of::<Self>() as wgpu::BufferAddress,
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
        { core::mem::size_of::<[[Instance; 4]; 1 << 20]>() },
        { core::mem::size_of::<[Instance; 4]>() },
    >;
}

#[derive(Debug)]
pub struct RawSubchunk {
    pub dispatch_id: u64,
    pub subchunk_coords: [i32; 3],
    pub start_coords: [i32; 3],
    pub block_face_quads: [Option<[block_face::Vertex; 4]>; 6],
    pub block_face_instance_groups: [Vec<block_face::Instance>; 6],
    pub tinted_block_face_quads: [Option<[tinted_block_face::Vertex; 4]>; 6],
    pub tinted_block_face_instance_groups: [Vec<tinted_block_face::Instance>; 6],
    pub custom_block_groups: Vec<RawCustomBlockGroup>,
    pub connectivity: SubchunkConnectivity,
}

#[derive(Debug)]
pub struct RawCustomBlockGroup {
    pub start_face_and_len: [u32; 2],
    pub instances: Vec<custom_block::Instance>,
}

#[tracing::instrument(skip_all)]
pub fn process_subchunk(
    block_registry: &resources::block::Registry,
    model_registry: &ModelRegistry,
    raw_chunks: &FastHashMap<[i32; 2], Arc<crate::RawChunk>>,
    pending_subchunk_tx: &Sender<Option<RawSubchunk>>,
    subchunk_coords: [i32; 3],
    dispatch_id: u64,
) {
    let [subchunk_x, subchunk_y, subchunk_z] = subchunk_coords;
    let mut block_faces: [Vec<_>; 6] = Default::default();
    let mut tinted_block_faces: [Vec<_>; 6] = Default::default();
    let mut custom_block_instance_groups = FastHashMap::new();
    let Some(connectivity) = crate::graphics::chunk::process_subchunk_models(
        block_registry,
        model_registry,
        raw_chunks,
        subchunk_coords,
        |model_processing_args| {
            let crate::graphics::chunk::ModelProcessingArgs {
                model_registry,
                chunk,
                block_opacity,
                face_cull_map,
                face_light_map,
                tint_color,
                subchunk_xyz,
                global_xyz,
                xyz,
                model_idx,
            } = model_processing_args;
            process_subchunk_model(
                &mut block_faces,
                &mut tinted_block_faces,
                &mut custom_block_instance_groups,
                model_registry,
                chunk,
                block_opacity,
                face_cull_map,
                face_light_map,
                tint_color,
                subchunk_xyz,
                global_xyz,
                xyz,
                model_idx,
            );
        },
    ) else {
        // Skip subchunk if `process_subchunk_models` returns that it's invisible.
        return;
    };
    let start_coords = [
        SUBCHUNK_AXIS_LEN_I32 * subchunk_x,
        SUBCHUNK_AXIS_LEN_I32 * subchunk_y + MIN_HEIGHT_I32,
        SUBCHUNK_AXIS_LEN_I32 * subchunk_z,
    ];
    // Block faces
    let mut block_face_quads: [Option<_>; 6] = [None; 6];
    let block_face_instance_groups: [Vec<_>; 6] = block_faces;
    for i in 0..6 {
        if block_face_instance_groups[i].is_empty() {
            continue;
        }
        let base_quad = block_face::Vertex::generate_base_quad(start_coords, i);
        block_face_quads[i] = Some(base_quad);
    }
    // Tinted block faces
    let mut tinted_block_face_quads: [Option<_>; 6] = [None; 6];
    let tinted_block_face_instance_groups: [Vec<_>; 6] = tinted_block_faces;
    for i in 0..6 {
        if tinted_block_face_instance_groups[i].is_empty() {
            continue;
        }
        let base_quad = tinted_block_face::Vertex::generate_base_quad(start_coords, i);
        tinted_block_face_quads[i] = Some(base_quad);
    }
    // Custom blocks
    let custom_block_groups = custom_block_instance_groups
        .into_iter()
        .map(|(info, instances)| RawCustomBlockGroup {
            start_face_and_len: info.start_face_and_len,
            instances,
        })
        .collect();
    pending_subchunk_tx
        .send(Some(RawSubchunk {
            dispatch_id,
            subchunk_coords,
            start_coords,
            block_face_quads,
            block_face_instance_groups,
            tinted_block_face_quads,
            tinted_block_face_instance_groups,
            custom_block_groups,
            connectivity,
        }))
        .unwrap();
}

#[tracing::instrument(skip_all)]
pub fn finalise_subchunk(
    subchunk_data_storage: &mut SubchunkDataStorage,
    queue: &wgpu::Queue,
    raw_subchunk: RawSubchunk,
) {
    let subchunk_coords = raw_subchunk.subchunk_coords;
    // Remove old subchunk.
    {
        let span = tracing::trace_span!("remove_old_subchunk", ?subchunk_coords);
        let _enter = span.enter();
        subchunk_data_storage.remove_subchunk(subchunk_coords);
    }
    macro_rules! alloc_area {
        (
            $buffer_manager:ident,
            $subchunk_coords:expr,
            $data:expr $(,)?
        ) => {
            subchunk_data_storage
                .$buffer_manager
                .alloc_area(queue, $subchunk_coords, $data)
        };
    }
    // Base block faces
    let mut block_face_start_vertices: [u32; 6] = [u32::MAX; 6];
    let mut block_face_instance_groups: [(u32, u32); 6] = Default::default();
    for (i, instance_group) in raw_subchunk
        .block_face_instance_groups
        .into_iter()
        .enumerate()
    {
        let Some(base_quad) = raw_subchunk.block_face_quads[i] else {
            continue;
        };
        let quad_start_vertex = alloc_area!(block_face_vertex, subchunk_coords, base_quad);
        let instance_group_len: u32 = instance_group.len().try_into().unwrap();
        let instance_group_start = alloc_area!(
            block_face_instance,
            subchunk_coords,
            instance_group.into_boxed_slice(),
        );
        block_face_start_vertices[i] = quad_start_vertex;
        block_face_instance_groups[i] = (instance_group_start, instance_group_len);
    }
    // Tinted block faces
    let mut tinted_block_face_start_vertices: [u32; 6] = [u32::MAX; 6];
    let mut tinted_block_face_instance_groups: [(u32, u32); 6] = Default::default();
    for (i, instance_group) in raw_subchunk
        .tinted_block_face_instance_groups
        .into_iter()
        .enumerate()
    {
        let Some(base_quad) = raw_subchunk.tinted_block_face_quads[i] else {
            continue;
        };
        let quad_start_vertex = alloc_area!(tinted_block_face_vertex, subchunk_coords, base_quad);
        let instance_group_len: u32 = instance_group.len().try_into().unwrap();
        let instance_group_start = alloc_area!(
            tinted_block_face_instance,
            subchunk_coords,
            instance_group.into_boxed_slice(),
        );
        tinted_block_face_start_vertices[i] = quad_start_vertex;
        tinted_block_face_instance_groups[i] = (instance_group_start, instance_group_len);
    }
    let custom_block_groups = raw_subchunk
        .custom_block_groups
        .into_iter()
        .map(|group| {
            let num_instances: u32 = group.instances.len().try_into().unwrap();
            let start_instance = alloc_area!(
                custom_block_instance,
                subchunk_coords,
                group.instances.into_boxed_slice(),
            );
            CustomBlockGroup {
                start_face_and_len: group.start_face_and_len,
                start_instance_and_len: [start_instance, num_instances],
            }
        })
        .collect();
    subchunk_data_storage.subchunks.insert(
        subchunk_coords,
        Subchunk {
            dispatch_id: raw_subchunk.dispatch_id,
            start_coords: raw_subchunk.start_coords,
            block_face_start_vertices,
            block_face_instance_groups,
            tinted_block_face_start_vertices,
            tinted_block_face_instance_groups,
            custom_block_groups,
            connectivity: raw_subchunk.connectivity,
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn process_subchunk_model(
    block_faces: &mut [Vec<block_face::Instance>; 6],
    tinted_block_faces: &mut [Vec<tinted_block_face::Instance>; 6],
    custom_block_instance_groups: &mut FastHashMap<
        resources::block::model::OtherInfo,
        Vec<custom_block::Instance>,
    >,
    model_registry: &ModelRegistry,
    chunk: &crate::RawChunk,
    block_opacity: BlockOpacity,
    face_cull_map: [bool; 6],
    face_light_map: [[u8; 2]; 6],
    tint_color: [u8; 4],
    [subchunk_x, subchunk_y, subchunk_z]: [i32; 3],
    [global_x, global_y, global_z]: [f32; 3],
    [x, y, z]: [usize; 3],
    model_idx: ModelIndex,
) {
    let model = &model_registry[model_idx];
    match model {
        ModelType::None => {}
        ModelType::Block(info) => {
            match block_opacity {
                BlockOpacity::Opaque => {
                    for i in 0..6 {
                        if face_cull_map[i] {
                            continue;
                        }
                        block_faces[i].push(block_face::Instance::new(
                            [x as u8, y as u8, z as u8],
                            info.per_face_atlas_uvs[i],
                            info.per_face_uv_rotations[i],
                            face_light_map[i],
                        ));
                    }
                }
                _ => {
                    for i in 0..6 {
                        if face_cull_map[i] {
                            continue;
                        }
                        tinted_block_faces[i].push(tinted_block_face::Instance::new(
                            [x as u8, y as u8, z as u8],
                            info.per_face_atlas_uvs[i],
                            info.per_face_uv_rotations[i],
                            face_light_map[i],
                            // Block doesn't have any tint, so just use opaque
                            // white as a null value.
                            [0xFF; 4],
                        ));
                    }
                }
            }
        }
        ModelType::TintedBlock(info) => {
            for i in 0..6 {
                if face_cull_map[i] {
                    continue;
                }
                tinted_block_faces[i].push(tinted_block_face::Instance::new(
                    [x as u8, y as u8, z as u8],
                    info.per_face_atlas_uvs[i],
                    #[cfg(feature = "graphics_backend_vulkan")]
                    info.per_face_uv_rotations[i],
                    face_light_map[i],
                    tint_color,
                ));
            }
        }
        ModelType::OverlayedBlock(info) => {
            for face in &info.faces {
                if face_cull_map[face.face_i as usize] {
                    continue;
                }
                if let Some(tint) = face.tint {
                    assert!(tint == Tint::Biome, "TODO: Alternative tints");
                    tinted_block_faces[face.face_i as usize].push(
                        tinted_block_face::Instance::new(
                            [x as u8, y as u8, z as u8],
                            face.atlas_uvs,
                            #[cfg(feature = "graphics_backend_vulkan")]
                            face.uv_rotation,
                            face_light_map[face.face_i as usize],
                            tint_color,
                        ),
                    );
                } else {
                    block_faces[face.face_i as usize].push(block_face::Instance::new(
                        [x as u8, y as u8, z as u8],
                        face.atlas_uvs,
                        #[cfg(feature = "graphics_backend_vulkan")]
                        face.uv_rotation,
                        face_light_map[face.face_i as usize],
                    ));
                }
            }
        }
        ModelType::Liquid(_info) => {
            // TODO:
        }
        ModelType::Other(info) => {
            // TODO:
            // - Add a `face_opacity_map`
            // - For each neighbour, if it's opaque, replace with centre block
            //   light level
            let light_section = chunk
                .lighting
                .get_section(MIN_HEIGHT_I32, subchunk_y + (MIN_HEIGHT_I32 / 16))
                .unwrap();
            let block_instances = custom_block_instance_groups.entry(*info).or_default();
            block_instances.push(custom_block::Instance::new(
                [global_x, global_y, global_z],
                tint_color,
                light_section.get(x, y, z),
                face_light_map,
            ));
        }
        ModelType::Composite(parts) => {
            for part in parts {
                process_subchunk_model(
                    block_faces,
                    tinted_block_faces,
                    custom_block_instance_groups,
                    model_registry,
                    chunk,
                    block_opacity,
                    face_cull_map,
                    face_light_map,
                    tint_color,
                    [subchunk_x, subchunk_y, subchunk_z],
                    [global_x, global_y, global_z],
                    [x, y, z],
                    part.model_idx,
                );
            }
        }
    }
}
