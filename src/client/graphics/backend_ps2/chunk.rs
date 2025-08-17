use crate::basic_types::AxisDirection;
use resources::block::RightAngleRotation;
use bitfield::bitfield;
use nalgebra::Rotation3;

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

// #[repr(C)]
// #[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
// pub struct CustomBlockGroup {
//     pub start_vertex: u32,
//     pub start_index_and_len: [u32; 2],
//     pub start_instance_and_len: [u32; 2],
// }

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

impl core::fmt::Debug for SubchunkConnectivity {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
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
    
    impl Vertex {
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
