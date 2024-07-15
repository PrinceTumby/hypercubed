use nalgebra::Vector3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AxisDirection {
    Down = 0,
    Up = 1,
    North = 2,
    South = 3,
    West = 4,
    East = 5,
}

impl AxisDirection {
    pub fn invert(&self) -> Self {
        match self {
            Self::Down => Self::Up,
            Self::Up => Self::Down,
            Self::North => Self::South,
            Self::South => Self::North,
            Self::West => Self::East,
            Self::East => Self::West,
        }
    }

    pub fn as_vector(&self) -> Vector3<f32> {
        match self {
            Self::Down => -Vector3::y(),
            Self::Up => Vector3::y(),
            Self::North => -Vector3::z(),
            Self::South => Vector3::z(),
            Self::West => -Vector3::x(),
            Self::East => Vector3::x(),
        }
    }
}
