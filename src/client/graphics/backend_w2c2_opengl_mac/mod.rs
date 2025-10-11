pub mod chunk;
pub mod debug;
pub mod egui_renderer;
pub mod gl;

use crate::platform::libs::winit;
use crate::portable_prelude::*;
use debug::line::Instance as DebugLineInstance;
use debug::point::Vertex as DebugPointVertex;
use debug::triangle::Instance as DebugTriangleInstance;
use nalgebra::{Perspective3, Point3};
use portable_std::{Arc, FastHashMap, FastHashSet};
use resources::block::model::ModelRegistry;
use resources::texture::RawAtlas;

pub use super::Camera;

#[derive(Clone)]
pub struct GraphicsResources {
    pub block_registry: Arc<resources::block::Registry>,
    pub model_registry: Arc<ModelRegistry>,
    pub atlas: Arc<RawAtlas>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GraphicsOptions {
    pub vsync: bool,
}

impl Default for GraphicsOptions {
    fn default() -> Self {
        Self { vsync: true }
    }
}

pub struct GraphicsState {
    pub resources: GraphicsResources,
    pub graphics_options: GraphicsOptions,
    pub egui_renderer: egui_renderer::Renderer,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub camera: Camera,
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
    pub radiance_cascades_ray_visualiser: bool,
    pub radiance_cascades_light_tree_visualiser: bool,
    pub radiance_cascades_light_tree_level: usize,
    pub radiance_cascades_areaquad_visualiser: bool,
    pub max_radiance_cascade: u32,
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
            radiance_cascades_ray_visualiser: false,
            radiance_cascades_light_tree_visualiser: false,
            radiance_cascades_light_tree_level: 0,
            radiance_cascades_areaquad_visualiser: false,
            max_radiance_cascade: 0,
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

#[derive(Default)]
pub struct DebugOutput {
    pub subchunks_culled: usize,
    pub subchunk_traversal_graph: Vec<([i32; 3], [i32; 3])>,
}

impl GraphicsState {
    pub fn new(width: u32, height: u32) -> anyhow::Result<Self> {
        let graphics_options = GraphicsOptions::default();
        let size = winit::dpi::PhysicalSize { width, height };
        let embedded_cache = crate::platform::get_embedded_cache();
        let camera = Camera {
            pos: Point3::new(0.0, 124.0, 0.0),
            proj_matrix: Perspective3::new(
                (size.width as f32) / (size.height as f32),
                f32::to_radians(super::DEFAULT_FOV),
                super::DEFAULT_ZNEAR,
                super::DEFAULT_ZFAR,
            ),
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
        };
        unsafe {
            gl::clear_color(0.471, 0.655, 1.0, 1.0);
            gl::viewport(0, 0, width.try_into().unwrap(), height.try_into().unwrap());
            gl::enable(gl::EnableComponent::DepthTest);
            gl::pixel_store_i32(gl::PixelStoreParam::UnpackAlignment, 1);
        }
        let egui_renderer = egui_renderer::Renderer::new();
        Ok(Self {
            resources: GraphicsResources {
                block_registry: Arc::new(embedded_cache.block_registry),
                model_registry: Arc::new(embedded_cache.models),
                atlas: Arc::new(embedded_cache.atlas),
            },
            graphics_options,
            egui_renderer,
            size,
            camera,
        })
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        println!("GraphicsState::resize({new_size:?})");
        unsafe {
            self.size = new_size;
            gl::viewport(
                0,
                0,
                new_size.width.try_into().unwrap(),
                new_size.height.try_into().unwrap(),
            );
            self.camera
                .proj_matrix
                .set_aspect((new_size.width as f32) / (new_size.height as f32));
        }
    }

    pub fn apply_new_graphics_options(&mut self, _new_options: GraphicsOptions) {
        todo!("GraphicsState::apply_new_graphics_options")
        // let old_options = std::mem::replace(&mut self.graphics_options, new_options);
        // if new_options.vsync != old_options.vsync {
        //     self.pixels.enable_vsync(new_options.vsync);
        // }
    }

    pub fn free_subchunk_data(&mut self, _subchunk_coords: [i32; 3]) {
        todo!("GraphicsState::free_subchunk_data")
    }

    // pub fn render(
    //     &mut self,
    //     _subchunks: &FastHashMap<[i32; 3], chunk::Subchunk>,
    //     _loaded_chunks: &FastHashSet<[i32; 2]>,
    //     _debug_state: &DebugState,
    // ) -> anyhow::Result<DebugOutput> {
    //     static TEST_RECTANGLE_VERTICES: [[gl::HostF32; 3]; 4] = to_host_f32_2d_array([
    //         [-0.5, 123.0, -1.0],
    //         [0.5, 123.0, -1.0],
    //         [-0.5, 125.0, -1.0],
    //         [0.5, 125.0, -1.0],
    //     ]);
    //     static TEST_RECTANGLE_COLORS: [[gl::HostF32; 3]; 4] = to_host_f32_2d_array([
    //         [1.0, 0.0, 0.0],
    //         [0.0, 1.0, 0.0],
    //         [0.0, 0.0, 1.0],
    //         [1.0, 0.0, 1.0],
    //     ]);
    //     static TEST_RECTANGLE_UV_COORDS: [[gl::HostF32; 2]; 4] = to_host_f32_2d_array([
    //         [0.0, 1.0],
    //         [1.0, 1.0],
    //         [0.0, 0.0],
    //         [1.0, 0.0],
    //     ]);
    //     unsafe {
    //         gl::clear(gl::ClearBufferBits::COLOR | gl::ClearBufferBits::DEPTH);
    //         gl::enable(gl::EnableComponent::Texture2d);
    //         gl::client_active_texture(0);
    //         gl::tex_env_mode(gl::TexEnvTarget::TextureEnv, gl::TexEnvMode::Modulate);
    //         gl::tex_image_2d(
    //             gl::Texture2dTarget::Texture,
    //             0,
    //             gl::TextureInternalFormat::Rgba,
    //             self.resources.atlas.width as usize,
    //             self.resources.atlas.height as usize,
    //             0,
    //             gl::Texture2dFormat::Rgba,
    //             gl::TextureDataType::U8,
    //             self.resources.atlas.texture_bytes.as_ptr(),
    //         );
    //         gl::vertex_pointer(
    //             3,
    //             gl::VertexPointerType::F32,
    //             0,
    //             &raw const TEST_RECTANGLE_VERTICES as *const u8,
    //         );
    //         gl::color_pointer(
    //             3,
    //             gl::ColorPointerType::F32,
    //             0,
    //             &raw const TEST_RECTANGLE_COLORS as *const u8,
    //         );
    //         gl::texture_coord_pointer(
    //             2,
    //             gl::TextureCoordPointerType::F32,
    //             0,
    //             &raw const TEST_RECTANGLE_UV_COORDS as *const u8,
    //         );
    //         gl::enable_client_state(gl::ClientArrayType::VertexArray);
    //         gl::enable_client_state(gl::ClientArrayType::ColorArray);
    //         gl::enable_client_state(gl::ClientArrayType::TextureCoordArray);
    //         gl::draw_arrays(gl::ShapeMode::TriangleStrip, 0, TEST_RECTANGLE_VERTICES.len());
    //         gl::disable(gl::EnableComponent::Texture2d);
    //     }
    //     // TODO:
    //     Ok(DebugOutput::default())
    // }

    pub fn render(
        &mut self,
        _subchunks: &FastHashMap<[i32; 3], chunk::Subchunk>,
        _loaded_chunks: &FastHashSet<[i32; 2]>,
        egui_ctx: &egui::Context,
        egui_full_output: egui::output::FullOutput,
        _debug_state: &DebugState,
        _debug_points: &[DebugPointVertex],
        _debug_lines: &[DebugLineInstance],
        _debug_triangles: &[DebugTriangleInstance],
    ) -> anyhow::Result<DebugOutput> {
        let pixels_per_point = egui_full_output.pixels_per_point;
        let egui_primitives = egui_ctx.tessellate(egui_full_output.shapes, pixels_per_point);
        unsafe {
            // Clear framebuffer and depth buffer
            gl::clear(gl::ClearBufferBits::COLOR | gl::ClearBufferBits::DEPTH);
            // Load and enable texture atlas
            gl::enable(gl::EnableComponent::Texture2d);
            gl::client_active_texture(0);
            gl::tex_wrap_s(gl::TexTarget::Texture2d, gl::TexWrapMode::Repeat);
            gl::tex_wrap_t(gl::TexTarget::Texture2d, gl::TexWrapMode::Repeat);
            gl::tex_mag_filter(gl::TexTarget::Texture2d, gl::TexFilterMode::Nearest);
            gl::tex_min_filter(gl::TexTarget::Texture2d, gl::TexFilterMode::Nearest);
            gl::tex_env_mode(gl::TexEnvTarget::TextureEnv, gl::TexEnvMode::Modulate);
            gl::tex_image_2d(
                gl::Texture2dTarget::Texture,
                0,
                gl::TextureInternalFormat::Rgba,
                self.resources.atlas.width as usize,
                self.resources.atlas.height as usize,
                0,
                gl::Texture2dFormat::Rgba,
                gl::TextureDataType::U8,
                self.resources.atlas.texture_bytes.as_ptr(),
            );
            // Load camera projection matrix
            gl::matrix_mode(gl::MatrixMode::Projection);
            gl::load_matrix_f32(&self.camera.generate_view_matrix_slice());
            // Render subchunks
            {
                // TODO:
            }
            // Render debug graphics
            {
                // TODO:
            }
            // Render egui UI
            self.egui_renderer.render(
                &self.size,
                egui_full_output.textures_delta.set,
                egui_primitives,
                pixels_per_point,
            );
            // Cleanup
            gl::disable(gl::EnableComponent::Texture2d);
        }
        // TODO:
        Ok(DebugOutput::default())
    }

    #[cfg(false)]
    pub fn render(
        &mut self,
        subchunks: &AHashMap<[i32; 3], chunk_rc::Subchunk>,
        loaded_chunks: &AHashSet<[i32; 2]>,
        egui_ctx: &egui::Context,
        egui_full_output: egui::output::FullOutput,
        debug_state: &DebugState,
        debug_points: &[DebugPointVertex],
        debug_lines: &[DebugLineInstance],
        debug_triangles: &[DebugTriangleInstance],
    ) -> anyhow::Result<DebugOutput> {
        let pixels_per_point = egui_full_output.pixels_per_point;
        let egui_primitives = egui_ctx.tessellate(egui_full_output.shapes, pixels_per_point);
        let egui_render_data = self
            .egui_renderer
            .prepare(
                &self.resources,
                &mut command_buffer,
                &self.size,
                egui_full_output.textures_delta.set,
                egui_primitives,
                pixels_per_point,
            )
            .context("Error while preparing egui renderer")?;
        // Main render subpass
        // Block rendering
        let subchunks_skipped;
        let mut subchunk_traversal_graph: Vec<([i32; 3], [i32; 3])> = Vec::new();
        {
            let camera_clipping_planes = self.camera.generate_clipping_planes();
            // let camera_clipping_planes = debug_state.cull_camera.generate_clipping_planes();
            let mut rendered_chunks: AHashSet<[i32; 3]> = AHashSet::new();
            let mut visited_chunks: AHashSet<[i32; 3]> = AHashSet::new();
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
            let camera_chunk_coords = {
                // let camera_pos = debug_state.cull_camera.pos;
                let camera_pos = self.camera.pos;
                let camera_x = (camera_pos.x.floor() as i32).div_euclid(SUBCHUNK_AXIS_LEN_I32);
                let camera_y = (camera_pos.y.floor() as i32 - MIN_HEIGHT_I32)
                    .div_euclid(SUBCHUNK_AXIS_LEN_I32);
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
                camera_chunk_coords
            };
            let mut num_subchunks_rendered = 0;
            let mut block_face_draw_commands: Vec<VulkanDrawIndirectCommand> = Vec::new();
            let mut tinted_block_face_draw_commands: Vec<VulkanDrawIndirectCommand> = Vec::new();
            let mut custom_block_draw_commands: Vec<VulkanDrawIndexedIndirectCommand> = Vec::new();
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
                    let (cur_x_flip, cur_y_flip, cur_z_flip) =
                        if debug_state.cave_cull_check_unflipped {
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
                    let facing_dir = self.camera.get_rot().transform_vector(&-Vector3::z());
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
                        if debug_state.cave_cull_check_not_backwards
                            && neighbour_back_travel_amount >= 1.1
                        {
                            continue;
                        }
                        if let Some(from_dir) = from_dir {
                            // Check we can go to the neighbour from the last subchunk through this
                            // subchunk
                            if debug_state.cave_cull_check_connectivity
                                && let Some(subchunk) = subchunk_maybe
                                && !subchunk.connected_faces.connects(&from_dir, &to_dir)
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
                for i in 0..6 {
                    let skip_face_dir = match i {
                        0 => chunk_coords[1] > camera_chunk_coords[1],
                        1 => chunk_coords[1] < camera_chunk_coords[1],
                        2 => chunk_coords[2] < camera_chunk_coords[2],
                        3 => chunk_coords[2] > camera_chunk_coords[2],
                        4 => chunk_coords[0] > camera_chunk_coords[0],
                        5 => chunk_coords[0] < camera_chunk_coords[0],
                        6.. => unreachable!(),
                    };
                    if skip_face_dir {
                        continue;
                    }
                    // Base block faces
                    if subchunk.block_face_start_vertices[i] != u32::MAX {
                        block_face_draw_commands.push(VulkanDrawIndirectCommand {
                            vertex_count: 4,
                            instance_count: subchunk.block_face_instance_groups[i].1,
                            first_vertex: subchunk.block_face_start_vertices[i],
                            first_instance: subchunk.block_face_instance_groups[i].0,
                        });
                    }
                    // Tinted block faces
                    if subchunk.tinted_block_face_start_vertices[i] != u32::MAX {
                        tinted_block_face_draw_commands.push(VulkanDrawIndirectCommand {
                            vertex_count: 4,
                            instance_count: subchunk.tinted_block_face_instance_groups[i].1,
                            first_vertex: subchunk.tinted_block_face_start_vertices[i],
                            first_instance: subchunk.tinted_block_face_instance_groups[i].0,
                        });
                    }
                }
                // Custom blocks
                for group in &subchunk.custom_block_groups {
                    custom_block_draw_commands.push(VulkanDrawIndexedIndirectCommand {
                        index_count: group.start_index_and_len[1],
                        instance_count: group.start_instance_and_len[1],
                        first_index: group.start_index_and_len[0],
                        vertex_offset: group.start_vertex,
                        first_instance: group.start_instance_and_len[0],
                    });
                }
            }
            {
                let subchunk_coord_set: AHashSet<_> = subchunks.keys().copied().collect();
                subchunks_skipped = subchunk_coord_set.difference(&rendered_chunks).count()
            }
            if !block_face_draw_commands.is_empty() {
                block_face_draw_commands_buffer = Some(
                    VulkanBuffer::from_iter(
                        &self.resources.memory_allocator,
                        &VulkanBufferCreateInfo {
                            usage: VulkanBufferUsage::INDIRECT_BUFFER,
                            ..Default::default()
                        },
                        &VulkanAllocationCreateInfo {
                            memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                                | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                            ..Default::default()
                        },
                        block_face_draw_commands,
                    )
                    .context("Error while creating block face draw commands buffer")?,
                );
            }
            if !tinted_block_face_draw_commands.is_empty() {
                tinted_block_face_draw_commands_buffer = Some(
                    VulkanBuffer::from_iter(
                        &self.resources.memory_allocator,
                        &VulkanBufferCreateInfo {
                            usage: VulkanBufferUsage::INDIRECT_BUFFER,
                            ..Default::default()
                        },
                        &VulkanAllocationCreateInfo {
                            memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                                | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                            ..Default::default()
                        },
                        tinted_block_face_draw_commands,
                    )
                    .context("Error while creating tinted block face draw commands buffer")?,
                );
            }
            if !custom_block_draw_commands.is_empty() {
                custom_block_draw_commands_buffer = Some(
                    VulkanBuffer::from_iter(
                        &self.resources.memory_allocator,
                        &VulkanBufferCreateInfo {
                            usage: VulkanBufferUsage::INDIRECT_BUFFER,
                            ..Default::default()
                        },
                        &VulkanAllocationCreateInfo {
                            memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                                | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                            ..Default::default()
                        },
                        custom_block_draw_commands,
                    )
                    .context("Error while creating custom block draw commands buffer")?,
                );
            }
        }
        // let lightmap_buffers = self.buffer_managers.block_face_instance.get_lightmap_buffers();
        let lightmap_buffer = self
            .buffer_managers
            .block_face_instance
            .get_lightmap_render_buffer();
        let lightmap_render_descriptor_set = VulkanDescriptorSet::new(
            self.resources.descriptor_set_allocator.clone(),
            self.radiance_cascades
                .lightmap_render_descriptor_set_layout
                .clone(),
            [VulkanWriteDescriptorSet::buffer(0, lightmap_buffer.clone())],
            [],
        )
        .context("Error while creating matrices descriptor set")?;
        // Render blocks
        command_buffer
            // Need to bind a pipeline with compatible layout for binding descriptor sets
            .bind_pipeline_graphics(self.block_graphics_pipeline.clone())
            .unwrap()
            .bind_descriptor_sets(
                VulkanPipelineBindPoint::Graphics,
                self.generic_block_graphics_pipeline_layout.clone(),
                0,
                (
                    self.camera_descriptor_set.clone(),
                    self.block_item_atlas_descriptor_set.clone(),
                    self.matrices_descriptor_set.clone(),
                    lightmap_render_descriptor_set,
                ),
            )
            .unwrap()
            .set_viewport(
                0,
                SmallVec::from(&[VulkanViewport {
                    extent: [self.size.width as f32, self.size.height as f32],
                    ..Default::default()
                }] as &[_]),
            )
            .unwrap();
        if let Some(draw_commands_buffer) = block_face_draw_commands_buffer {
            unsafe {
                // Block graphics pipeline already bound
                command_buffer
                    // .bind_descriptor_sets(
                    //     VulkanPipelineBindPoint::Graphics,
                    //     self.generic_block_graphics_pipeline_layout.clone(),
                    //     3,
                    //     vec![lightmap_render_descriptor_set],
                    // )
                    // .unwrap()
                    .bind_vertex_buffers(
                        0,
                        (
                            self.buffer_managers.block_face_vertex.get_buffer(),
                            self.buffer_managers.block_face_instance.get_buffer(),
                        ),
                    )
                    .unwrap()
                    .draw_indirect(draw_commands_buffer)
                    .unwrap();
            }
        }
        if let Some(draw_commands_buffer) = tinted_block_face_draw_commands_buffer {
            unsafe {
                command_buffer
                    .bind_pipeline_graphics(self.tinted_block_graphics_pipeline.clone())
                    .unwrap()
                    .bind_vertex_buffers(
                        0,
                        (
                            self.buffer_managers.tinted_block_face_vertex.get_buffer(),
                            self.buffer_managers.tinted_block_face_instance.get_buffer(),
                        ),
                    )
                    .unwrap()
                    .draw_indirect(draw_commands_buffer)
                    .unwrap();
            }
        }
        if let Some(draw_commands_buffer) = custom_block_draw_commands_buffer {
            unsafe {
                command_buffer
                    .bind_pipeline_graphics(self.custom_block_graphics_pipeline.clone())
                    .unwrap()
                    .bind_vertex_buffers(
                        0,
                        (
                            self.custom_block_vertices_buffer.clone(),
                            self.buffer_managers.custom_block_instance.get_buffer(),
                        ),
                    )
                    .unwrap()
                    .bind_index_buffer(self.custom_block_indices_buffer.clone())
                    .unwrap()
                    .draw_indexed_indirect(draw_commands_buffer)
                    .unwrap();
            }
        }
        // Render debug graphics
        let mut debug_point_buffer = None;
        let mut debug_line_buffer = None;
        let mut debug_triangle_buffer = None;
        if !debug_points.is_empty() {
            debug_point_buffer = Some(
                VulkanBuffer::from_iter(
                    &self.resources.memory_allocator,
                    &VulkanBufferCreateInfo {
                        usage: VulkanBufferUsage::VERTEX_BUFFER,
                        ..Default::default()
                    },
                    &VulkanAllocationCreateInfo {
                        memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                            | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                        ..Default::default()
                    },
                    debug_points.iter().copied(),
                )
                .context("Error while creating debug point vertex buffer")?,
            );
        }
        if !debug_lines.is_empty() {
            debug_line_buffer = Some(
                VulkanBuffer::from_iter(
                    &self.resources.memory_allocator,
                    &VulkanBufferCreateInfo {
                        usage: VulkanBufferUsage::VERTEX_BUFFER,
                        ..Default::default()
                    },
                    &VulkanAllocationCreateInfo {
                        memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                            | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                        ..Default::default()
                    },
                    debug_lines.iter().copied(),
                )
                .context("Error while creating debug line instance buffer")?,
            );
        }
        if !debug_triangles.is_empty() {
            debug_triangle_buffer = Some(
                VulkanBuffer::from_iter(
                    &self.resources.memory_allocator,
                    &VulkanBufferCreateInfo {
                        usage: VulkanBufferUsage::VERTEX_BUFFER,
                        ..Default::default()
                    },
                    &VulkanAllocationCreateInfo {
                        memory_type_filter: VulkanMemoryTypeFilter::PREFER_DEVICE
                            | VulkanMemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                        ..Default::default()
                    },
                    debug_triangles.iter().copied(),
                )
                .context("Error while creating debug triangle instance buffer")?,
            );
        }
        if let Some(buffer) = debug_point_buffer {
            unsafe {
                command_buffer
                    .bind_pipeline_graphics(self.debug_point_pipeline.clone())
                    .unwrap()
                    .bind_vertex_buffers(0, (buffer,))
                    .unwrap()
                    .draw(debug_points.len().try_into().unwrap(), 1, 0, 0)
                    .unwrap();
            }
        }
        if let Some(buffer) = debug_line_buffer {
            unsafe {
                command_buffer
                    .bind_pipeline_graphics(self.debug_line_pipeline.clone())
                    .unwrap()
                    .bind_vertex_buffers(0, (buffer,))
                    .unwrap()
                    .draw(2, debug_lines.len().try_into().unwrap(), 0, 0)
                    .unwrap();
            }
        }
        if let Some(buffer) = debug_triangle_buffer {
            unsafe {
                command_buffer
                    .bind_pipeline_graphics(self.debug_triangle_pipeline.clone())
                    .unwrap()
                    .bind_vertex_buffers(0, (buffer,))
                    .unwrap()
                    .draw(3, debug_triangles.len().try_into().unwrap(), 0, 0)
                    .unwrap();
            }
        }
        // Render egui UI
        if let Some(egui_render_data) = egui_render_data {
            self.egui_renderer
                .render(&mut command_buffer, self.size, egui_render_data);
        }
        command_buffer
            .end_render_pass(VulkanSubpassEndInfo::default())
            .unwrap();
        // Submit command buffer to GPU
        let built_command_buffer = command_buffer.build().unwrap();
        // vulkano::sync::now(self.resources.device.clone())
        //     .join(swapchain_image_future)
        //     .then_execute(self.resources.queues[0].clone(), built_command_buffer)
        //     .unwrap()
        //     .then_swapchain_present(
        //         self.resources.queues[0].clone(),
        //         VulkanSwapchainPresentInfo::swapchain_image_index(
        //             self.swapchain.clone(),
        //             swapchain_image_i,
        //         ),
        //     )
        //     .then_signal_fence_and_flush()
        //     .unwrap()
        //     .wait(None)
        //     .unwrap();
        {
            self.resources.render_queue.with(|mut queue_guard| unsafe {
                queue_guard.wait_idle().unwrap();
                let render_semaphore = VulkanSemaphore::from_pool(&self.resources.device)
                    .map(Arc::new)
                    .context("Error while creating render semaphore")
                    .unwrap();
                let finish_fence = VulkanFence::from_pool(&self.resources.device)
                    .map(Arc::new)
                    .context("Error while creating render fence")
                    .unwrap();
                queue_guard
                    .submit(
                        &[VulkanSubmitInfo {
                            command_buffers: vec![VulkanCommandBufferSubmitInfo::new(
                                built_command_buffer.clone(),
                            )],
                            wait_semaphores: vec![VulkanSemaphoreSubmitInfo::new(
                                swapchain_semaphore,
                            )],
                            signal_semaphores: vec![
                                VulkanSemaphoreSubmitInfo::new(render_semaphore.clone()),
                                // VulkanSemaphoreSubmitInfo::new(render_semaphore_2.clone()),
                            ],
                            ..Default::default()
                        }],
                        // None,
                        Some(&finish_fence),
                    )
                    .unwrap();
                queue_guard
                    .present(&VulkanPresentInfo {
                        wait_semaphores: vec![VulkanSemaphorePresentInfo::new(render_semaphore)],
                        swapchain_infos: vec![VulkanSwapchainPresentInfo::new(
                            self.swapchain.clone(),
                            swapchain_image_i,
                        )],
                        ..Default::default()
                    })
                    .unwrap()
                    .for_each(|result| _ = result.unwrap());
                finish_fence.wait(None).unwrap();
            })
        }
        self.egui_renderer
            .free_textures(&egui_full_output.textures_delta.free);
        if is_swapchain_suboptimal {
            (self.swapchain, self.swapchain_images) = self
                .swapchain
                .recreate(&self.swapchain.create_info())
                .context("Error while recreating swapchain")?;
        }
        Ok(DebugOutput {
            subchunks_culled: subchunks_skipped,
            subchunk_traversal_graph,
        })
    }
}

pub const fn to_host_f32_array<const N: usize>(input: [f32; N]) -> [gl::HostF32; N] {
    let mut out = [gl::HostF32::new(0.0); N];
    let mut i: usize = 0;
    while i < N {
        out[i] = gl::HostF32::new(input[i]);
        i += 1;
    }
    out
}

pub const fn to_host_f32_2d_array<const N: usize, const M: usize>(
    input: [[f32; N]; M],
) -> [[gl::HostF32; N]; M] {
    let mut out = [[gl::HostF32::new(0.0); N]; M];
    let mut i: usize = 0;
    while i < N {
        let mut j: usize = 0;
        while j < M {
            out[j][i] = gl::HostF32::new(input[j][i]);
            j += 1;
        }
        i += 1;
    }
    out
}
