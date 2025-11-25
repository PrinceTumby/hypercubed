use super::gl;
use crate::basic_types::AxisDirection;
use nalgebra::{Matrix4, Point3, Rotation3, Vector3, Vector4};
use resources::block::RightAngleRotation;
use resources::block::model::Tint;

pub struct Subchunk {
    pub start_coords: [i32; 3],
    /// Will be `None` if the subchunk contains no faces.
    pub buffer: Option<gl::buffer::batch_collected::Buffer>,
    /// Equal to `u32::MAX` if the direction group contains no faces.
    /// The seventh group is always rendered, containing faces not in a direction group.
    pub group_start_vertices: [u32; 7],
    pub group_vertex_counts: [u32; 7],
    pub connected_faces: SubchunkConnectivity,
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

#[inline]
pub fn face_matrix_rotations() -> [Rotation3<f32>; 6] {
    [
        // Top
        Rotation3::identity(),
        // Bottom
        Rotation3::from_euler_angles(core::f32::consts::PI, 0.0, 0.0),
        // North
        Rotation3::from_euler_angles(-core::f32::consts::FRAC_PI_2, 0.0, core::f32::consts::PI),
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

pub fn generate_subchunk_matrix(subchunk_start_coords: [i32; 3]) -> [[f32; 4]; 4] {
    let subchunk_start_coords = subchunk_start_coords.map(|n| n as f32);
    // TODO: Apply a translation to the middle of the subchunk, so we can get more range out of an
    //       i16.
    Matrix4::identity()
        .append_scaling(1.0 / 1024.0)
        .append_translation(&Vector3::from(subchunk_start_coords))
        .into()
}

#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct BlockVertex {
    /// Local subchunk position, multiplied by 1024, rounded to an [`i16`].
    pub subchunk_fixed_point_pos: [i16; 3],
    // FIXME: This should be `i16`.
    pub uvs: [u16; 2],
    pub colour_rgba: [u8; 4],
}

#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(transparent)]
pub struct BlockFace(pub [BlockVertex; 6]);

impl BlockFace {
    pub fn new_basic(
        subchunk_xyz: [u8; 3],
        uvs: [u16; 4],
        uv_rotation: RightAngleRotation,
        light_levels: [u8; 2],
        face_i: usize,
    ) -> Self {
        debug_assert!(subchunk_xyz[0] < 16);
        debug_assert!(subchunk_xyz[1] < 16);
        debug_assert!(subchunk_xyz[2] < 16);
        debug_assert!(light_levels[0] < 16);
        debug_assert!(light_levels[1] < 16);
        debug_assert!(face_i < 6);
        // Calculate the vertex subchunk positions.
        let base_positions: [Vector3<f32>; 4] = [
            Vector3::new(-0.5, 0.5, 0.5),
            Vector3::new(0.5, 0.5, 0.5),
            Vector3::new(-0.5, 0.5, -0.5),
            Vector3::new(0.5, 0.5, -0.5),
        ];
        let face_matrix = face_matrix_rotations()[face_i];
        let subchunk_xyz_vec3 = Vector3::from(subchunk_xyz.map(|n| n as f32));
        let subchunk_fixed_point_positions: [[i16; 3]; 4] = base_positions
            // Rotate by the face matrix.
            .map(|face_local_pos| face_matrix.transform_vector(&face_local_pos))
            // Add the XYZ offset of the block within the subchunk, as well as the 0.5 offset.
            .map(|block_local_pos| (block_local_pos + subchunk_xyz_vec3).add_scalar(0.5))
            // Scale...
            .map(|subchunk_local_pos| subchunk_local_pos * 1024.0)
            // ...and convert to a fixed-point `i16`.
            .map(|scaled_pos| <[f32; 3]>::from(scaled_pos).map(|n| n.round() as i16));
        // Calculate the vertex UVs.
        let base_uvs = [
            // NOTE: This flips the UV coordinates, which seems to be needed?
            [uvs[0], uvs[3]],
            [uvs[2], uvs[3]],
            [uvs[0], uvs[1]],
            [uvs[2], uvs[1]],
        ];
        let rotated_uv_indices = match uv_rotation {
            RightAngleRotation::Zero => [0, 1, 2, 3],
            RightAngleRotation::Ninety => [1, 3, 0, 2],
            RightAngleRotation::OneEighty => [3, 2, 1, 0],
            RightAngleRotation::TwoSeventy => [2, 0, 3, 1],
        };
        let rotated_uvs = rotated_uv_indices.map(|i| base_uvs[i]);
        // Calculate block lighting.
        // TODO: Per-face directional lighting.
        let block_light_rgb_vec3 = Self::calculate_light_rgb_vec3(light_levels);
        // Calculate per-face directional lighting.
        let face_normal = face_matrix.transform_vector(&Vector3::y());
        let dir_light_coef = Self::calculate_dir_light_coef(face_normal);
        // Calculate final face colour.
        let colour_rgb_vec3 = block_light_rgb_vec3 * dir_light_coef;
        let colour_rgba_vec4 = colour_rgb_vec3.push(1.0);
        // TODO: Figure out sRGB textures, remove `fast-srgb` crate dependency.
        let colour_rgba: [u8; 4] = colour_rgba_vec4.map(|n| (n * 255.0).round() as u8).into();
        // Assemble the 4 vertices making up a face.
        let face_vertices: [BlockVertex; 4] = core::array::from_fn(|i| BlockVertex {
            subchunk_fixed_point_pos: subchunk_fixed_point_positions[i],
            uvs: rotated_uvs[i],
            colour_rgba,
        });
        // Return face as 2 triangles.
        Self([0, 1, 2, 1, 3, 2].map(|i| face_vertices[i]))
    }

    pub fn new_tinted(
        subchunk_xyz: [u8; 3],
        uvs: [u16; 4],
        uv_rotation: RightAngleRotation,
        light_levels: [u8; 2],
        tint_rgba: [u8; 4],
        face_i: usize,
    ) -> Self {
        debug_assert!(subchunk_xyz[0] < 16);
        debug_assert!(subchunk_xyz[1] < 16);
        debug_assert!(subchunk_xyz[2] < 16);
        debug_assert!(light_levels[0] < 16);
        debug_assert!(light_levels[1] < 16);
        debug_assert!(face_i < 6);
        // Calculate the vertex subchunk positions.
        let base_positions: [Vector3<f32>; 4] = [
            Vector3::new(-0.5, 0.5, 0.5),
            Vector3::new(0.5, 0.5, 0.5),
            Vector3::new(-0.5, 0.5, -0.5),
            Vector3::new(0.5, 0.5, -0.5),
        ];
        let face_matrix = face_matrix_rotations()[face_i];
        let subchunk_xyz_vec3 = Vector3::from(subchunk_xyz.map(|n| n as f32));
        let subchunk_fixed_point_positions: [[i16; 3]; 4] = base_positions
            // Rotate by the face matrix.
            .map(|face_local_pos| face_matrix.transform_vector(&face_local_pos))
            // Add the XYZ offset of the block within the subchunk, as well as the 0.5 offset.
            .map(|block_local_pos| (block_local_pos + subchunk_xyz_vec3).add_scalar(0.5))
            // Scale...
            .map(|subchunk_local_pos| subchunk_local_pos * 1024.0)
            // ...and convert to a fixed-point `i16`.
            .map(|scaled_pos| <[f32; 3]>::from(scaled_pos).map(|n| n.round() as i16));
        // Calculate the vertex UVs.
        let base_uvs = [
            // NOTE: This flips the UV coordinates, which seems to be needed?
            [uvs[0], uvs[3]],
            [uvs[2], uvs[3]],
            [uvs[0], uvs[1]],
            [uvs[2], uvs[1]],
        ];
        let rotated_uv_indices = match uv_rotation {
            RightAngleRotation::Zero => [0, 1, 2, 3],
            RightAngleRotation::Ninety => [1, 3, 0, 2],
            RightAngleRotation::OneEighty => [3, 2, 1, 0],
            RightAngleRotation::TwoSeventy => [2, 0, 3, 1],
        };
        let rotated_uvs = rotated_uv_indices.map(|i| base_uvs[i]);
        // Calculate the face RGBA colour (duplicated for all vertices).
        // For a tinted face, this is based on the light levels, and the tint colour.
        // TODO: Per-face directional lighting.
        let light_rgb_vec3 = Self::calculate_light_rgb_vec3(light_levels);
        let light_rgba_vec4 = light_rgb_vec3.push(1.0);
        let tint_rgba_vec4 = Vector4::from(tint_rgba.map(|x| x as f32 / 255.0));
        let colour_rgba_vec4 = light_rgba_vec4.component_mul(&tint_rgba_vec4);
        let colour_rgba: [u8; 4] = colour_rgba_vec4.map(|n| (n * 255.0).round() as u8).into();
        // Assemble the 4 vertices making up a face.
        let face_vertices: [BlockVertex; 4] = core::array::from_fn(|i| BlockVertex {
            subchunk_fixed_point_pos: subchunk_fixed_point_positions[i],
            uvs: rotated_uvs[i],
            colour_rgba,
        });
        // Return face as 2 triangles.
        Self([0, 1, 2, 1, 3, 2].map(|i| face_vertices[i]))
    }

    pub fn new_custom(
        model_face: &[resources::block::model::ModelVertex; 4],
        subchunk_xyz: [u8; 3],
        tint_rgba: [u8; 4],
        centre_light_levels: [u8; 2],
        neighbour_light_levels: [[u8; 2]; 6],
    ) -> Self {
        let subchunk_xyz_vec3 = Vector3::from(subchunk_xyz.map(|n| n as f32));
        let tint_rgba_vec4 = Vector4::from(tint_rgba.map(|n| n as f32 / 255.0));
        let face_vertices = model_face.map(|v| {
            // Calculate the vertex subchunk fixed-point position.
            let subchunk_pos = v.local_pos + subchunk_xyz_vec3 + Vector3::repeat(0.5);
            let subchunk_fixed_point_pos: [i16; 3] =
                (subchunk_pos * 1024.0).map(|n| n.round() as i16).into();
            // The tint colour will be multiplied into the final colour RGBA, so just make it one
            // if the vertex isn't tinted.
            let applied_tint_rgba_vec4 = match v.tint {
                None => Vector4::repeat(1.0),
                Some(Tint::Biome) => tint_rgba_vec4,
            };
            // Calculate block lighting.
            let block_light_rgb_vec3 = {
                // Try to find the block light levels that are "most applicable" for the current
                // vertex. The light levels sampled are the levels for the block itself, followed
                // by all six neighbours.
                let adjusted_pos = v.local_pos + (v.normal * 0.02);
                let light_levels = [
                    centre_light_levels,
                    neighbour_light_levels[0],
                    neighbour_light_levels[1],
                    neighbour_light_levels[2],
                    neighbour_light_levels[3],
                    neighbour_light_levels[4],
                    neighbour_light_levels[5],
                ];
                let light_positions = [
                    Point3::new(0.0, 0.0, 0.0),
                    Point3::new(0.0, 1.01, 0.0),
                    Point3::new(0.0, -1.01, 0.0),
                    Point3::new(0.0, 0.0, -1.01),
                    Point3::new(0.0, 0.0, 1.01),
                    Point3::new(0.0, 1.01, 0.0),
                    Point3::new(0.0, -1.01, 0.0),
                ];
                let (closest_light_i, _closest_light_dist) =
                    light_positions.iter().enumerate().fold(
                        (0, f32::INFINITY),
                        |(current_closest_i, current_closest_dist), (i, pos)| {
                            let dist = (pos - adjusted_pos).magnitude_squared();
                            if dist < current_closest_dist {
                                (i, dist)
                            } else {
                                (current_closest_i, current_closest_dist)
                            }
                        },
                    );
                let closest_light_levels = light_levels[closest_light_i];
                Self::calculate_light_rgb_vec3(closest_light_levels)
            };
            // Calculate per-face directional lighting.
            let dir_light_coef = Self::calculate_dir_light_coef(v.normal);
            // Combine the block lighting with the per-face directional lighting, expand to RGBA.
            let light_rgb_vec3 = block_light_rgb_vec3 * dir_light_coef;
            let light_rgba_vec4 = light_rgb_vec3.push(1.0);
            // Final vertex colour is the tint colour combined with lighting.
            let colour_rgba_vec4 = applied_tint_rgba_vec4.component_mul(&light_rgba_vec4);
            let colour_rgba: [u8; 4] = colour_rgba_vec4.map(|n| (n * 255.0).round() as u8).into();
            BlockVertex {
                subchunk_fixed_point_pos,
                uvs: v.uvs,
                colour_rgba,
            }
        });
        // Return face as 2 triangles.
        Self([1, 0, 2, 1, 2, 3].map(|i| face_vertices[i]))
    }

    pub(super) fn calculate_light_rgb_vec3(light_levels: [u8; 2]) -> Vector3<f32> {
        debug_assert!(light_levels[0] < 16);
        debug_assert!(light_levels[1] < 16);
        let [sky_light_level, block_light_level] = light_levels;
        let light_percentage = f32::clamp(
            // FIXME: I think sky light and block light have different maxima?
            f32::max(sky_light_level as f32, block_light_level as f32) / 14.0,
            0.001,
            1.0,
        );
        let gamma = 0.5;
        let light_gamma = light_percentage.powf(1.0 / gamma);
        Vector3::repeat(0.02).lerp(&Vector3::repeat(1.0), light_gamma)
    }

    fn calculate_dir_light_coef(normal: Vector3<f32>) -> f32 {
        let light_source_dir = Vector3::new(2.0, 5.0, 1.0).normalize();
        let dir_lighting = Vector3::dot(&normal, &light_source_dir);
        f32::mul_add(dir_lighting, 0.3, 0.7)
    }
}
