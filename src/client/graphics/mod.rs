cfg_if::cfg_if! {
    if #[cfg(feature = "graphics_backend_vulkan")] {
        mod backend_vulkan;
        pub use backend_vulkan::*;
    } else if #[cfg(feature = "graphics_backend_wgpu")] {
        mod backend_wgpu;
        pub use backend_wgpu::*;
    } else if #[cfg(feature = "graphics_backend_opengl")] {
        mod backend_opengl;
        pub use backend_opengl::*;
    } else if #[cfg(feature = "graphics_backend_software")] {
        mod backend_software;
        pub use backend_software::*;
    } else {
        compile_error!("A graphics backend feature must be enabled.");
    }
}

use crate::basic_types::AxisDirection;
use crate::client::{MIN_HEIGHT_I32, SUBCHUNK_AXIS_LEN_I32};
use nalgebra::{Isometry3, Matrix4, Perspective3, Point3, UnitQuaternion, Vector3};
use portable_std::{FastHashMap, FastHashSet};
use std::collections::VecDeque;

pub const DEFAULT_FOV: f32 = 80.0;
pub const DEFAULT_ZNEAR: f32 = 0.01;
pub const DEFAULT_ZFAR: f32 = 1024.0;

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub pos: Point3<f32>,
    pub proj_matrix: Perspective3<f32>,
    /// Represented in degrees.
    pub yaw: f32,
    /// Represented in degrees.
    pub pitch: f32,
    /// Represented in degrees.
    pub roll: f32,
}

impl Camera {
    pub fn get_rot(&self) -> UnitQuaternion<f32> {
        UnitQuaternion::from_euler_angles(
            self.pitch.to_radians(),
            -self.yaw.to_radians(),
            -self.roll.to_radians(),
        )
    }

    pub fn generate_view_matrix(&self) -> Matrix4<f32> {
        let translate = Isometry3::new(self.pos.coords, nalgebra::zero())
            .inverse()
            .to_matrix();
        let rotate = self.get_rot().inverse().to_homogeneous();
        self.proj_matrix.as_matrix() * rotate * translate
    }

    pub fn generate_view_matrix_slice(&self) -> [[f32; 4]; 4] {
        *self.generate_view_matrix().as_ref()
    }

    pub fn generate_reversed_depth_view_matrix(&self) -> Matrix4<f32> {
        // Using a standard depth buffer had issues with Z-fighting on faraway objects (snow
        // clipping through spruce leaves from high enough up was a particularly bad case).
        Matrix4::new_nonuniform_scaling(&Vector3::new(1.0, 1.0, -0.5))
            .append_translation(&Vector3::new(0.0, 0.0, 0.5))
            * self.generate_view_matrix()
    }

    pub fn generate_reversed_depth_view_matrix_slice(&self) -> [[f32; 4]; 4] {
        *self.generate_reversed_depth_view_matrix().as_ref()
    }

    pub fn generate_debug_crosshair_view_matrix_slice(&self) -> [[f32; 4]; 4] {
        let fake_pos = Point3::from(self.get_rot() * Vector3::z().scale(30.0));
        let up = self.get_rot() * Vector3::y();
        let look_at = Matrix4::look_at_rh(&fake_pos, &Point3::origin(), &up);
        let view_matrix = self.proj_matrix.as_matrix() * look_at;
        *view_matrix.as_ref()
    }

    /// Generates a normal and offset for each view clipping plane.
    /// Planes are in order of left, right, bottom, top, near, far.
    pub fn generate_clipping_planes(&self) -> [(Vector3<f32>, f32); 6] {
        /// Converts constants from a plane equation to a normal vector and offset
        fn convert_abcd(a: f32, b: f32, c: f32, d: f32) -> (Vector3<f32>, f32) {
            let normal = Vector3::new(a, b, c);
            let normal_len = normal.magnitude();
            (normal / normal_len, d / normal_len)
        }
        let m = self.generate_view_matrix();
        [
            // Left
            convert_abcd(m.m41 + m.m11, m.m42 + m.m12, m.m43 + m.m13, m.m44 + m.m14),
            // Right
            convert_abcd(m.m41 - m.m11, m.m42 - m.m12, m.m43 - m.m13, m.m44 - m.m14),
            // Bottom
            convert_abcd(m.m41 + m.m21, m.m42 + m.m22, m.m43 + m.m23, m.m44 + m.m24),
            // Top
            convert_abcd(m.m41 - m.m21, m.m42 - m.m22, m.m43 - m.m23, m.m44 - m.m24),
            // Near
            convert_abcd(m.m41 + m.m31, m.m42 + m.m32, m.m43 + m.m33, m.m44 + m.m34),
            // Far
            convert_abcd(m.m41 - m.m31, m.m42 - m.m32, m.m43 - m.m33, m.m44 - m.m34),
        ]
    }
}

#[tracing::instrument(skip_all)]
pub fn for_each_visible_subchunk<F>(
    camera: &Camera,
    subchunks: &FastHashMap<[i32; 3], chunk::Subchunk>,
    loaded_chunks: &FastHashSet<[i32; 2]>,
    debug_state: &DebugState,
    mut subchunk_fn: F,
) -> DebugOutput
where
    F: FnMut([i32; 3], &chunk::Subchunk),
{
    let mut subchunk_traversal_graph: Vec<([i32; 3], [i32; 3])> = Vec::new();
    let camera_clipping_planes = camera.generate_clipping_planes();
    // let camera_clipping_planes = debug_state.cull_camera.generate_clipping_planes();
    let mut rendered_chunks: FastHashSet<[i32; 3]> = FastHashSet::new();
    let mut visited_chunks: FastHashSet<[i32; 3]> = FastHashSet::new();
    #[derive(Clone, Copy, Debug)]
    struct QueuedChunk {
        pub coords: [i32; 3],
        pub from_dir: Option<AxisDirection>,
        pub back_travel_amount: f32,
        pub flipping_state: FlippingState,
    }
    // TODO: Come up with a better name for this, document how it works
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum FlippingState {
        Unflipped {
            x_positive: Option<bool>,
            y_positive: Option<bool>,
            z_positive: Option<bool>,
        },
        Flipped,
    }
    //let mut subchunk_graph: DiGraph<QueuedChunk, ()> = DiGraph::new();
    let mut chunk_queue: VecDeque<QueuedChunk> = VecDeque::new();
    // Start the subchunk search from the camera's subchunk.
    {
        // let camera_pos = debug_state.cull_camera.pos;
        let camera_pos = camera.pos;
        let camera_x = (camera_pos.x.floor() as i32).div_euclid(SUBCHUNK_AXIS_LEN_I32);
        let camera_y =
            (camera_pos.y.floor() as i32 - MIN_HEIGHT_I32).div_euclid(SUBCHUNK_AXIS_LEN_I32);
        let camera_z = (camera_pos.z.floor() as i32).div_euclid(SUBCHUNK_AXIS_LEN_I32);
        let camera_chunk_coords = [camera_x, camera_y, camera_z];
        chunk_queue.push_back(QueuedChunk {
            coords: camera_chunk_coords,
            from_dir: None,
            back_travel_amount: 0.0,
            flipping_state: FlippingState::Unflipped {
                x_positive: None,
                y_positive: None,
                z_positive: None,
            },
        });
        visited_chunks.insert(camera_chunk_coords);
    }
    let mut num_subchunks_rendered = 0;
    while let Some(queued_chunk) = chunk_queue.pop_front() {
        let QueuedChunk {
            coords: chunk_coords,
            from_dir,
            back_travel_amount: chunk_back_travel_amount,
            flipping_state: chunk_flip_state,
        } = queued_chunk;
        let subchunk_maybe = subchunks.get(&chunk_coords);
        // Visit neighbours
        'neighbour_blk: {
            let (cur_x_flip, cur_y_flip, cur_z_flip) = if debug_state.cave_cull_check_unflipped {
                match chunk_flip_state {
                    FlippingState::Unflipped {
                        x_positive,
                        y_positive,
                        z_positive,
                    } => (x_positive, y_positive, z_positive),
                    FlippingState::Flipped => break 'neighbour_blk,
                }
            } else {
                (None, None, None)
            };
            let [chunk_x, chunk_y, chunk_z] = chunk_coords;
            let neighbour_chunks = [
                ([chunk_x - 1, chunk_y, chunk_z], AxisDirection::West),
                ([chunk_x + 1, chunk_y, chunk_z], AxisDirection::East),
                ([chunk_x, chunk_y, chunk_z - 1], AxisDirection::North),
                ([chunk_x, chunk_y, chunk_z + 1], AxisDirection::South),
                ([chunk_x, chunk_y - 1, chunk_z], AxisDirection::Down),
                ([chunk_x, chunk_y + 1, chunk_z], AxisDirection::Up),
            ];
            // let facing_dir = debug_state
            //     .cull_camera
            //     .get_rot()
            //     .transform_vector(&-Vector3::z());
            let facing_dir = camera.get_rot().transform_vector(&-Vector3::z());
            'neighbour_loop: for (neighbour_coord, to_dir) in neighbour_chunks {
                const WORLD_HEIGHT_I32: i32 = 384;
                if neighbour_coord[1] < 0 || neighbour_coord[1] > WORLD_HEIGHT_I32 / 16 {
                    continue;
                }
                if !loaded_chunks.contains(&[neighbour_coord[0], neighbour_coord[2]]) {
                    continue;
                }
                // Check we're haven't gone backwards too much
                let back_travel_diff = -facing_dir.dot(&to_dir.as_vector());
                let neighbour_back_travel_amount =
                    (chunk_back_travel_amount + back_travel_diff).max(0.0);
                if debug_state.cave_cull_check_not_backwards && neighbour_back_travel_amount >= 1.1
                {
                    continue;
                }
                if let Some(from_dir) = from_dir {
                    // Check we can go to the neighbour from the last subchunk through this
                    // subchunk
                    if debug_state.cave_cull_check_connectivity {
                        if let Some(subchunk) = subchunk_maybe {
                            if !subchunk.connected_faces.connects(&from_dir, &to_dir) {
                                continue;
                            }
                        }
                    }
                }
                // Check neighbour lies in camera frustum
                if debug_state.cave_cull_check_frustum {
                    // use super::MIN_HEIGHT_I32;
                    let start_coords = [
                        (neighbour_coord[0] * 16) as f32,
                        (neighbour_coord[1] * 16 + MIN_HEIGHT_I32) as f32,
                        (neighbour_coord[2] * 16) as f32,
                    ];
                    let end_coords = start_coords.map(|n| n + 16.0);
                    for (i, clip_plane) in camera_clipping_planes.into_iter().enumerate() {
                        let (normal, offset) = clip_plane;
                        if i >= debug_state.cull_planes_active {
                            break;
                        }
                        let inward_point = Point3::new(
                            match normal.x > 0.0 {
                                false => start_coords[0],
                                true => end_coords[0],
                            },
                            match normal.y > 0.0 {
                                false => start_coords[1],
                                true => end_coords[1],
                            },
                            match normal.z > 0.0 {
                                false => start_coords[2],
                                true => end_coords[2],
                            },
                        );
                        if inward_point.coords.dot(&normal) + offset < 0.0 {
                            continue 'neighbour_loop;
                        }
                    }
                }
                // Check we haven't already rendered the neighbour
                if visited_chunks.contains(&neighbour_coord) {
                    continue;
                }
                // Calculate flip state for neighbour
                visited_chunks.insert(neighbour_coord);
                chunk_queue.push_back(QueuedChunk {
                    coords: neighbour_coord,
                    from_dir: Some(to_dir.invert()),
                    back_travel_amount: neighbour_back_travel_amount,
                    flipping_state: {
                        let (new_x_flip, new_y_flip, new_z_flip) = match to_dir {
                            AxisDirection::Down => (None, Some(false), None),
                            AxisDirection::Up => (None, Some(true), None),
                            AxisDirection::North => (None, None, Some(false)),
                            AxisDirection::South => (None, None, Some(true)),
                            AxisDirection::West => (Some(false), None, None),
                            AxisDirection::East => (Some(true), None, None),
                        };
                        if [
                            cur_x_flip.zip(new_x_flip),
                            cur_y_flip.zip(new_y_flip),
                            cur_z_flip.zip(new_z_flip),
                        ]
                        .iter()
                        .any(|&flips| flips.is_some_and(|(x, y)| x != y))
                        {
                            FlippingState::Flipped
                        } else {
                            FlippingState::Unflipped {
                                x_positive: new_x_flip.or(cur_x_flip),
                                y_positive: new_y_flip.or(cur_y_flip),
                                z_positive: new_z_flip.or(cur_z_flip),
                            }
                        }
                    },
                });
                subchunk_traversal_graph.push((chunk_coords, neighbour_coord));
            }
        }
        let Some(subchunk) = subchunk_maybe else {
            continue;
        };
        if num_subchunks_rendered >= debug_state.max_render_chunks {
            break;
        } else {
            num_subchunks_rendered += 1;
        }
        rendered_chunks.insert(chunk_coords);
        subchunk_fn(chunk_coords, subchunk);
    }
    DebugOutput {
        subchunks_culled: {
            let subchunk_coord_set: FastHashSet<_> = subchunks.keys().copied().collect();
            subchunk_coord_set.difference(&rendered_chunks).count()
        },
        subchunk_traversal_graph,
    }
}
