pub mod chunk;
pub mod debug;
pub mod environment;
pub mod lightmap;

// Backends
#[cfg(feature = "graphics_backend_opengl")]
pub mod backend_opengl;
#[cfg(feature = "graphics_backend_vulkan")]
pub mod backend_vulkan;
#[cfg(feature = "graphics_backend_wgpu")]
pub mod backend_wgpu;

// Check we've enabled at least one graphics backend supported by the current platform.
#[cfg(any(
    all(
        feature = "platform_winit",
        not(any(
            feature = "graphics_backend_opengl",
            feature = "graphics_backend_vulkan",
            feature = "graphics_backend_wgpu",
        )),
    ),
    all(
        feature = "platform_linux_drm",
        not(feature = "graphics_backend_opengl"),
    ),
))]
compile_error!("At least one graphics backend feature must be enabled.");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(feature = "platform_winit", feature = "platform_linux_drm"),
    derive(clap::ValueEnum)
)]
pub enum SelectedGraphicsBackend {
    #[cfg(feature = "graphics_backend_opengl")]
    #[cfg_attr(
        any(feature = "platform_winit", feature = "platform_linux_drm"),
        value(name = "opengl")
    )]
    OpenGL,
    #[cfg(feature = "graphics_backend_vulkan")]
    #[cfg_attr(
        any(feature = "platform_winit", feature = "platform_linux_drm"),
        value(name = "vulkan")
    )]
    Vulkan,
    #[cfg(feature = "graphics_backend_wgpu")]
    #[cfg_attr(
        any(feature = "platform_winit", feature = "platform_linux_drm"),
        value(name = "wgpu")
    )]
    Wgpu,
}

#[allow(clippy::derivable_impls)]
impl Default for SelectedGraphicsBackend {
    fn default() -> Self {
        cfg_select! {
            feature = "graphics_backend_vulkan" => {
                Self::Vulkan
            }
            feature = "graphics_backend_opengl" => {
                Self::OpenGL
            }
            feature = "graphics_backend_wgpu" => {
                Self::Wgpu
            }
        }
    }
}

impl core::fmt::Display for SelectedGraphicsBackend {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let name = match *self {
            #[cfg(feature = "graphics_backend_opengl")]
            Self::OpenGL => "OpenGL",
            #[cfg(feature = "graphics_backend_vulkan")]
            Self::Vulkan => "Vulkan",
            #[cfg(feature = "graphics_backend_wgpu")]
            Self::Wgpu => "wgpu",
        };
        write!(f, "{name}")
    }
}

use chunk::{HasSubchunkData, SubchunkData};
use nalgebra::{Isometry3, Matrix4, Perspective3, Point3, UnitQuaternion, Vector3};
use portable_std::{Arc, FastHashMap, FastHashSet, VecDeque};
use threadpool::ThreadPool;
use winit::event_loop::OwnedDisplayHandle;
use winit::window::Window;

use crate::basic_types::AxisDirection;
use crate::platform::libs::winit;
use crate::{ClientPlayState, MIN_HEIGHT_I32, SUBCHUNK_AXIS_LEN_I32};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GraphicsOptions {
    pub vsync: bool,
    pub lightmap_gamma_setting: f32,
}

impl Default for GraphicsOptions {
    fn default() -> Self {
        Self {
            vsync: true,
            lightmap_gamma_setting: 0.5,
        }
    }
}

pub trait GraphicsBackend {
    fn new(
        window: Arc<Window>,
        display: OwnedDisplayHandle,
        game_data: resources::GameResourceData,
    ) -> anyhow::Result<Box<Self>>
    where
        Self: Sized;

    fn get_block_registry(&self) -> &resources::block::Registry;

    fn get_subchunks_data(&self) -> FastHashMap<[i32; 3], SubchunkData>;

    fn get_size(&self) -> winit::dpi::PhysicalSize<u32>;

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>);

    fn get_graphics_options(&self) -> GraphicsOptions;

    fn apply_new_graphics_options(&mut self, new_options: GraphicsOptions);

    fn dispatch_subchunk_updates(
        &mut self,
        thread_pool: &ThreadPool,
        raw_chunks: Arc<FastHashMap<[i32; 2], Arc<crate::RawChunk>>>,
        subchunks: FastHashSet<[i32; 3]>,
    );

    fn remove_chunk(&mut self, chunk_coords: [i32; 2]);

    #[allow(clippy::too_many_arguments)]
    fn render(
        &mut self,
        play_state: &ClientPlayState,
        current_time_s: f64,
        egui_ctx: &egui::Context,
        egui_full_output: egui::output::FullOutput,
        debug_state: &DebugState,
        debug_points: &[debug::Point],
        debug_lines: &[debug::Line],
        debug_triangles: &[debug::Triangle],
    ) -> anyhow::Result<Option<DebugOutput>>;

    // Miscellaneous methods, not required to be implemented.

    fn wants_egui_debug_section(&self) -> bool {
        false
    }

    fn render_egui_debug_section(&mut self, ctx: &mut egui::Ui) {
        _ = ctx;
    }

    /// Allows a graphics backend to override the camera's near plane distance.
    /// Useful for graphics backends without floating point depth buffers, or other methods to
    /// improve depth precision at far distances.
    fn get_camera_znear_override(&self) -> Option<f32> {
        None
    }
}

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
    pub fn dummy() -> Self {
        Self {
            pos: Point3::origin(),
            proj_matrix: Perspective3::new(
                800.0 / 600.0,
                f32::to_radians(DEFAULT_FOV),
                DEFAULT_ZNEAR,
                DEFAULT_ZFAR,
            ),
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
        }
    }

    pub fn get_zfar(&self) -> f32 {
        DEFAULT_ZFAR
    }

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
        self.generate_view_matrix()
            .append_nonuniform_scaling(&Vector3::new(1.0, 1.0, -0.5))
            .append_translation(&Vector3::new(0.0, 0.0, 0.5))
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

#[derive(Default)]
pub struct DebugOutput {
    pub subchunks_culled: usize,
    pub subchunk_traversal_graph: Vec<([i32; 3], [i32; 3])>,
}

#[derive(Clone, Copy, Debug)]
pub struct DebugState {
    pub visualisation_draw_method: DebugVisualisationDrawMethod,
    pub cull_planes_active: usize,
    pub rendering_view_frustum: bool,
    pub free_cam: bool,
    pub cave_cull_check_unflipped: bool,
    pub cave_cull_check_not_backwards: bool,
    pub cave_cull_check_frustum: bool,
    pub cave_cull_check_connectivity: bool,
    pub cave_cull_render_connectivity: bool,
    pub cave_cull_render_traversal_graph: bool,
    pub cave_cull_debug_render_dist: f32,
    pub max_render_chunks: usize,
    pub debug_texture_zoom: f32,
}

impl Default for DebugState {
    fn default() -> Self {
        Self {
            visualisation_draw_method: DebugVisualisationDrawMethod::default(),
            cull_planes_active: 6,
            rendering_view_frustum: false,
            free_cam: false,
            cave_cull_check_unflipped: true,
            cave_cull_check_not_backwards: false,
            cave_cull_check_frustum: true,
            cave_cull_check_connectivity: true,
            cave_cull_render_connectivity: false,
            cave_cull_render_traversal_graph: false,
            cave_cull_debug_render_dist: 24.0,
            max_render_chunks: 3000,
            debug_texture_zoom: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DebugVisualisationDrawMethod {
    #[default]
    Egui,
    Gpu,
}

impl DebugVisualisationDrawMethod {
    pub fn label_text(&self) -> &'static str {
        match *self {
            Self::Egui => "Use egui for debug visualisation",
            Self::Gpu => "Use the GPU directly for debug visualisation",
        }
    }
}

#[tracing::instrument(skip_all)]
pub fn for_each_visible_subchunk<'a, F, S>(
    camera: &Camera,
    subchunks: &'a FastHashMap<[i32; 3], S>,
    loaded_chunks: &FastHashSet<[i32; 2]>,
    debug_state: &DebugState,
    mut subchunk_fn: F,
) -> DebugOutput
where
    F: FnMut([i32; 3], &'a S),
    S: HasSubchunkData,
{
    // Find all chunks that are actually visible (have all of their neighbours), to prevent
    // visibility "leakage" at the edge of the loaded area.
    let visible_chunks = {
        let mut visible_chunks = FastHashSet::with_capacity(loaded_chunks.len());
        for &[chunk_x, chunk_z] in loaded_chunks {
            if loaded_chunks.contains(&[chunk_x - 1, chunk_z])
                && loaded_chunks.contains(&[chunk_x + 1, chunk_z])
                && loaded_chunks.contains(&[chunk_x, chunk_z - 1])
                && loaded_chunks.contains(&[chunk_x, chunk_z + 1])
            {
                visible_chunks.insert([chunk_x, chunk_z]);
            }
        }
        visible_chunks
    };
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
                if !visible_chunks.contains(&[neighbour_coord[0], neighbour_coord[2]]) {
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
                    if debug_state.cave_cull_check_connectivity
                        && let Some(subchunk) = subchunk_maybe
                        && !subchunk
                            .get_data()
                            .connectivity
                            .connects(&from_dir, &to_dir)
                    {
                        continue;
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
