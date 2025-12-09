use bitfield::bitfield;

bitfield! {
    // 0: Ignore depth?
    // 1-31: Unused
    #[repr(transparent)]
    #[derive(Clone, Copy, Default)]
    #[cfg_attr(not(target_arch = "spirv"), derive(bytemuck::Pod, bytemuck::Zeroable))]
    pub struct PackedFlags(u32);
    impl Debug;
    ignore_depth_raw, set_ignore_depth_raw: 0;
}

impl PackedFlags {
    pub const NONE: Self = Self(0);
    pub const IGNORE_DEPTH: Self = Self(1);

    pub fn new(ignore_depth: bool) -> Self {
        let mut fields = Self::NONE;
        fields.set_ignore_depth_raw(ignore_depth);
        fields
    }

    pub fn ignore_depth(&self) -> bool {
        self.0 & 0x1 != 0
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
#[cfg_attr(not(target_arch = "spirv"), derive(bytemuck::Pod, bytemuck::Zeroable))]
pub struct PointVertex {
    pub pos: [f32; 3],
    pub color: [u8; 4],
    pub size: f32,
    pub flags: PackedFlags,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
#[cfg_attr(not(target_arch = "spirv"), derive(bytemuck::Pod, bytemuck::Zeroable))]
pub struct LineInstance {
    pub p1: [f32; 3],
    pub p2: [f32; 3],
    pub color: [u8; 4],
    pub size: f32,
    pub flags: PackedFlags,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
#[cfg_attr(not(target_arch = "spirv"), derive(bytemuck::Pod, bytemuck::Zeroable))]
pub struct TriangleInstance {
    pub p1: [f32; 3],
    pub p2: [f32; 3],
    pub p3: [f32; 3],
    pub color: [u8; 4],
    pub flags: PackedFlags,
}

#[cfg(feature = "vulkano")]
pub mod vertex_input_state {
    use super::*;
    use vulkan_prelude::*;

    pub fn point() -> VulkanVertexInputState {
        VulkanVertexInputState {
            bindings: vulkan_vertex_bindings![
                0 => (PointVertex, Vertex),
            ],
            attributes: vulkan_vertex_attributes!(1, [
                // pos
                [0 <- 0] => R32G32B32_SFLOAT,
                // color
                [1 <- 0] => R8G8B8A8_UNORM,
                // size
                [2 <- 0] => R32_SFLOAT,
                // flags
                [3 <- 0] => R32_UINT,
            ]),
            ..Default::default()
        }
    }

    pub fn line() -> VulkanVertexInputState {
        VulkanVertexInputState {
            bindings: vulkan_vertex_bindings![
                0 => (LineInstance, Instance),
            ],
            attributes: vulkan_vertex_attributes!(1, [
                // p1
                [0 <- 0] => R32G32B32_SFLOAT,
                // p2
                [1 <- 0] => R32G32B32_SFLOAT,
                // color
                [2 <- 0] => R8G8B8A8_UNORM,
                // size
                [3 <- 0] => R32_SFLOAT,
                // flags
                [4 <- 0] => R32_UINT,
            ]),
            ..Default::default()
        }
    }

    pub fn triangle() -> VulkanVertexInputState {
        VulkanVertexInputState {
            bindings: vulkan_vertex_bindings![
                0 => (TriangleInstance, Instance),
            ],
            attributes: vulkan_vertex_attributes!(1, [
                // p1
                [0 <- 0] => R32G32B32_SFLOAT,
                // p2
                [1 <- 0] => R32G32B32_SFLOAT,
                // p3
                [2 <- 0] => R32G32B32_SFLOAT,
                // color
                [3 <- 0] => R8G8B8A8_UNORM,
                // flags
                [4 <- 0] => R32_UINT,
            ]),
            ..Default::default()
        }
    }
}
