pub use super::chunk::{
    BufferManager, CustomBlockGroup, DrawArgsBuffer, IndexListBuffer, SubchunkConnectivity,
    VertexBufferManager, VertexListBuffer,
};
use super::shader_exports::chunk_rc::consts::CASCADE_0_NUM_RAYS;
use super::shader_exports::chunk_rc::types::{
    self as shader_chunk_rc_types,
    vertex_input_state,
};
use super::shader_exports::shader_stage_from_entry_point;
use crate::client::RayTracedQuadInfo;
use vulkan_prelude::*;
use anyhow::Context;
use nalgebra::{Matrix3, Rotation3};
use std::marker::{PhantomData, Send, Sync};
use std::sync::Arc;

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
    // NOTE: RADIANCE CASCADES
    pub rc_info: Option<SubchunkRadianceCascadeInfo>,
}

pub struct SubchunkRadianceCascadeInfo {
    pub blas: Arc<VulkanAccelerationStructure>,
    pub quads_info: Vec<RayTracedQuadInfo>,
    pub quads_info_offsets: [u32; 2],
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
pub struct InstanceBufferManager<
    T: bytemuck::Pod + Send + Sync,
    const ITEMS_PER_CHUNK: usize,
    const NUM_CHUNKS: usize,
> {
    buffer: VulkanSubbuffer<[T]>,
    // lightmap_buffers: [VulkanSubbuffer<[u8]>; 2],
    lightmap_front_buffer: VulkanSubbuffer<[u8]>,
    lightmap_back_buffer: VulkanSubbuffer<[u8]>,
    usage_map: Vec<BufferArea>,
    phantom: PhantomData<[[T; ITEMS_PER_CHUNK]; NUM_CHUNKS]>,
}

impl<T: bytemuck::Pod + Send + Sync, const ITEMS_PER_CHUNK: usize, const NUM_CHUNKS: usize>
    InstanceBufferManager<T, ITEMS_PER_CHUNK, NUM_CHUNKS>
{
    pub fn new(
        device: &Arc<VulkanDevice>,
        // memory_allocator: Arc<dyn VulkanMemoryAllocator>
        render_queue_family_index: u32,
        compute_queue_family_index: u32,
    ) -> anyhow::Result<Self> {
        dbg!(CASCADE_0_NUM_RAYS);
        // type LightmapNode = [u32; 256];
        type LightmapNode = [u16; 256];
        Ok(Self {
            buffer: vulkan_new_buffer_slice_large(
                device,
                VulkanBufferUsage::VERTEX_BUFFER
                    | VulkanBufferUsage::TRANSFER_DST
                    | VulkanBufferUsage::STORAGE_BUFFER,
                VulkanSharing::Exclusive,
                ITEMS_PER_CHUNK * NUM_CHUNKS,
            )
            .context("Error while creating instance buffer")?,
            // buffer: vulkan_new_buffer_slice(
            //     memory_allocator.clone(),
            //     VulkanBufferCreateInfo {
            //         usage: VulkanBufferUsage::VERTEX_BUFFER
            //             | VulkanBufferUsage::TRANSFER_DST
            //             | VulkanBufferUsage::STORAGE_BUFFER,
            //         ..Default::default()
            //     },
            //     VulkanAllocationCreateInfo {
            //         memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE,
            //         allocate_preference: VulkanMemoryAllocatePreference::AlwaysAllocate,
            //         ..Default::default()
            //     },
            //     ITEMS_PER_CHUNK * NUM_CHUNKS,
            // )
            // .context("Error while creating instance buffer")?,
            lightmap_front_buffer: vulkan_new_buffer_slice_large::<LightmapNode>(
                device,
                VulkanBufferUsage::STORAGE_BUFFER | VulkanBufferUsage::TRANSFER_DST,
                VulkanSharing::Concurrent(&[render_queue_family_index, compute_queue_family_index]),
                ITEMS_PER_CHUNK * NUM_CHUNKS,
            )
            .context("Error while creating cascade 0 front lightmap buffer")?
            .into_bytes(),
            lightmap_back_buffer: vulkan_new_buffer_slice_large::<LightmapNode>(
                device,
                VulkanBufferUsage::STORAGE_BUFFER | VulkanBufferUsage::TRANSFER_SRC,
                VulkanSharing::Exclusive,
                ITEMS_PER_CHUNK * NUM_CHUNKS,
            )
            .context("Error while creating cascade 0 back lightmap buffer")?
            .into_bytes(),
            // lightmap_buffers: [
            //     vulkan_new_buffer_slice_large::<[u32; 256]>(
            //         device.clone(),
            //         VulkanBufferUsage::STORAGE_BUFFER,
            //         ITEMS_PER_CHUNK * NUM_CHUNKS,
            //     )
            //     .context("Error while creating cascade 0 lightmap buffer")?
            //     .into_bytes(),
            //     // vulkan_new_buffer_slice_large::<[[u16; CASCADE_0_NUM_RAYS]; 64]>(
            //     //     device.clone(),
            //     //     VulkanBufferUsage::STORAGE_BUFFER,
            //     //     ITEMS_PER_CHUNK * NUM_CHUNKS,
            //     // )
            //     // .context("Error while creating cascade 1 lightmap buffer")?
            //     // .into_bytes(),
            // ],
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

    // pub fn get_lightmap_buffers(&self) -> &[VulkanSubbuffer<[u8]>; 1] {
    //     &self.lightmap_buffers
    // }

    pub fn get_lightmap_render_buffer(&self) -> &VulkanSubbuffer<[u8]> {
        &self.lightmap_front_buffer
    }

    pub fn get_lightmap_compute_buffer(&self) -> &VulkanSubbuffer<[u8]> {
        &self.lightmap_back_buffer
    }

    pub fn swap_lightmap_buffers(&mut self) {
        std::mem::swap(
            &mut self.lightmap_front_buffer,
            &mut self.lightmap_back_buffer,
        );
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

pub mod compute {
    use super::*;

    pub fn create_raytracing_debug_pipeline(
        device: &Arc<VulkanDevice>,
        layout: &Arc<VulkanPipelineLayout>,
    ) -> anyhow::Result<Arc<VulkanComputePipeline>> {
        VulkanComputePipeline::new(
            device,
            None, // No pipeline cache
            &VulkanComputePipelineCreateInfo {
                flags: VulkanPipelineCreateFlags::default(),
                stage: shader_stage_from_entry_point(
                    &mut None,
                    device,
                    "shader::chunk_rc::rc_compute::raytrace_debug",
                ),
                layout,
                base_pipeline: None,
                _ne: vulkano_non_exhaustive(),
            },
        )
        .context("Error while creating raytracing debug pipeline")
    }

    pub fn create_cascade_update_pipelines(
        device: &Arc<VulkanDevice>,
        layout: &Arc<VulkanPipelineLayout>,
    ) -> anyhow::Result<[Arc<VulkanComputePipeline>; 2]> {
        Ok([
            VulkanComputePipeline::new(
                device,
                None, // No pipeline cache
                &VulkanComputePipelineCreateInfo {
                    flags: VulkanPipelineCreateFlags::default(),
                    stage: shader_stage_from_entry_point(
                        &mut None,
                        device,
                        // "shader::chunk_rc::rc_compute::update_cascade_0",
                        "shader::chunk_rc::rc_compute::update_all_cascades",
                        // "shader::chunk_rc::rc_compute::single_pass_update",
                    ),
                    layout,
                    base_pipeline: None,
                    _ne: vulkano_non_exhaustive(),
                },
            )
            .context("Error while creating cascade 0 update pipeline")?,
            VulkanComputePipeline::new(
                device,
                None, // No pipeline cache
                &VulkanComputePipelineCreateInfo {
                    flags: VulkanPipelineCreateFlags::default(),
                    stage: shader_stage_from_entry_point(
                        &mut None,
                        device,
                        "shader::chunk_rc::rc_compute::update_cascade_1",
                    ),
                    layout,
                    base_pipeline: None,
                    _ne: vulkano_non_exhaustive(),
                },
            )
            .context("Error while creating cascade 1 update pipeline")?,
        ])
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
                        "shader::chunk_rc::block_face::vertex",
                    ),
                    shader_stage_from_entry_point(
                        &mut None,
                        device,
                        "shader::chunk_rc::block_face::fragment",
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
                subpass: Some(VulkanPipelineSubpassType::BeginRenderPass(&subpass)),
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

    pub use shader_chunk_rc_types::BlockFaceVertex as Vertex;
    
    pub use shader_chunk_rc_types::BlockFaceInstance as Instance;

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
                        "shader::chunk_rc::tinted_block_face::vertex",
                    ),
                    shader_stage_from_entry_point(
                        &mut None,
                        device,
                        "shader::chunk_rc::tinted_block_face::fragment",
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

    pub use shader_chunk_rc_types::TintedBlockFaceInstance as Instance;

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
                        "shader::chunk_rc::custom_block::vertex",
                    ),
                    shader_stage_from_entry_point(
                        &mut None,
                        device,
                        "shader::chunk_rc::custom_block::fragment",
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

    pub use shader_chunk_rc_types::CustomBlockVertex as Vertex;

    pub use shader_chunk_rc_types::CustomBlockInstance as Instance;

    pub type VertexList = VertexListBuffer<Vertex>;
    pub type IndexList = IndexListBuffer<u32>;
    pub type CustomBlockInstanceBufferManager =
        super::super::chunk::InstanceBufferManager<Instance, 4, { 1 << 20 }>;
}
