use nalgebra::{Point3, Rotation3, Vector3};
use crate::block::RightAngleRotation;
#[cfg(not(feature = "std"))]
use nalgebra::ComplexField;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[derive(serde::Serialize, serde::Deserialize)]
#[derive(bincode::Encode, bincode::Decode)]
pub struct AABB {
    #[bincode(with_serde)]
    pub corner_1: Point3<f32>,
    #[bincode(with_serde)]
    pub corner_2: Point3<f32>,
}

impl AABB {
    pub const fn empty_at_origin() -> Self {
        Self {
            corner_1: Point3::new(0.0, 0.0, 0.0),
            corner_2: Point3::new(0.0, 0.0, 0.0),
        }
    }

    /// Computes the smallest AABB that contains both `self` and `other`.
    pub fn max(&self, other: &Self) -> Self {
        Self {
            corner_1: Point3::new(
                f32::min(self.corner_1.x, other.corner_1.x),
                f32::min(self.corner_1.y, other.corner_1.y),
                f32::min(self.corner_1.z, other.corner_1.z),
            ),
            corner_2: Point3::new(
                f32::max(self.corner_2.x, other.corner_2.x),
                f32::max(self.corner_2.y, other.corner_2.y),
                f32::max(self.corner_2.z, other.corner_2.z),
            ),
        }
    }

    pub fn extended_in_direction(&self, dir: Vector3<f32>) -> Self {
        let mut out = *self;
        if dir.x < 0.0 {
            out.corner_1.x += dir.x;
        } else {
            out.corner_2.x += dir.x;
        }
        if dir.y < 0.0 {
            out.corner_1.y += dir.y;
        } else {
            out.corner_2.y += dir.y;
        }
        if dir.z < 0.0 {
            out.corner_1.z += dir.z;
        } else {
            out.corner_2.z += dir.z;
        }
        out
    }

    pub fn expanded_by(&self, dims: Vector3<f32>) -> Self {
        Self {
            corner_1: self.corner_1 - dims,
            corner_2: self.corner_2 + dims,
        }
    }

    pub fn contracted_by(&self, dims: Vector3<f32>) -> Self {
        Self {
            corner_1: self.corner_1 + dims,
            corner_2: self.corner_2 - dims,
        }
    }

    pub fn intersects(&self, other: &Self) -> bool {
        self.corner_1.x < other.corner_2.x
            && self.corner_2.x > other.corner_1.x
            && self.corner_1.y < other.corner_2.y
            && self.corner_2.y > other.corner_1.y
            && self.corner_1.z < other.corner_2.z
            && self.corner_2.z > other.corner_1.z
    }

    pub fn intersects_sphere(&self, centre: Point3<f32>, radius: f32) -> bool {
        let mut distance = 0.0;
        if centre.x < self.corner_1.x {
            distance += (centre.x - self.corner_1.x).powi(2);
        } else if centre.x > self.corner_2.x {
            distance += (centre.x - self.corner_2.x).powi(2);
        }
        if centre.y < self.corner_1.y {
            distance += (centre.y - self.corner_1.y).powi(2);
        } else if centre.y > self.corner_2.y {
            distance += (centre.y - self.corner_2.y).powi(2);
        }
        if centre.z < self.corner_1.z {
            distance += (centre.z - self.corner_1.z).powi(2);
        } else if centre.z > self.corner_2.z {
            distance += (centre.z - self.corner_2.z).powi(2);
        }
        distance <= radius.powi(2)
    }

    pub fn apply_blockstate_rotations(
        &mut self,
        x_rotation: RightAngleRotation,
        y_rotation: RightAngleRotation,
    ) {
        let x_y_axis_angles = [
            (Vector3::x_axis(), x_rotation),
            (Vector3::y_axis(), y_rotation),
        ];
        let [x_blockstate_rot, y_blockstate_rot] =
            x_y_axis_angles.map(|(axis, angle)| match angle {
                RightAngleRotation::Zero => Rotation3::identity(),
                RightAngleRotation::Ninety => {
                    Rotation3::from_axis_angle(&axis, -core::f32::consts::FRAC_PI_2)
                }
                RightAngleRotation::OneEighty => {
                    Rotation3::from_axis_angle(&axis, core::f32::consts::PI)
                }
                RightAngleRotation::TwoSeventy => {
                    Rotation3::from_axis_angle(&axis, core::f32::consts::FRAC_PI_2)
                }
            });
        let xy_blockstate_rot = (y_blockstate_rot * x_blockstate_rot).to_homogeneous();
        let complete_mat = xy_blockstate_rot
            .prepend_translation(&Vector3::new(-0.5, -0.5, -0.5))
            .append_translation(&Vector3::new(0.5, 0.5, 0.5));
        let [p1, p2] = [self.corner_1, self.corner_2].map(|p| complete_mat.transform_point(&p));
        fn quantise_256(n: f32) -> f32 {
            (n * 256.0).round() / 256.0
        }
        self.corner_1 = Point3::new(
            quantise_256(f32::min(p1.x, p2.x)),
            quantise_256(f32::min(p1.y, p2.y)),
            quantise_256(f32::min(p1.z, p2.z)),
        );
        self.corner_2 = Point3::new(
            quantise_256(f32::max(p1.x, p2.x)),
            quantise_256(f32::max(p1.y, p2.y)),
            quantise_256(f32::max(p1.z, p2.z)),
        );
    }

    /// Returns the penetration normal and entry time for `self` moving at a given velocity
    /// interacting with `other`, a static collider.
    /// Returns `None` if no collision occurs in the next time frame.
    pub fn get_collision_info(
        &self,
        other: &Self,
        velocity: Vector3<f32>,
    ) -> Option<(Vector3<f32>, f32)> {
        fn entry_exit_times(s1: f32, s2: f32, o1: f32, o2: f32, velocity: f32) -> [f32; 2] {
            if velocity > 0.0 {
                [(o1 - s2) / velocity, (o2 - s1) / velocity]
            } else if velocity == 0.0 {
                [
                    f32::copysign(f32::INFINITY, s1 - o2),
                    f32::copysign(f32::INFINITY, s2 - o1),
                ]
            } else {
                [(o2 - s1) / velocity, (o1 - s2) / velocity]
            }
        }
        let [x_entry, x_exit] = entry_exit_times(
            self.corner_1.x,
            self.corner_2.x,
            other.corner_1.x,
            other.corner_2.x,
            velocity.x,
        );
        let [y_entry, y_exit] = entry_exit_times(
            self.corner_1.y,
            self.corner_2.y,
            other.corner_1.y,
            other.corner_2.y,
            velocity.y,
        );
        let [z_entry, z_exit] = entry_exit_times(
            self.corner_1.z,
            self.corner_2.z,
            other.corner_1.z,
            other.corner_2.z,
            velocity.z,
        );
        // Check if collision occured
        if x_entry < 0.0 && y_entry < 0.0 && z_entry < 0.0 {
            return None;
        }
        if x_entry > 1.0 || y_entry > 1.0 || z_entry > 1.0 {
            return None;
        }
        // Get first collision axis
        let entry = [x_entry, y_entry, z_entry]
            .into_iter()
            .max_by(|a, b| a.total_cmp(b))
            .unwrap();
        let exit = [x_exit, y_exit, z_exit]
            .into_iter()
            .min_by(|a, b| a.total_cmp(b))
            .unwrap();
        if entry > exit {
            return None;
        }
        // Get normal of collision surface
        let normal = if entry == x_entry {
            Vector3::new((1.0_f32).copysign(-velocity.x), 0.0, 0.0)
        } else if entry == y_entry {
            Vector3::new(0.0, (1.0_f32).copysign(-velocity.y), 0.0)
        } else {
            Vector3::new(0.0, 0.0, (1.0_f32).copysign(-velocity.z))
        };
        Some((normal, entry))
    }

    pub fn compute_x_offset(&self, other: &Self, initial_value: f32) -> f32 {
        // Ensure that `self` and `other` intersect in Y and Z axes.
        if other.corner_2.y < self.corner_1.y
            || other.corner_1.y > self.corner_2.y
            || other.corner_2.z < self.corner_1.z
            || other.corner_1.z > self.corner_2.z
        {
            return initial_value;
        }
        if initial_value > 0.0 && other.corner_2.x <= self.corner_1.x {
            initial_value.min(self.corner_1.x - other.corner_2.x)
        } else if initial_value < 0.0 && other.corner_1.x >= self.corner_2.x {
            initial_value.max(self.corner_2.x - other.corner_1.x)
        } else {
            initial_value
        }
    }

    pub fn compute_y_offset(&self, other: &Self, initial_value: f32) -> f32 {
        // Ensure that `self` and `other` intersect in X and Z axes.
        if other.corner_2.x < self.corner_1.x
            || other.corner_1.x > self.corner_2.x
            || other.corner_2.z < self.corner_1.z
            || other.corner_1.z > self.corner_2.z
        {
            return initial_value;
        }
        if initial_value > 0.0 && other.corner_2.y <= self.corner_1.y {
            initial_value.min(self.corner_1.y - other.corner_2.y)
        } else if initial_value < 0.0 && other.corner_1.y >= self.corner_2.y {
            initial_value.max(self.corner_2.y - other.corner_1.y)
        } else {
            initial_value
        }
    }

    pub fn compute_z_offset(&self, other: &Self, initial_value: f32) -> f32 {
        // Ensure that `self` and `other` intersect in X and Y axes.
        if other.corner_2.x < self.corner_1.x
            || other.corner_1.x > self.corner_2.x
            || other.corner_2.y < self.corner_1.y
            || other.corner_1.y > self.corner_2.y
        {
            return initial_value;
        }
        if initial_value > 0.0 && other.corner_2.z <= self.corner_1.z {
            initial_value.min(self.corner_1.z - other.corner_2.z)
        } else if initial_value < 0.0 && other.corner_1.z >= self.corner_2.z {
            initial_value.max(self.corner_2.z - other.corner_1.z)
        } else {
            initial_value
        }
    }
}

impl core::ops::Add<Vector3<f32>> for AABB {
    type Output = Self;

    fn add(self, offset: Vector3<f32>) -> Self {
        Self {
            corner_1: self.corner_1 + offset,
            corner_2: self.corner_2 + offset,
        }
    }
}

impl core::ops::AddAssign<Vector3<f32>> for AABB {
    fn add_assign(&mut self, offset: Vector3<f32>) {
        self.corner_1 += offset;
        self.corner_2 += offset;
    }
}