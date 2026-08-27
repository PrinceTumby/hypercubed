use super::shader_exports::chunk as shader_chunk_types;
use super::shader_exports::shader_stage_from_entry_point;
use crate::graphics::chunk::{HasSubchunkData, SubchunkConnectivity, SubchunkData};
use crate::{MIN_HEIGHT_I32, SUBCHUNK_AXIS_LEN_I32};
use anyhow::Context;
use core::num::NonZeroU64;
use nalgebra::{Matrix3, Rotation3};
use portable_std::FastHashMap;
use resources::block::RightAngleRotation;
use resources::block::blockstate::BlockOpacity;
use resources::block::model::{ModelIndex, ModelType};
use resources::block::model::{ModelRegistry, Tint};
use std::marker::{Send, Sync};
use std::sync::Arc;
use std::sync::mpsc::Sender;
use vulkano_prelude::*;

pub struct Subchunk {
    pub dispatch_id: u64,
    pub start_coords: [i32; 3],
    pub block_face_instances_buffer: Option<AddressedBuffer<[block_face::Instance]>>,
    /// Equal to `(0, 0)` if the direction group contains no instances.
    pub block_face_instance_groups: [(u32, u32); 6],
    pub tinted_block_face_instances_buffer: Option<AddressedBuffer<[tinted_block_face::Instance]>>,
    /// Equal to `(0, 0)` if the direction group contains no instances.
    pub tinted_block_face_instance_groups: [(u32, u32); 6],
    pub custom_block_instances_buffer: Option<AddressedBuffer<[custom_block::Instance]>>,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddressedBuffer<T: ?Sized> {
    pub buffer: VulkanSubbuffer<T>,
    pub address: NonZeroU64,
}

impl<T: ?Sized> AddressedBuffer<T> {
    /// Panics if retrieving the buffer device address fails.
    pub fn from_buffer(buffer: VulkanSubbuffer<T>) -> Self {
        let address = buffer
            .device_address()
            .expect("Error while getting buffer device address");
        Self { buffer, address }
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

pub mod block_face {
    use super::*;

    pub fn create_graphics_pipeline(
        device: &Arc<VulkanDevice>,
        layout: &Arc<VulkanPipelineLayout>,
        subpass: &VulkanSubpass,
    ) -> anyhow::Result<Arc<VulkanGraphicsPipeline>> {
        VulkanGraphicsPipeline::new(
            device,
            None, // No pipeline cache
            &VulkanGraphicsPipelineCreateInfo {
                flags: VulkanPipelineCreateFlags::default(),
                stages: &[
                    // shader_stage_from_entry_point(
                    //     &mut None,
                    //     device,
                    //     "shader::chunk::block_face::vertex",
                    // ),
                    // shader_stage_from_entry_point(
                    //     &mut None,
                    //     device,
                    //     "shader::chunk::block_face::fragment",
                    // ),
                    shader_stage_from_entry_point(&mut None, device, "chunk::block_face", "vertex"),
                    shader_stage_from_entry_point(
                        &mut None,
                        device,
                        "chunk::block_face",
                        "fragment",
                    ),
                ],
                vertex_input_state: Some(&VulkanVertexInputState::default()),
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
}

pub mod tinted_block_face {
    use super::*;

    pub fn create_graphics_pipeline(
        device: &Arc<VulkanDevice>,
        layout: &Arc<VulkanPipelineLayout>,
        subpass: &VulkanSubpass,
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
                        "chunk::tinted_block_face",
                        "vertex",
                    ),
                    shader_stage_from_entry_point(
                        &mut None,
                        device,
                        "chunk::tinted_block_face",
                        "fragment",
                    ),
                ],
                vertex_input_state: Some(&VulkanVertexInputState::default()),
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
}

pub mod custom_block {
    use super::*;

    pub fn create_graphics_pipeline(
        device: &Arc<VulkanDevice>,
        layout: &Arc<VulkanPipelineLayout>,
        subpass: &VulkanSubpass,
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
                        "chunk::custom_block",
                        "vertex",
                    ),
                    shader_stage_from_entry_point(
                        &mut None,
                        device,
                        "chunk::custom_block",
                        "fragment",
                    ),
                ],
                vertex_input_state: Some(&VulkanVertexInputState::default()),
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
}

#[tracing::instrument(skip_all)]
pub fn process_subchunk(
    block_registry: &resources::block::Registry,
    model_registry: &ModelRegistry,
    raw_chunks: &FastHashMap<[i32; 2], Arc<crate::RawChunk>>,
    pending_subchunk_tx: &Sender<Option<([i32; 3], Subchunk)>>,
    memory_allocator: &Arc<dyn VulkanMemoryAllocator>,
    subchunk_coords: [i32; 3],
    dispatch_id: u64,
) -> anyhow::Result<()> {
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
        return Ok(());
    };
    let start_coords = [
        SUBCHUNK_AXIS_LEN_I32 * subchunk_x,
        SUBCHUNK_AXIS_LEN_I32 * subchunk_y + MIN_HEIGHT_I32,
        SUBCHUNK_AXIS_LEN_I32 * subchunk_z,
    ];
    // Block faces
    let mut block_face_instances = Vec::new();
    let mut block_face_instance_groups: [(u32, u32); 6] = [(0, 0); 6];
    for (i, instance_group) in block_faces
        .into_iter()
        .enumerate()
        .filter(|(_i, group)| !group.is_empty())
    {
        let instance_group_len: u32 = instance_group.len().try_into().unwrap();
        let instance_group_start: u32 = block_face_instances.len().try_into().unwrap();
        block_face_instances.extend(instance_group);
        block_face_instance_groups[i] = (instance_group_start, instance_group_len);
    }
    let block_face_instances_buffer = (!block_face_instances.is_empty())
        .then(|| {
            VulkanBuffer::from_iter(
                memory_allocator,
                &VulkanBufferCreateInfo {
                    usage: VulkanBufferUsage::SHADER_DEVICE_ADDRESS,
                    ..Default::default()
                },
                &VulkanAllocationCreateInfo {
                    memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                        | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                block_face_instances,
            )
            .context("Error while creating subchunk block face instances buffer")
            .map(AddressedBuffer::from_buffer)
        })
        .transpose()?;
    // Tinted block faces
    let mut tinted_block_face_instances = Vec::new();
    let mut tinted_block_face_instance_groups: [(u32, u32); 6] = [(0, 0); 6];
    for (i, instance_group) in tinted_block_faces
        .into_iter()
        .enumerate()
        .filter(|(_i, group)| !group.is_empty())
    {
        let instance_group_len: u32 = instance_group.len().try_into().unwrap();
        let instance_group_start: u32 = tinted_block_face_instances.len().try_into().unwrap();
        tinted_block_face_instances.extend(instance_group);
        tinted_block_face_instance_groups[i] = (instance_group_start, instance_group_len);
    }
    let tinted_block_face_instances_buffer = (!tinted_block_face_instances.is_empty())
        .then(|| {
            VulkanBuffer::from_iter(
                memory_allocator,
                &VulkanBufferCreateInfo {
                    usage: VulkanBufferUsage::SHADER_DEVICE_ADDRESS,
                    ..Default::default()
                },
                &VulkanAllocationCreateInfo {
                    memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                        | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                tinted_block_face_instances,
            )
            .context("Error while creating subchunk tinted block face instances buffer")
            .map(AddressedBuffer::from_buffer)
        })
        .transpose()?;
    // Custom blocks
    let mut custom_block_instances = Vec::new();
    let custom_block_groups = custom_block_instance_groups
        .into_iter()
        .map(|(info, instances)| {
            let instance_group_len: u32 = instances.len().try_into().unwrap();
            let instance_group_start: u32 = custom_block_instances.len().try_into().unwrap();
            custom_block_instances.extend(instances);
            CustomBlockGroup {
                start_face_and_len: info.start_face_and_len,
                start_instance_and_len: [instance_group_start, instance_group_len],
            }
        })
        .collect();
    let custom_block_instances_buffer = (!custom_block_instances.is_empty())
        .then(|| {
            VulkanBuffer::from_iter(
                memory_allocator,
                &VulkanBufferCreateInfo {
                    usage: VulkanBufferUsage::SHADER_DEVICE_ADDRESS,
                    ..Default::default()
                },
                &VulkanAllocationCreateInfo {
                    memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                        | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                custom_block_instances,
            )
            .context("Error while creating subchunk custom block instances buffer")
            .map(AddressedBuffer::from_buffer)
        })
        .transpose()?;
    pending_subchunk_tx
        .send(Some((
            subchunk_coords,
            Subchunk {
                dispatch_id,
                start_coords,
                block_face_instances_buffer,
                block_face_instance_groups,
                tinted_block_face_instances_buffer,
                tinted_block_face_instance_groups,
                custom_block_instances_buffer,
                custom_block_groups,
                connectivity,
            },
        )))
        .unwrap();
    Ok(())
}

#[allow(clippy::too_many_arguments)]
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
