pub use smallvec::SmallVec;
pub use vulkano::acceleration_structure::{
    AccelerationStructure as VulkanAccelerationStructure,
    AccelerationStructureBuildGeometryInfo as VulkanAccelerationStructureBuildGeometryInfo,
    AccelerationStructureBuildRangeInfo as VulkanAccelerationStructureBuildRangeInfo,
    AccelerationStructureBuildType as VulkanAccelerationStructureBuildType,
    AccelerationStructureCreateFlags as VulkanAccelerationStructureCreateFlags,
    AccelerationStructureCreateInfo as VulkanAccelerationStructureCreateInfo,
    AccelerationStructureGeometries as VulkanAccelerationStructureGeometries,
    AccelerationStructureGeometryAabbsData as VulkanAccelerationStructureGeometryAabbsData,
    AccelerationStructureGeometryInstancesData as VulkanAccelerationStructureGeometryInstancesData,
    AccelerationStructureGeometryInstancesDataType as VulkanAccelerationStructureGeometryInstancesDataType,
    AccelerationStructureGeometryTrianglesData as VulkanAccelerationStructureGeometryTrianglesData,
    AccelerationStructureInstance as VulkanAccelerationStructureInstance,
    AccelerationStructureType as VulkanAccelerationStructureType,
    BuildAccelerationStructureFlags as VulkanBuildAccelerationStructureFlags,
    BuildAccelerationStructureMode as VulkanBuildAccelerationStructureMode,
    GeometryFlags as VulkanGeometryFlags, GeometryInstanceFlags as VulkanGeometryInstanceFlags,
};
pub use vulkano::buffer::{
    Buffer as VulkanBuffer, BufferCreateInfo as VulkanBufferCreateInfo,
    BufferUsage as VulkanBufferUsage, IndexBuffer as VulkanIndexBuffer,
    RawBuffer as VulkanRawBuffer, Subbuffer as VulkanSubbuffer,
};
pub use vulkano::command_buffer::allocator::{
    CommandBufferAllocator as VulkanCommandBufferAllocator,
    StandardCommandBufferAllocator as VulkanStandardCommandBufferAllocator,
};
pub use vulkano::command_buffer::auto::{
    AutoCommandBufferBuilder as VulkanAutoCommandBufferBuilder,
    PrimaryAutoCommandBuffer as VulkanPrimaryAutoCommandBuffer,
};
pub use vulkano::command_buffer::{
    BufferCopy as VulkanBufferCopy, BufferImageCopy as VulkanBufferImageCopy,
    CommandBufferBeginInfo as VulkanCommandBufferBeginInfo,
    CommandBufferLevel as VulkanCommandBufferLevel,
    CommandBufferSubmitInfo as VulkanCommandBufferSubmitInfo,
    CommandBufferUsage as VulkanCommandBufferUsage, CopyBufferInfo as VulkanCopyBufferInfo,
    CopyBufferInfoTyped as VulkanCopyBufferInfoTyped,
    CopyBufferToImageInfo as VulkanCopyBufferToImageInfo,
    DrawIndexedIndirectCommand as VulkanDrawIndexedIndirectCommand,
    DrawIndirectCommand as VulkanDrawIndirectCommand,
    RecordingCommandBuffer as VulkanRecordingCommandBuffer,
    RenderPassBeginInfo as VulkanRenderPassBeginInfo,
    SemaphoreSubmitInfo as VulkanSemaphoreSubmitInfo, SubmitInfo as VulkanSubmitInfo,
    SubpassBeginInfo as VulkanSubpassBeginInfo, SubpassContents as VulkanSubpassContents,
    SubpassEndInfo as VulkanSubpassEndInfo,
};
pub use vulkano::descriptor_set::allocator::{
    DescriptorSetAllocator as VulkanDescriptorSetAllocator,
    StandardDescriptorSetAllocator as VulkanStandardDescriptorSetAllocator,
    StandardDescriptorSetAllocatorCreateInfo as VulkanStandardDescriptorSetAllocatorCreateInfo,
};
pub use vulkano::descriptor_set::layout::{
    DescriptorBindingFlags as VulkanDescriptorBindingFlags,
    DescriptorSetLayout as VulkanDescriptorSetLayout,
    DescriptorSetLayoutBinding as VulkanDescriptorSetLayoutBinding,
    DescriptorSetLayoutCreateFlags as VulkanDescriptorSetLayoutCreateFlags,
    DescriptorSetLayoutCreateInfo as VulkanDescriptorSetLayoutCreateInfo,
    DescriptorType as VulkanDescriptorType,
};
pub use vulkano::descriptor_set::{
    CopyDescriptorSet as VulkanCopyDescriptorSet, DescriptorSet as VulkanDescriptorSet,
    WriteDescriptorSet as VulkanWriteDescriptorSet,
};
pub use vulkano::device::physical::PhysicalDeviceType as VulkanPhysicalDeviceType;
pub use vulkano::device::{
    Device as VulkanDevice, DeviceCreateInfo as VulkanDeviceCreateInfo,
    DeviceExtensions as VulkanDeviceExtensions, DeviceFeatures as VulkanDeviceFeatures,
    Queue as VulkanQueue, QueueCreateInfo as VulkanQueueCreateInfo, QueueFlags as VulkanQueueFlags,
};
pub use vulkano::format::Format as VulkanFormat;
pub use vulkano::image::sampler::{
    Filter as VulkanFilter, Sampler as VulkanSampler,
    SamplerAddressMode as VulkanSamplerAddressMode, SamplerCreateInfo as VulkanSamplerCreateInfo,
    SamplerMipmapMode as VulkanSamplerMipmapMode,
};
pub use vulkano::image::view::ImageView as VulkanImageView;
pub use vulkano::image::{
    Image as VulkanImage, ImageAspects as VulkanImageAspects,
    ImageCreateInfo as VulkanImageCreateInfo, ImageLayout as VulkanImageLayout,
    ImageSubresourceLayers as VulkanImageSubresourceLayers, ImageType as VulkanImageType,
    ImageUsage as VulkanImageUsage,
};
pub use vulkano::instance::debug::{
    DebugUtilsMessageSeverity as VulkanDebugUtilsMessageSeverity,
    DebugUtilsMessageType as VulkanDebugUtilsMessageType,
    DebugUtilsMessenger as VulkanDebugUtilsMessenger,
    DebugUtilsMessengerCallback as VulkanDebugUtilsMessengerCallback,
    DebugUtilsMessengerCreateInfo as VulkanDebugUtilsMessengerCreateInfo,
    ValidationFeatureEnable as VulkanValidationFeatureEnable,
};
pub use vulkano::instance::{
    Instance as VulkanInstance, InstanceCreateInfo as VulkanInstanceCreateInfo,
    InstanceExtensions as VulkanInstanceExtensions,
};
pub use vulkano::memory::allocator::{
    AllocationCreateInfo as VulkanAllocationCreateInfo,
    MemoryAllocatePreference as VulkanMemoryAllocatePreference,
    MemoryAllocator as VulkanMemoryAllocator, MemoryTypeFilter as VulkanMemoryTypeFilter,
    StandardMemoryAllocator as VulkanStandardMemoryAllocator,
};
pub use vulkano::memory::{
    DedicatedAllocation as VulkanDedicatedAllocation, DeviceMemory as VulkanDeviceMemory,
    MemoryAllocateInfo as VulkanMemoryAllocateInfo, ResourceMemory as VulkanResourceMemory,
};
pub use vulkano::pipeline::compute::ComputePipelineCreateInfo as VulkanComputePipelineCreateInfo;
pub use vulkano::pipeline::graphics::GraphicsPipelineCreateInfo as VulkanGraphicsPipelineCreateInfo;
pub use vulkano::pipeline::graphics::color_blend::{
    AttachmentBlend as VulkanAttachmentBlend, BlendFactor as VulkanBlendFactor,
    BlendOp as VulkanBlendOp, ColorBlendAttachmentState as VulkanColorBlendAttachmentState,
    ColorBlendState as VulkanColorBlendState, ColorBlendStateFlags as VulkanColorBlendStateFlags,
    ColorComponents as VulkanColorComponents, LogicOp as VulkanLogicOp,
};
pub use vulkano::pipeline::graphics::depth_stencil::{
    CompareOp as VulkanCompareOp, DepthState as VulkanDepthState,
    DepthStencilState as VulkanDepthStencilState,
    DepthStencilStateFlags as VulkanDepthStencilStateFlags,
};
pub use vulkano::pipeline::graphics::input_assembly::{
    InputAssemblyState as VulkanInputAssemblyState, PrimitiveTopology as VulkanPrimitiveTopology,
};
pub use vulkano::pipeline::graphics::multisample::MultisampleState as VulkanMultisampleState;
pub use vulkano::pipeline::graphics::rasterization::{
    CullMode as VulkanCullMode, RasterizationState as VulkanRasterizationState,
};
pub use vulkano::pipeline::graphics::subpass::{
    PipelineRenderingCreateInfo as VulkanPipelineRenderingCreateInfo,
    PipelineSubpassType as VulkanPipelineSubpassType,
};
pub use vulkano::pipeline::graphics::vertex_input::{
    VertexInputAttributeDescription as VulkanVertexInputAttributeDescription,
    VertexInputBindingDescription as VulkanVertexInputBindingDescription,
    VertexInputRate as VulkanVertexInputRate, VertexInputState as VulkanVertexInputState,
};
pub use vulkano::pipeline::graphics::viewport::{
    Scissor as VulkanScissor, Viewport as VulkanViewport, ViewportState as VulkanViewportState,
};
pub use vulkano::pipeline::layout::{
    PipelineLayoutCreateFlags as VulkanPipelineLayoutCreateFlags,
    PipelineLayoutCreateInfo as VulkanPipelineLayoutCreateInfo,
    PushConstantRange as VulkanPushConstantRange,
};
pub use vulkano::pipeline::{
    ComputePipeline as VulkanComputePipeline, DynamicState as VulkanDynamicState,
    GraphicsPipeline as VulkanGraphicsPipeline, PipelineBindPoint as VulkanPipelineBindPoint,
    PipelineCreateFlags as VulkanPipelineCreateFlags, PipelineLayout as VulkanPipelineLayout,
    PipelineShaderStageCreateFlags as VulkanPipelineShaderStageCreateFlags,
    PipelineShaderStageCreateInfo as VulkanPipelineShaderStageCreateInfo,
};
pub use vulkano::render_pass::{
    Framebuffer as VulkanFramebuffer, FramebufferCreateFlags as VulkanFramebufferCreateFlags,
    FramebufferCreateInfo as VulkanFramebufferCreateInfo, RenderPass as VulkanRenderPass,
    Subpass as VulkanSubpass,
};
pub use vulkano::shader::{
    EntryPoint as VulkanEntryPoint, ShaderModule as VulkanShaderModule,
    ShaderModuleCreateInfo as VulkanShaderModuleCreateInfo, ShaderStage as VulkanShaderStage,
    ShaderStages as VulkanShaderStages,
};
pub use vulkano::swapchain::{
    AcquireNextImageInfo as VulkanAcquireNextImageInfo, AcquiredImage as VulkanAcquiredImage,
    PresentInfo as VulkanPresentInfo, PresentMode as VulkanPresentMode,
    SemaphorePresentInfo as VulkanSemaphorePresentInfo, Surface as VulkanSurface,
    Swapchain as VulkanSwapchain, SwapchainCreateInfo as VulkanSwapchainCreateInfo,
    SwapchainPresentInfo as VulkanSwapchainPresentInfo,
    acquire_next_image as vulkan_swapchain_acquire_next_image,
};
pub use vulkano::sync::fence::Fence as VulkanFence;
pub use vulkano::sync::semaphore::Semaphore as VulkanSemaphore;
pub use vulkano::sync::{GpuFuture, Sharing as VulkanSharing};
pub use vulkano::{
    DeviceSize as VulkanDeviceSize, Packed24_8 as VulkanPacked24_8, Validated as VulkanValidated,
    VulkanError, VulkanLibrary, single_pass_renderpass as vulkan_single_pass_renderpass,
};

// HACK: Some vulkano structs don't seem to implement Default when they should, so this is useful
//       to still be able to create the structs.
//       Should probably file a bug report.
pub fn vulkano_non_exhaustive() -> vulkano::NonExhaustive<'static> {
    VulkanAllocationCreateInfo::default()._ne
}

#[inline]
pub fn vertex_stride<T: Copy>() -> u32 {
    std::mem::size_of::<T>()
        .next_multiple_of(std::mem::align_of::<T>())
        .try_into()
        .unwrap()
}

#[macro_export]
macro_rules! vulkan_vertex_default_input_rate {
    (Vertex) => {
        VulkanVertexInputRate::Vertex
    };
    (Instance) => {
        VulkanVertexInputRate::Instance { divisor: 1 }
    };
}

#[macro_export]
macro_rules! vulkan_vertex_bindings {
    ( $( $binding:literal => ($vertex_type:ty, $input_rate:ident) $(,)? )* ) => {
        ::foldhash::HashMap::from_iter([
            $(
                (
                    $binding,
                    VulkanVertexInputBindingDescription {
                        stride: vertex_stride::<$vertex_type>(),
                        input_rate: vulkan_vertex_default_input_rate!($input_rate),
                        ..Default::default()
                    }
                )
            ),*
        ])
        .into()
    };
}

#[macro_export]
macro_rules! vulkan_vertex_attributes {
    ( $num_bindings:literal, [ $( [ $attribute:literal <- $binding:literal ] => $format:ident $(,)? )* ] ) => {{
        let mut __current_attribute_offsets: [u32; $num_bindings] = [0; $num_bindings];
        ::foldhash::HashMap::from_iter([
            $(
                (
                    $attribute,
                    {
                        let __format = ::vulkano::format::Format::$format;
                        let __current_offset = __current_attribute_offsets[$binding];
                        let __block_size: u32 = __format.block_size().try_into().unwrap();
                        __current_attribute_offsets[$binding] += __block_size;
                        VulkanVertexInputAttributeDescription {
                            binding: $binding,
                            format: __format,
                            offset: __current_offset,
                            ..Default::default()
                        }
                    }
                )
            ),*
        ])
        .into()
    }};
}

// TODO: Send a pull request to vulkano about Buffer::new_slice. I think it's pretty confusing that
// it creates a buffer with `len` bytes, rather than `size_of::<T> * len` bytes.

/// Creates a new uninitialized buffer for a slice.
/// Contains enough space to store `len` items of `T`.
pub fn vulkan_new_buffer_slice<T: vulkano::buffer::BufferContents>(
    allocator: &std::sync::Arc<dyn VulkanMemoryAllocator>,
    create_info: &VulkanBufferCreateInfo,
    allocation_info: &VulkanAllocationCreateInfo,
    num_items: usize,
) -> Result<VulkanSubbuffer<[T]>, VulkanValidated<vulkano::buffer::AllocateBufferError>> {
    let num_items_device_size: VulkanDeviceSize = num_items.try_into().unwrap();
    let t_device_size: VulkanDeviceSize = std::mem::size_of::<T>().try_into().unwrap();
    VulkanBuffer::new_slice(
        allocator,
        create_info,
        allocation_info,
        t_device_size * num_items_device_size,
    )
}

/// Creates a new uninitialized buffer for a slice.
/// Contains enough space to store `len` items of `T`.
///
/// Always creates a dedicated device allocation, and so is designed for long-lived large
/// allocations (>=256MiB).
pub fn vulkan_new_buffer_slice_large<T: vulkano::buffer::BufferContents>(
    device: &std::sync::Arc<VulkanDevice>,
    usage: VulkanBufferUsage,
    sharing: VulkanSharing,
    num_items: usize,
) -> Result<VulkanSubbuffer<[T]>, VulkanValidated<VulkanError>> {
    let num_items_device_size: VulkanDeviceSize = num_items.try_into().unwrap();
    let t_device_size: VulkanDeviceSize = std::mem::size_of::<T>().try_into().unwrap();
    let byte_size = num_items_device_size * t_device_size;
    let create_info = VulkanBufferCreateInfo {
        usage,
        size: byte_size,
        sharing,
        ..Default::default()
    };
    let raw_buffer = VulkanRawBuffer::new(device, &create_info)?;
    let memory_requirements = device.buffer_memory_requirements(&create_info)?;
    let allowed_memory_type_bits = memory_requirements.memory_type_bits;
    assert!(
        allowed_memory_type_bits != 0,
        "No suitable memory found for large buffer"
    );
    let memory_type_index = allowed_memory_type_bits.trailing_zeros();
    let memory_allocation = VulkanDeviceMemory::allocate(
        device,
        &VulkanMemoryAllocateInfo {
            allocation_size: byte_size,
            memory_type_index,
            dedicated_allocation: Some(VulkanDedicatedAllocation::Buffer(&raw_buffer)),
            ..Default::default()
        },
    )?;
    let resource_memory = VulkanResourceMemory::new_dedicated(memory_allocation);
    let buffer = raw_buffer
        .bind_memory(resource_memory)
        .map_err(|(err, _raw_buf, _resource_mem)| err)?;
    let subbuffer = VulkanSubbuffer::new(std::sync::Arc::new(buffer));
    Ok(subbuffer.reinterpret())
}
