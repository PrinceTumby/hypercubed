use super::SubchunkDataStorage;
use super::shader_exports::chunk::types::{self as shader_chunk_types, vertex_input_state};
use super::shader_exports::shader_stage_from_entry_point;
use crate::basic_types::AxisDirection;
use crate::graphics::chunk::{HasSubchunkData, SubchunkConnectivity, SubchunkData};
use crate::{MAX_HEIGHT_I32, MIN_HEIGHT_I32, SUBCHUNK_AXIS_LEN, SUBCHUNK_AXIS_LEN_I32};
use ahash::AHasher;
use anyhow::Context;
use core::hash::Hasher;
use fixedbitset::FixedBitSet;
use nalgebra::{Matrix3, Rotation3};
use portable_std::{FastHashMap, FastHashSet};
use resources::block::RightAngleRotation;
use resources::block::blockstate::{self, BlockOpacity};
use resources::block::model::{ModelIndex, ModelType};
use resources::block::model::{ModelRegistry, Tint};
use resources::identifier;
use std::marker::{PhantomData, Send, Sync};
use std::sync::Arc;
use std::sync::mpsc::Sender;
use vulkan_prelude::*;

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
                usage | VulkanBufferUsage::TRANSFER_DST,
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
        // Find free area large enough to hold items.
        let mut current_start_chunk: u64 = 0;
        for i in 0..self.usage_map.len() {
            let area = &mut self.usage_map[i];
            if area.is_free() {
                use std::cmp::Ordering;
                match area.num_chunks.cmp(&num_chunks_needed) {
                    Ordering::Greater => {
                        // Split area into used portion and leftover free protion.
                        let new_free_area = BufferArea {
                            usage: BufferAreaUsage::Free,
                            num_chunks: area.num_chunks - num_chunks_needed,
                        };
                        area.num_chunks = num_chunks_needed;
                        area.usage = BufferAreaUsage::Used(subchunk_coords);
                        self.usage_map.insert(i + 1, new_free_area);
                    }
                    Ordering::Equal => {
                        // Mark entire area as used.
                        area.usage = BufferAreaUsage::Used(subchunk_coords);
                    }
                    Ordering::Less => {
                        current_start_chunk += area.num_chunks;
                        continue;
                    }
                }
                // Write items to buffer.
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
        // Mark all subchunk owned areas as free.
        for area in &mut self.usage_map {
            if area.belongs_to(subchunk_coords) {
                area.usage = BufferAreaUsage::Free;
            }
        }
        // Merge free areas.
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
    let spruce_leaves_registry_index = block_registry
        .get_index_from_identifier(&identifier!("minecraft:spruce_leaves"))
        .unwrap();
    let [subchunk_x, subchunk_y, subchunk_z] = subchunk_coords;
    let Some(chunk) = &raw_chunks.get(&[subchunk_x, subchunk_z]) else {
        pending_subchunk_tx.send(None).unwrap();
        return;
    };
    let chunk_section = &chunk.sections[usize::try_from(subchunk_y).unwrap()];
    if chunk_section.block_count == 0 {
        pending_subchunk_tx.send(None).unwrap();
        return;
    }
    // Skip chunks with missing neighbours, so that for every chunk we actually render, it
    // has all its neighbours to decide whether border faces should be rendered.
    // I believe Minecraft does the same.
    {
        let surrounding_chunk_coords = [
            [subchunk_x - 1, subchunk_z],
            [subchunk_x + 1, subchunk_z],
            [subchunk_x, subchunk_z - 1],
            [subchunk_x, subchunk_z + 1],
        ];
        for neighbour_chunk in surrounding_chunk_coords {
            if !raw_chunks.contains_key(&neighbour_chunk) {
                pending_subchunk_tx.send(None).unwrap();
                return;
            }
        }
    }
    let mut block_faces: [Vec<_>; 6] = Default::default();
    let mut tinted_block_faces: [Vec<_>; 6] = Default::default();
    let mut custom_block_instance_groups = FastHashMap::new();
    for y in 0..SUBCHUNK_AXIS_LEN {
        let global_y_i32 = (SUBCHUNK_AXIS_LEN_I32 * subchunk_y) + y as i32 + MIN_HEIGHT_I32;
        let global_y = global_y_i32 as f32;
        for z in 0..SUBCHUNK_AXIS_LEN {
            let global_z_i32 = (SUBCHUNK_AXIS_LEN_I32 * subchunk_z) + z as i32;
            let global_z = global_z_i32 as f32;
            for x in 0..SUBCHUNK_AXIS_LEN {
                let global_x_i32 = (SUBCHUNK_AXIS_LEN_I32 * subchunk_x) + x as i32;
                let global_x = global_x_i32 as f32;
                let global_palette_index = chunk_section.block_states.get(x, y, z);
                let blockstate_info = &block_registry[global_palette_index];
                let model_idx = match &blockstate_info.model_data {
                    blockstate::ModelData::Single(model_idx) => *model_idx,
                    blockstate::ModelData::RandomChoice(models) => 'model_blk: {
                        // Find weight for model by hashed position.
                        let mut block_hasher = AHasher::default();
                        block_hasher.write_i32(global_x_i32);
                        block_hasher.write_i32(global_y_i32);
                        block_hasher.write_i32(global_z_i32);
                        let hash = block_hasher.finish();
                        let mut current_percentage = (hash % 65537) as f32 / 65536.0;
                        for variant in models.iter() {
                            if current_percentage <= variant.weight {
                                break 'model_blk variant.model;
                            } else {
                                current_percentage -= variant.weight;
                            }
                        }
                        // Should be unreachable
                        let variant = &models[models.len() - 1];
                        variant.model
                    }
                };
                let block_opacity = blockstate_info.extra_info.opacity;
                let direction_map = [
                    (x as i32, y as i32 + 1, z as i32),
                    (x as i32, y as i32 - 1, z as i32),
                    (x as i32, y as i32, z as i32 - 1),
                    (x as i32, y as i32, z as i32 + 1),
                    (x as i32 + 1, y as i32, z as i32),
                    (x as i32 - 1, y as i32, z as i32),
                ];
                let mut face_cull_map = [false; 6];
                let mut face_light_map = [[0u8; 2]; 6];
                for (i, (x, y, z)) in direction_map.into_iter().enumerate() {
                    let check_global_y = (SUBCHUNK_AXIS_LEN_I32 * subchunk_y + y) + MIN_HEIGHT_I32;
                    let check_chunk = match [x, z].iter().any(|n| !(0..=15).contains(n)) {
                        false => chunk,
                        true => match (x, z) {
                            (-1, _) => &raw_chunks[&[subchunk_x - 1, subchunk_z]],
                            (16, _) => &raw_chunks[&[subchunk_x + 1, subchunk_z]],
                            (_, -1) => &raw_chunks[&[subchunk_x, subchunk_z - 1]],
                            (_, 16) => &raw_chunks[&[subchunk_x, subchunk_z + 1]],
                            _ => unreachable!(),
                        },
                    };
                    // Get lighting
                    {
                        let light_section = check_chunk
                            .lighting
                            .get_section(
                                MIN_HEIGHT_I32,
                                check_global_y.div_euclid(SUBCHUNK_AXIS_LEN_I32),
                            )
                            .unwrap();
                        let (x, y, z) = (
                            ((x + SUBCHUNK_AXIS_LEN_I32) % SUBCHUNK_AXIS_LEN_I32) as usize,
                            y.rem_euclid(16) as usize,
                            ((z + SUBCHUNK_AXIS_LEN_I32) % SUBCHUNK_AXIS_LEN_I32) as usize,
                        );
                        face_light_map[i] = light_section.get(x, y, z);
                    }
                    if !(MIN_HEIGHT_I32..=MAX_HEIGHT_I32).contains(&check_global_y) {
                        continue;
                    }
                    let check_sections = &check_chunk.sections;
                    let indexing_section = &check_sections[usize::try_from(
                        (SUBCHUNK_AXIS_LEN_I32 * subchunk_y + y) / SUBCHUNK_AXIS_LEN_I32,
                    )
                    .unwrap()];
                    let (x, y, z) = (
                        ((x + SUBCHUNK_AXIS_LEN_I32) % SUBCHUNK_AXIS_LEN_I32) as usize,
                        y as usize,
                        ((z + SUBCHUNK_AXIS_LEN_I32) % SUBCHUNK_AXIS_LEN_I32) as usize,
                    );
                    let global_palette_index = indexing_section.block_states.get(x, y % 16, z);
                    let neighbour_blockstate_info = &block_registry[global_palette_index];
                    let neighbour_block_opacity = neighbour_blockstate_info.extra_info.opacity;
                    face_cull_map[i] = match (block_opacity, neighbour_block_opacity) {
                        (_, BlockOpacity::Opaque) => true,
                        (BlockOpacity::Glass, BlockOpacity::Glass) => true,
                        (BlockOpacity::GlassPane, BlockOpacity::GlassPane) => true,
                        (_, _) => false,
                    };
                }
                // Spruce Leaves are hardcoded, so override tint colour here
                let tint_color = match blockstate_info.block_index {
                    ident if ident == spruce_leaves_registry_index => [0x61, 0x99, 0x61, 0xFF],
                    _ => [0x91, 0xBD, 0x59, 0xFF],
                };
                process_subchunk_model(
                    &mut block_faces,
                    &mut tinted_block_faces,
                    &mut custom_block_instance_groups,
                    model_registry,
                    chunk,
                    blockstate_info,
                    block_opacity,
                    face_cull_map,
                    face_light_map,
                    tint_color,
                    [subchunk_x, subchunk_y, subchunk_z],
                    [global_x, global_y, global_z],
                    [x, y, z],
                    model_idx,
                );
            }
        }
    }
    // Runs a variant of Minecraft's cave culling algorithm, specifically the connected
    // face generation.
    // Outlined here: https://tomcc.github.io/2014/08/31/visibility-1.html
    let connectivity = 'connected_faces: {
        use crate::protocol::chunk::Palette;
        // If we can immediately tell all the subchunk blocks are opaque, skip this entire
        // process and just return that no subchunk faces are connected.
        match chunk_section.block_states.palette() {
            Palette::SingleValue(global_palette_index) => {
                let blockstate_info = &block_registry[*global_palette_index];
                break 'connected_faces match blockstate_info.extra_info.opacity {
                    BlockOpacity::Opaque => SubchunkConnectivity::empty(),
                    _ => SubchunkConnectivity::full(),
                };
            }
            Palette::Palette(indices) => {
                let mut num_opaque = 0;
                for global_palette_index in indices {
                    let blockstate_info = &block_registry[*global_palette_index];
                    if blockstate_info.extra_info.opacity == BlockOpacity::Opaque {
                        num_opaque += 1;
                    }
                }
                if num_opaque == 0 {
                    break 'connected_faces SubchunkConnectivity::full();
                } else if num_opaque == indices.len() {
                    break 'connected_faces SubchunkConnectivity::empty();
                }
            }
            Palette::Direct => {}
        }
        #[repr(transparent)]
        #[derive(Clone, Copy)]
        struct FaceSet(pub u8);
        impl FaceSet {
            pub fn empty() -> Self {
                Self(0)
            }

            pub fn add_dir(&mut self, dir: AxisDirection) {
                self.0 |= 1 << (dir as u8);
            }

            pub fn get_directions(&self) -> [(AxisDirection, bool); 6] {
                [
                    AxisDirection::Down,
                    AxisDirection::Up,
                    AxisDirection::North,
                    AxisDirection::South,
                    AxisDirection::West,
                    AxisDirection::East,
                ]
                .map(|dir| (dir, self.0 & (1 << (dir as u8)) != 0))
            }
        }
        let mut current_group: usize = 0;
        let mut current_group_faces = FaceSet::empty();
        let mut group_faces: Vec<FaceSet> = Vec::new();
        // Y major, then Z, then X.
        let mut unchecked_blocks = FixedBitSet::with_capacity(SUBCHUNK_AXIS_LEN.pow(3));
        #[inline]
        fn coords_to_bit_idx(coords: [i8; 3]) -> usize {
            let [x, y, z] = coords.map(|n| n as usize);
            y * SUBCHUNK_AXIS_LEN.pow(2) + z * SUBCHUNK_AXIS_LEN + x
        }
        unchecked_blocks.clear();
        // Add all non-opaque blocks
        for x in 0..SUBCHUNK_AXIS_LEN {
            for y in 0..SUBCHUNK_AXIS_LEN {
                for z in 0..SUBCHUNK_AXIS_LEN {
                    let global_palette_index = chunk_section.block_states.get(x, y, z);
                    let blockstate_info = &block_registry[global_palette_index];
                    if blockstate_info.extra_info.opacity != BlockOpacity::Opaque {
                        let bit_index = coords_to_bit_idx([x, y, z].map(|n| n as i8));
                        unchecked_blocks.insert(bit_index);
                    }
                }
            }
        }
        // Flood fill from each non-opaque block, to split all the blocks into groups.
        let mut queue: FastHashSet<[i8; 3]> = FastHashSet::new();
        while !queue.is_empty() || !unchecked_blocks.is_clear() {
            let [x, y, z] = queue
                .iter()
                .copied()
                .next()
                .inspect(|coord| {
                    queue.remove(coord);
                })
                .unwrap_or_else(|| {
                    // No more blocks in queue, make a new group and grab a new block
                    // that hasn't been checked yet.
                    let coord = {
                        let bit_index = unchecked_blocks.minimum().unwrap();
                        [
                            (bit_index & 0xF) as i8,
                            ((bit_index >> 8) & 0xF) as i8,
                            ((bit_index >> 4) & 0xF) as i8,
                        ]
                    };
                    group_faces.push(current_group_faces);
                    current_group += 1;
                    current_group_faces = FaceSet::empty();
                    coord
                });
            unchecked_blocks.remove(coords_to_bit_idx([x, y, z]));
            let surrounding_block_coords = [
                [x - 1, y, z],
                [x + 1, y, z],
                [x, y, z - 1],
                [x, y, z + 1],
                [x, y - 1, z],
                [x, y + 1, z],
            ];
            for new_coord in surrounding_block_coords {
                let [new_x, new_y, new_z] = new_coord;
                // If fill escapes subchunk, add escaping face to group
                if new_x < 0 {
                    current_group_faces.add_dir(AxisDirection::West);
                } else if new_x >= SUBCHUNK_AXIS_LEN as i8 {
                    current_group_faces.add_dir(AxisDirection::East);
                } else if new_y < 0 {
                    current_group_faces.add_dir(AxisDirection::Down);
                } else if new_y >= SUBCHUNK_AXIS_LEN as i8 {
                    current_group_faces.add_dir(AxisDirection::Up);
                } else if new_z < 0 {
                    current_group_faces.add_dir(AxisDirection::North);
                } else if new_z >= SUBCHUNK_AXIS_LEN as i8 {
                    current_group_faces.add_dir(AxisDirection::South);
                } else if unchecked_blocks.contains(coords_to_bit_idx(new_coord)) {
                    queue.insert(new_coord);
                }
            }
        }
        group_faces.push(current_group_faces);
        // Add connected faces for each group to subchunk connectivity
        let mut subchunk_connectivity = SubchunkConnectivity::empty();
        for face_set in group_faces {
            let directions = face_set.get_directions();
            for (face_1, face_1_in_set) in directions {
                if !face_1_in_set {
                    continue;
                }
                for (face_2, face_2_in_set) in directions {
                    if !face_2_in_set {
                        continue;
                    }
                    subchunk_connectivity.add_connection(&face_1, &face_2);
                }
            }
        }
        subchunk_connectivity
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
    command_buffer: &mut VulkanAutoCommandBufferBuilder<VulkanPrimaryAutoCommandBuffer>,
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
            subchunk_data_storage.$buffer_manager.alloc_area(
                command_buffer,
                $subchunk_coords,
                $data,
            )
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

#[tracing::instrument(skip_all)]
fn process_subchunk_model(
    block_faces: &mut [Vec<block_face::Instance>; 6],
    tinted_block_faces: &mut [Vec<tinted_block_face::Instance>; 6],
    custom_block_instance_groups: &mut FastHashMap<
        resources::block::model::OtherInfo,
        Vec<custom_block::Instance>,
    >,
    model_registry: &ModelRegistry,
    chunk: &crate::RawChunk,
    blockstate_info: &blockstate::Blockstate,
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
        ModelType::None => return,
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
                            match info.per_face_uv_rotations[i] {
                                RightAngleRotation::Zero => 0,
                                RightAngleRotation::Ninety => 1,
                                RightAngleRotation::OneEighty => 2,
                                RightAngleRotation::TwoSeventy => 3,
                            },
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
                            match info.per_face_uv_rotations[i] {
                                RightAngleRotation::Zero => 0,
                                RightAngleRotation::Ninety => 1,
                                RightAngleRotation::OneEighty => 2,
                                RightAngleRotation::TwoSeventy => 3,
                            },
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
                    match info.per_face_uv_rotations[i] {
                        RightAngleRotation::Zero => 0,
                        RightAngleRotation::Ninety => 1,
                        RightAngleRotation::OneEighty => 2,
                        RightAngleRotation::TwoSeventy => 3,
                    },
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
                            match face.uv_rotation {
                                RightAngleRotation::Zero => 0,
                                RightAngleRotation::Ninety => 1,
                                RightAngleRotation::OneEighty => 2,
                                RightAngleRotation::TwoSeventy => 3,
                            },
                            face_light_map[face.face_i as usize],
                            tint_color,
                        ),
                    );
                } else {
                    block_faces[face.face_i as usize].push(block_face::Instance::new(
                        [x as u8, y as u8, z as u8],
                        face.atlas_uvs,
                        #[cfg(feature = "graphics_backend_vulkan")]
                        match face.uv_rotation {
                            RightAngleRotation::Zero => 0,
                            RightAngleRotation::Ninety => 1,
                            RightAngleRotation::OneEighty => 2,
                            RightAngleRotation::TwoSeventy => 3,
                        },
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
            let block_instances = custom_block_instance_groups
                .entry(*info)
                .or_insert_with(Vec::new);
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
                    blockstate_info,
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
