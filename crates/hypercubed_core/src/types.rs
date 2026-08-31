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

/// A floating-point value in the range `0.0..=1.0`, representing a percentage value from 0% to
/// 100%.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct PercentageF32(f32);

impl core::ops::Mul for PercentageF32 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        Self((self.0 * rhs.0).clamp(0.0, 1.0))
    }
}

impl core::ops::Mul<PercentageF32> for f32 {
    type Output = f32;

    #[inline]
    fn mul(self, rhs: PercentageF32) -> Self::Output {
        self * rhs.0
    }
}

impl core::ops::Mul<PercentageF32> for u32 {
    type Output = u32;

    #[inline]
    fn mul(self, rhs: PercentageF32) -> Self::Output {
        (self as f32 * rhs.0).round() as Self::Output
    }
}

impl PercentageF32 {
    #[inline]
    pub const fn from_f32_0_1_clamped(x: f32) -> Self {
        Self(x.clamp(0.0, 1.0))
    }

    #[inline]
    pub const fn from_unorm8(x: u8) -> Self {
        // Technique taken from <https://fgiesen.wordpress.com/2024/11/06/exact-unorm8-to-float>.
        const K0: u16 = 3;
        const K1: f32 = 1.0 / (255.0 * 3.0);
        Self((x as u16 * K0) as f32 * K1)
    }

    #[inline]
    pub const fn as_f32(&self) -> f32 {
        self.0
    }

    #[inline]
    pub const fn to_f32(self) -> f32 {
        self.0
    }
}
