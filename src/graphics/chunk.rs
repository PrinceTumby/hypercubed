use crate::basic_types::AxisDirection;

pub trait HasSubchunkData {
    fn get_data(&self) -> SubchunkData;
}

#[derive(Clone, Copy, Debug)]
pub struct SubchunkData {
    pub start_coords: [i32; 3],
    pub connectivity: SubchunkConnectivity,
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
