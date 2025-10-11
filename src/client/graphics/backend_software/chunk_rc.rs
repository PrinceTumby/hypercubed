pub use super::chunk::SubchunkConnectivity;
use bitfield::bitfield;
use nalgebra::Rotation3;
use resources::block::RightAngleRotation;

pub struct Subchunk {
    pub start_coords: [i32; 3],
    // /// Equal to `u32::MAX` if the direction group contains no instances.
    // pub block_face_start_vertices: [u32; 6],
    // /// Equal to `(0, 0)` if the direction group contains no instances.
    // pub block_face_instance_groups: [(u32, u32); 6],
    // /// Equal to `u32::MAX` if the direction group contains no instances.
    // pub tinted_block_face_start_vertices: [u32; 6],
    // /// Equal to `(0, 0)` if the direction group contains no instances.
    // pub tinted_block_face_instance_groups: [(u32, u32); 6],
    // pub custom_block_groups: Vec<CustomBlockGroup>,
    pub connected_faces: SubchunkConnectivity,
}

pub mod block_face {
    use super::*;

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

        pub mod indices {
            pub const TOP: u8 = 0;
            pub const BOTTOM: u8 = 1;
            pub const NORTH: u8 = 2;
            pub const SOUTH: u8 = 3;
            pub const EAST: u8 = 4;
            pub const WEST: u8 = 5;
        }
    }

    #[derive(Copy, Clone, Debug)]
    pub struct Vertex {
        pub subchunk_start_coords: [f32; 3],
        pub face_matrix_index: u32,
    }

    #[derive(Copy, Clone, Debug)]
    pub struct Instance {
        pub uvs: [u16; 4],
        pub packed_fields: InstanceFields,
    }

    bitfield! {
        // 0-3: X offset
        // 4-7: Y offset
        // 8-11: Z offset
        // 12-13: UV rotation
        // 14: Emits light?
        // 15-19: Unused
        // 20-23: Sky light level
        // 24-27: Block light level
        // 28-31: Unused
        #[repr(transparent)]
        #[derive(Clone, Copy)]
        pub struct InstanceFields(u32);
        impl Debug;
        pub x_offset, set_x_offset: 3, 0;
        pub y_offset, set_y_offset: 7, 4;
        pub z_offset, set_z_offset: 11, 8;
        pub uv_rotation, set_uv_rotation: 13, 12;
        pub emits_light, set_emits_light: 14;
        pub sky_light_level, set_sky_light_level: 23, 20;
        pub block_light_level, set_block_light_level: 27, 24;
    }

    impl Instance {
        pub fn new(
            subchunk_xyz: [u8; 3],
            uvs: [u16; 4],
            uv_rotation: RightAngleRotation,
            light_levels: [u8; 2],
            emits_light: bool,
        ) -> Self {
            debug_assert!(subchunk_xyz[0] < 16);
            debug_assert!(subchunk_xyz[1] < 16);
            debug_assert!(subchunk_xyz[2] < 16);
            debug_assert!(light_levels[0] < 16);
            debug_assert!(light_levels[1] < 16);
            let packed_uv_rotation = match uv_rotation {
                RightAngleRotation::Zero => 0,
                RightAngleRotation::Ninety => 1,
                RightAngleRotation::OneEighty => 2,
                RightAngleRotation::TwoSeventy => 3,
            };
            let mut packed_fields = InstanceFields(0);
            packed_fields.set_x_offset(subchunk_xyz[0] as u32);
            packed_fields.set_y_offset(subchunk_xyz[1] as u32);
            packed_fields.set_z_offset(subchunk_xyz[2] as u32);
            packed_fields.set_uv_rotation(packed_uv_rotation as u32);
            packed_fields.set_emits_light(emits_light);
            packed_fields.set_sky_light_level(light_levels[0] as u32);
            packed_fields.set_block_light_level(light_levels[1] as u32);
            Self { uvs, packed_fields }
        }
    }
}

pub mod tinted_block_face {
    use super::*;

    pub use super::block_face::Vertex;

    #[derive(Copy, Clone, Debug)]
    pub struct Instance {
        pub uvs: [u16; 4],
        pub tint_color: [u8; 4],
        pub packed_fields: InstanceFields,
    }

    pub use super::block_face::InstanceFields;

    impl Instance {
        pub fn new(
            subchunk_xyz: [u8; 3],
            uvs: [u16; 4],
            uv_rotation: RightAngleRotation,
            light_levels: [u8; 2],
            tint_color: [u8; 4],
            emits_light: bool,
        ) -> Self {
            debug_assert!(subchunk_xyz[0] < 16);
            debug_assert!(subchunk_xyz[1] < 16);
            debug_assert!(subchunk_xyz[2] < 16);
            debug_assert!(light_levels[0] < 16);
            debug_assert!(light_levels[1] < 16);
            let packed_uv_rotation = match uv_rotation {
                RightAngleRotation::Zero => 0,
                RightAngleRotation::Ninety => 1,
                RightAngleRotation::OneEighty => 2,
                RightAngleRotation::TwoSeventy => 3,
            };
            let mut packed_fields = InstanceFields(0);
            packed_fields.set_x_offset(subchunk_xyz[0] as u32);
            packed_fields.set_y_offset(subchunk_xyz[1] as u32);
            packed_fields.set_z_offset(subchunk_xyz[2] as u32);
            packed_fields.set_uv_rotation(packed_uv_rotation as u32);
            packed_fields.set_emits_light(emits_light);
            packed_fields.set_sky_light_level(light_levels[0] as u32);
            packed_fields.set_block_light_level(light_levels[1] as u32);
            Self {
                uvs,
                tint_color,
                packed_fields,
            }
        }
    }
}

pub mod custom_block {
    use super::*;

    #[derive(Copy, Clone, Debug)]
    pub struct Vertex {
        pub pos: [f32; 3],
        pub uvs: [u16; 2],
        pub normal: [f32; 3],
        pub packed_fields: VertexFields,
    }

    impl Vertex {
        pub fn new(pos: [f32; 3], uvs: [u16; 2], normal: [f32; 3], is_tinted: bool) -> Self {
            let mut packed_fields = VertexFields(0);
            packed_fields.set_is_tinted(is_tinted);
            Self {
                pos,
                uvs,
                normal,
                packed_fields,
            }
        }
    }

    bitfield! {
        // 0: Tinted?
        // 1-31: Unused
        #[repr(transparent)]
        #[derive(Clone, Copy)]
        pub struct VertexFields(u32);
        impl Debug;
        pub is_tinted, set_is_tinted: 0;
    }

    #[derive(Copy, Clone, Debug)]
    pub struct Instance {
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
        pub packed_fields: InstanceFields,
    }

    bitfield! {
        // 0: Emits light?
        // 1-7: Unused
        #[repr(transparent)]
        #[derive(Clone, Copy)]
        pub struct InstanceFields(u8);
        impl Debug;
        pub emits_light, set_emits_light: 0;
    }

    impl Instance {
        pub fn new(
            pos: [f32; 3],
            tint_color: [u8; 4],
            centre_light_levels: [u8; 2],
            neighbour_light_levels: [[u8; 2]; 6],
            emits_light: bool,
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
            let mut packed_fields = InstanceFields(0);
            packed_fields.set_emits_light(emits_light);
            Self {
                pos,
                tint_color,
                light_level_pairs: converted_light_level_pairs,
                packed_fields,
            }
        }
    }
}
