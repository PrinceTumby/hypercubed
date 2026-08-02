use bitfield::bitfield;

/// Stores a set of [`[u16; 2]`] UVs as a single [`u32`].
/// Avoids needing SPIR-V "Int16" capability.
#[repr(transparent)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuUv(u32);

impl GpuUv {
    pub fn new(uvs: [u16; 2]) -> Self {
        Self(uvs[0] as u32 | ((uvs[1] as u32) << 16))
    }

    pub fn get(&self) -> [u32; 2] {
        [self.0 & 0xFFFF, (self.0 >> 16) & 0xFFFF]
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SubchunkFaceGroupInfo {
    pub buffer_address: u64,
    pub subchunk_start_coords: [i32; 3],
    pub face_matrix_index: u32,
}

// Block face

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BlockFaceVertex {
    pub subchunk_start_coords: [f32; 3],
    pub face_matrix_index: u32,
}

impl BlockFaceVertex {
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
pub struct BlockFaceInstance {
    pub uvs: [u16; 4],
    pub packed_fields: BlockFaceInstanceFields,
}

impl BlockFaceInstance {
    /// `packed_uv_rotation` is valid within 0..=3, specifies a rotation in increments of 90
    /// degrees.
    pub fn new(
        subchunk_xyz: [u8; 3],
        uvs: [u16; 4],
        packed_uv_rotation: u8,
        light_levels: [u8; 2],
    ) -> Self {
        debug_assert!(subchunk_xyz[0] < 16);
        debug_assert!(subchunk_xyz[1] < 16);
        debug_assert!(subchunk_xyz[2] < 16);
        debug_assert!(light_levels[0] < 16);
        debug_assert!(light_levels[1] < 16);
        assert!(packed_uv_rotation < 4);
        let mut packed_fields = BlockFaceInstanceFields(0);
        packed_fields.set_x_offset(subchunk_xyz[0] as u32);
        packed_fields.set_y_offset(subchunk_xyz[1] as u32);
        packed_fields.set_z_offset(subchunk_xyz[2] as u32);
        packed_fields.set_uv_rotation(packed_uv_rotation as u32);
        packed_fields.set_sky_light_level(light_levels[0] as u32);
        packed_fields.set_block_light_level(light_levels[1] as u32);
        Self { uvs, packed_fields }
    }
}

bitfield! {
    // 0-3: X offset
    // 4-7: Y offset
    // 8-11: Z offset
    // 12-13: UV rotation
    // 14-19: Unused
    // 20-23: Sky light level
    // 24-27: Block light level
    // 28-31: Unused
    #[repr(transparent)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct BlockFaceInstanceFields(u32);
    impl Debug;
    pub x_offset, set_x_offset: 3, 0;
    pub y_offset, set_y_offset: 7, 4;
    pub z_offset, set_z_offset: 11, 8;
    pub uv_rotation, set_uv_rotation: 13, 12;
    pub sky_light_level, set_sky_light_level: 23, 20;
    pub block_light_level, set_block_light_level: 27, 24;
}

// Tinted block face

pub use BlockFaceVertex as TintedBlockFaceVertex;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TintedBlockFaceInstance {
    pub uvs: [u16; 4],
    pub tint_color: [u8; 4],
    pub packed_fields: BlockFaceInstanceFields,
}

impl TintedBlockFaceInstance {
    /// `packed_uv_rotation` is valid within 0..=3, specifies a rotation in increments of 90
    /// degrees.
    pub fn new(
        subchunk_xyz: [u8; 3],
        uvs: [u16; 4],
        packed_uv_rotation: u8,
        light_levels: [u8; 2],
        tint_color: [u8; 4],
    ) -> Self {
        debug_assert!(subchunk_xyz[0] < 16);
        debug_assert!(subchunk_xyz[1] < 16);
        debug_assert!(subchunk_xyz[2] < 16);
        debug_assert!(light_levels[0] < 16);
        debug_assert!(light_levels[1] < 16);
        assert!(packed_uv_rotation < 4);
        let mut packed_fields = BlockFaceInstanceFields(0);
        packed_fields.set_x_offset(subchunk_xyz[0] as u32);
        packed_fields.set_y_offset(subchunk_xyz[1] as u32);
        packed_fields.set_z_offset(subchunk_xyz[2] as u32);
        packed_fields.set_uv_rotation(packed_uv_rotation as u32);
        packed_fields.set_sky_light_level(light_levels[0] as u32);
        packed_fields.set_block_light_level(light_levels[1] as u32);
        Self {
            uvs,
            tint_color,
            packed_fields,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CustomBlockVertex {
    pub pos: [f32; 3],
    pub uv: GpuUv,
    pub normal: [f32; 3],
    pub packed_fields: CustomBlockVertexFields,
}

impl CustomBlockVertex {
    pub fn new(pos: [f32; 3], uv: [u16; 2], normal: [f32; 3], is_tinted: bool) -> Self {
        let uv = GpuUv::new(uv);
        let mut packed_fields = CustomBlockVertexFields(0);
        packed_fields.set_is_tinted(is_tinted);
        Self {
            pos,
            uv,
            normal,
            packed_fields,
        }
    }
}

bitfield! {
    // 0: Tinted?
    // 1-31: Unused
    #[repr(transparent)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct CustomBlockVertexFields(u32);
    impl Debug;
    pub is_tinted, set_is_tinted: 0;
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CustomBlockInstance {
    pub pos: [f32; 3],
    pub tint_color: [u8; 4],
    /// Light levels for surrounding blocks in order:
    /// 1: Centre
    /// 2: Above
    /// 3: Below
    /// 4: North
    /// 5: South
    /// 6: East
    /// 7: West
    pub light_level_pairs: [u8; 7],
    pub packed_fields: CustomBlockInstanceFields,
}

impl CustomBlockInstance {
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
        let mut converted_light_level_pairs = [0u8; 7];
        converted_light_level_pairs[0] = centre_light_levels[0] | (centre_light_levels[1] << 4);
        for (i, pair) in neighbour_light_levels.into_iter().enumerate() {
            converted_light_level_pairs[i + 1] = pair[0] | (pair[1] << 4);
        }
        Self {
            pos,
            tint_color,
            light_level_pairs: converted_light_level_pairs,
            packed_fields: CustomBlockInstanceFields(0),
        }
    }
}

bitfield! {
    // 0-7: Unused
    #[repr(transparent)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct CustomBlockInstanceFields(u8);
    impl Debug;
}

pub mod vertex_input_state {
    use super::*;
    use vulkan_prelude::*;

    pub fn block_face() -> VulkanVertexInputState {
        VulkanVertexInputState {
            bindings: vulkan_vertex_bindings![
                0 => (BlockFaceInstance, Instance),
            ],
            attributes: vulkan_vertex_attributes!(1, [
                // uvs
                [0 <- 0] => R16G16B16A16_UINT,
                // packed_fields
                [1 <- 0] => R32_UINT,
            ]),
            ..Default::default()
        }
    }

    pub fn tinted_block_face() -> VulkanVertexInputState {
        VulkanVertexInputState {
            bindings: vulkan_vertex_bindings![
                0 => (TintedBlockFaceInstance, Instance),
            ],
            attributes: vulkan_vertex_attributes!(1, [
                // uvs
                [0 <- 0] => R16G16B16A16_UINT,
                // tint_color
                [1 <- 0] => R8G8B8A8_UNORM,
                // packed_fields
                [2 <- 0] => R32_UINT,
            ]),
            ..Default::default()
        }
    }

    pub fn custom_block() -> VulkanVertexInputState {
        VulkanVertexInputState {
            bindings: vulkan_vertex_bindings![
                // Vertex pulling is used from face data.
                0 => (CustomBlockInstance, Instance),
            ],
            attributes: vulkan_vertex_attributes!(1, [
                // pos
                [0 <- 0] => R32G32B32_SFLOAT,
                // tint_color
                [1 <- 0] => R8G8B8A8_UNORM,
                // light_level_pairs (first half)
                [2 <- 0] => R8G8B8A8_UINT,
                // light_level_pairs (second half) and packed_fields
                [3 <- 0] => R8G8B8A8_UINT,
            ]),
            ..Default::default()
        }
    }
}
