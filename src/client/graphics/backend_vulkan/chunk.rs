use super::shader_exports::chunk::types::{self as shader_chunk_types, vertex_input_state};
use super::shader_exports::shader_stage_from_entry_point;
use crate::basic_types::AxisDirection;
use anyhow::Context;
use nalgebra::{Matrix3, Rotation3};
use std::marker::{PhantomData, Send, Sync};
use std::sync::Arc;
use vulkan_prelude::*;

pub struct Subchunk {
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
    pub connected_faces: SubchunkConnectivity,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CustomBlockGroup {
    pub start_face_and_len: [u32; 2],
    pub start_instance_and_len: [u32; 2],
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
pub struct VertexListBuffer<T: bytemuck::Pod + Send + Sync> {
    buffer: VulkanSubbuffer<[T]>,
    num_items: u32,
}

impl<T: bytemuck::Pod + Send + Sync> VertexListBuffer<T> {
    /// Panics if `items.len() > u32::MAX`.
    pub fn new(
        memory_allocator: &Arc<dyn VulkanMemoryAllocator>,
        items: &[T],
    ) -> anyhow::Result<Self> {
        Ok(Self {
            buffer: VulkanBuffer::from_iter(
                memory_allocator,
                &VulkanBufferCreateInfo {
                    usage: VulkanBufferUsage::VERTEX_BUFFER,
                    ..Default::default()
                },
                &VulkanAllocationCreateInfo {
                    memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                        | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                items.iter().copied(),
            )?,
            num_items: items.len().try_into().unwrap(),
        })
    }

    pub fn get_buffer(&self) -> VulkanSubbuffer<[T]> {
        self.buffer.clone()
    }

    pub fn num_items(&self) -> u32 {
        self.num_items
    }

    pub fn size(&self) -> u64 {
        self.buffer.size()
    }
}

#[derive(Debug)]
pub struct IndexListBuffer<T: bytemuck::Pod + Sync + Send> {
    buffer: VulkanSubbuffer<[T]>,
    num_items: u32,
}

impl<T: bytemuck::Pod + Sync + Send> IndexListBuffer<T> {
    /// Panics if `items.len() > u32::MAX`.
    pub fn new(
        memory_allocator: &Arc<dyn VulkanMemoryAllocator>,
        items: &[T],
    ) -> anyhow::Result<Self> {
        Ok(Self {
            buffer: VulkanBuffer::from_iter(
                memory_allocator,
                &VulkanBufferCreateInfo {
                    usage: VulkanBufferUsage::INDEX_BUFFER,
                    ..Default::default()
                },
                &VulkanAllocationCreateInfo {
                    memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                        | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                items.iter().copied(),
            )?,
            num_items: items.len().try_into().unwrap(),
        })
    }

    pub fn get_buffer(&self) -> VulkanSubbuffer<[T]> {
        self.buffer.clone()
    }

    pub fn num_items(&self) -> u32 {
        self.num_items
    }

    pub fn size(&self) -> VulkanDeviceSize {
        self.buffer.size()
    }
}

#[derive(Debug)]
pub struct DrawArgsBuffer<T: bytemuck::Pod + Send + Sync> {
    buffer: VulkanSubbuffer<[T]>,
    num_items: u32,
}

impl<T: bytemuck::Pod + Send + Sync> DrawArgsBuffer<T> {
    /// Panics if `items.len() > u32::MAX`.
    pub fn new(
        memory_allocator: &Arc<dyn VulkanMemoryAllocator>,
        items: &[T],
    ) -> anyhow::Result<Self> {
        Ok(Self {
            buffer: VulkanBuffer::from_iter(
                memory_allocator,
                &VulkanBufferCreateInfo {
                    usage: VulkanBufferUsage::INDIRECT_BUFFER,
                    ..Default::default()
                },
                &VulkanAllocationCreateInfo {
                    memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                        | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                items.iter().copied(),
            )?,
            num_items: items.len().try_into().unwrap(),
        })
    }

    pub fn get_buffer(&self) -> VulkanSubbuffer<[T]> {
        self.buffer.clone()
    }

    pub fn num_items(&self) -> u32 {
        self.num_items
    }

    pub fn size(&self) -> VulkanDeviceSize {
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
// - Convert `NUM_CHUNKS` to `INITIAL_NUM_CHUNKS`
// - When the buffer fills up, allocate a new buffer increased by `INITIAL_NUM_CHUNKS` / 16
// - Copy all old buffer contents over to new buffer
// - Expand `usage_map` with the new free space
// - Retry allocation
#[derive(Debug)]
pub struct BufferManager<
    T: bytemuck::Pod + Send + Sync,
    const ITEMS_PER_CHUNK: usize,
    const NUM_CHUNKS: usize,
> {
    buffer: VulkanSubbuffer<[T]>,
    usage_map: Vec<BufferArea>,
    phantom: PhantomData<[[T; ITEMS_PER_CHUNK]; NUM_CHUNKS]>,
}

impl<T: bytemuck::Pod + Send + Sync, const ITEMS_PER_CHUNK: usize, const NUM_CHUNKS: usize>
    BufferManager<T, ITEMS_PER_CHUNK, NUM_CHUNKS>
{
    pub fn new(device: &Arc<VulkanDevice>, usage: VulkanBufferUsage) -> anyhow::Result<Self> {
        Ok(Self {
            buffer: vulkan_new_buffer_slice_large(
                device,
                usage
                    | VulkanBufferUsage::TRANSFER_DST
                    // NOTE: RADIANCE CASCADES
                    | VulkanBufferUsage::STORAGE_BUFFER,
                VulkanSharing::Exclusive,
                ITEMS_PER_CHUNK * NUM_CHUNKS,
            )?,
            usage_map: vec![BufferArea {
                usage: BufferAreaUsage::Free,
                num_chunks: NUM_CHUNKS as u64,
            }],
            phantom: PhantomData,
        })
    }

    pub fn alloc_area(
        &mut self,
        command_buffer: &mut VulkanAutoCommandBufferBuilder<VulkanPrimaryAutoCommandBuffer>,
        subchunk_coords: [i32; 3],
        items: Box<[T]>,
    ) -> u32 {
        debug_assert!(!items.is_empty());
        let num_chunks_needed = (items.len() as u64).div_ceil(ITEMS_PER_CHUNK as u64);
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
                let buffer_start_item = ITEMS_PER_CHUNK as u64 * current_start_chunk;
                let buffer_end_item = buffer_start_item + items.len() as u64;
                command_buffer
                    .update_buffer(
                        self.buffer
                            .clone()
                            .slice(buffer_start_item..buffer_end_item),
                        items,
                    )
                    .unwrap();
                return (ITEMS_PER_CHUNK as u64 * current_start_chunk)
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

    pub fn get_buffer(&self) -> VulkanSubbuffer<[T]> {
        self.buffer.clone()
    }

    pub fn size(&self) -> VulkanDeviceSize {
        self.buffer.size()
    }

    pub fn usage_fraction(&self) -> f64 {
        let mut total_chunks: u64 = 0;
        let mut used_chunks: u64 = 0;
        for area in &self.usage_map {
            total_chunks += area.num_chunks;
            if !area.is_free() {
                used_chunks += area.num_chunks;
            }
        }
        used_chunks as f64 / total_chunks as f64
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct VertexBufferManager<V: bytemuck::Pod + Send + Sync, const NUM_CHUNKS: usize>(
    BufferManager<V, 4, NUM_CHUNKS>,
);

impl<V: bytemuck::Pod + Send + Sync, const NUM_CHUNKS: usize> VertexBufferManager<V, NUM_CHUNKS> {
    pub fn new(device: &Arc<VulkanDevice>) -> anyhow::Result<Self> {
        BufferManager::new(device, VulkanBufferUsage::VERTEX_BUFFER).map(Self)
    }

    /// Returns the `first_vertex` indirect draw argument.
    pub fn alloc_area(
        &mut self,
        command_buffer: &mut VulkanAutoCommandBufferBuilder<VulkanPrimaryAutoCommandBuffer>,
        subchunk_coords: [i32; 3],
        base_quad: [V; 4],
    ) -> u32 {
        self.0
            .alloc_area(command_buffer, subchunk_coords, Box::new(base_quad))
    }

    pub fn free_subchunk_areas(&mut self, subchunk_coords: [i32; 3]) {
        self.0.free_subchunk_areas(subchunk_coords)
    }

    pub fn get_buffer(&self) -> VulkanSubbuffer<[V]> {
        self.0.get_buffer()
    }

    pub fn size(&self) -> VulkanDeviceSize {
        self.0.size()
    }

    pub fn usage_fraction(&self) -> f64 {
        self.0.usage_fraction()
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct InstanceBufferManager<
    I: bytemuck::Pod + Send + Sync,
    const ITEMS_PER_CHUNK: usize,
    const NUM_CHUNKS: usize,
>(BufferManager<I, ITEMS_PER_CHUNK, NUM_CHUNKS>);

impl<I: bytemuck::Pod + Send + Sync, const ITEMS_PER_CHUNK: usize, const NUM_CHUNKS: usize>
    InstanceBufferManager<I, ITEMS_PER_CHUNK, NUM_CHUNKS>
{
    pub fn new(device: &Arc<VulkanDevice>) -> anyhow::Result<Self> {
        BufferManager::new(device, VulkanBufferUsage::VERTEX_BUFFER).map(Self)
    }

    /// Returns the `first_instance` indirect draw argument.
    pub fn alloc_area(
        &mut self,
        command_buffer: &mut VulkanAutoCommandBufferBuilder<VulkanPrimaryAutoCommandBuffer>,
        subchunk_coords: [i32; 3],
        instances: Box<[I]>,
    ) -> u32 {
        self.0
            .alloc_area(command_buffer, subchunk_coords, instances)
    }

    pub fn free_subchunk_areas(&mut self, subchunk_coords: [i32; 3]) {
        self.0.free_subchunk_areas(subchunk_coords)
    }

    pub fn get_buffer(&self) -> VulkanSubbuffer<[I]> {
        self.0.get_buffer()
    }

    pub fn size(&self) -> VulkanDeviceSize {
        self.0.size()
    }

    pub fn usage_fraction(&self) -> f64 {
        self.0.usage_fraction()
    }
}

pub mod block_face {
    use super::*;

    pub fn create_graphics_pipeline(
        device: &Arc<VulkanDevice>,
        layout: &Arc<VulkanPipelineLayout>,
        subpass: &VulkanSubpass,
        // config: &SurfaceConfiguration,
        // layout: &PipelineLayout,
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
                        "shader::chunk::block_face::vertex",
                    ),
                    shader_stage_from_entry_point(
                        &mut None,
                        device,
                        "shader::chunk::block_face::fragment",
                    ),
                ],
                vertex_input_state: Some(&vertex_input_state::block_face()),
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
                    cull_mode: VulkanCullMode::Back,
                    ..Default::default()
                }),
                multisample_state: Some(&Default::default()),
                depth_stencil_state: Some(&VulkanDepthStencilState {
                    depth: Some(VulkanDepthState {
                        write_enable: true,
                        compare_op: VulkanCompareOp::GreaterOrEqual,
                    }),
                    ..Default::default()
                }),
                color_blend_state: Some(&VulkanColorBlendState {
                    attachments: &[VulkanColorBlendAttachmentState::default()],
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
        .context("Error while creating block face graphics pipeline")
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

    pub use shader_chunk_types::BlockFaceVertex as Vertex;

    pub use shader_chunk_types::BlockFaceInstance as Instance;

    pub type BlockFaceVertexBufferManager = VertexBufferManager<Vertex, { 1 << 20 }>;
    pub type BlockFaceInstanceBufferManager = InstanceBufferManager<Instance, 4, { 1 << 18 }>;
}

pub mod tinted_block_face {
    use super::*;

    pub fn create_graphics_pipeline(
        device: &Arc<VulkanDevice>,
        layout: &Arc<VulkanPipelineLayout>,
        subpass: &VulkanSubpass,
        // config: &SurfaceConfiguration,
        // layout: &PipelineLayout,
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
                        "shader::chunk::tinted_block_face::vertex",
                    ),
                    shader_stage_from_entry_point(
                        &mut None,
                        device,
                        "shader::chunk::tinted_block_face::fragment",
                    ),
                ],
                vertex_input_state: Some(&vertex_input_state::tinted_block_face()),
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
                    cull_mode: VulkanCullMode::Back,
                    ..Default::default()
                }),
                multisample_state: Some(&Default::default()),
                depth_stencil_state: Some(&VulkanDepthStencilState {
                    depth: Some(VulkanDepthState {
                        write_enable: true,
                        compare_op: VulkanCompareOp::GreaterOrEqual,
                    }),
                    ..Default::default()
                }),
                color_blend_state: Some(&VulkanColorBlendState {
                    attachments: &[VulkanColorBlendAttachmentState::default()],
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
        .context("Error while creating block face graphics pipeline")
    }

    pub use super::block_face::Vertex;

    pub use shader_chunk_types::TintedBlockFaceInstance as Instance;

    pub type TintedBlockFaceVertexBufferManager = VertexBufferManager<Vertex, { 1 << 20 }>;
    pub type TintedBlockFaceInstanceBufferManager = InstanceBufferManager<Instance, 4, { 1 << 18 }>;
}

pub mod custom_block {
    use super::*;

    pub fn create_graphics_pipeline(
        device: &Arc<VulkanDevice>,
        layout: &Arc<VulkanPipelineLayout>,
        subpass: &VulkanSubpass,
        // config: &SurfaceConfiguration,
        // layout: &PipelineLayout,
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
                        "shader::chunk::custom_block::vertex",
                    ),
                    shader_stage_from_entry_point(
                        &mut None,
                        device,
                        "shader::chunk::custom_block::fragment",
                    ),
                ],
                vertex_input_state: Some(&vertex_input_state::custom_block()),
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
                    cull_mode: VulkanCullMode::Back,
                    ..Default::default()
                }),
                multisample_state: Some(&Default::default()),
                depth_stencil_state: Some(&VulkanDepthStencilState {
                    depth: Some(VulkanDepthState {
                        write_enable: true,
                        compare_op: VulkanCompareOp::GreaterOrEqual,
                    }),
                    ..Default::default()
                }),
                color_blend_state: Some(&VulkanColorBlendState {
                    attachments: &[VulkanColorBlendAttachmentState::default()],
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
        .context("Error while creating block face graphics pipeline")
    }

    pub use shader_chunk_types::CustomBlockVertex as Vertex;

    pub use shader_chunk_types::CustomBlockInstance as Instance;

    pub type VertexList = VertexListBuffer<Vertex>;
    pub type IndexList = IndexListBuffer<u32>;
    pub type CustomBlockInstanceBufferManager =
        super::super::chunk::InstanceBufferManager<Instance, 4, { 1 << 20 }>;
}
