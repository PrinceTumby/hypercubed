use super::Texture;
use resources::block::RightAngleRotation;
use nalgebra::{Matrix3, Rotation3};
use std::collections::HashMap;
use std::marker::PhantomData;
use wgpu::{
    Buffer, BufferSlice, ComputePipeline, ComputePipelineDescriptor, Device,
    PipelineCompilationOptions, PipelineLayout, RenderPipeline, RenderPipelineDescriptor,
    SurfaceConfiguration, VertexAttribute, include_wgsl, vertex_attr_array,
};

pub use super::chunk::{
    BufferManager, CustomBlockGroup, DrawArgsBuffer, IndexListBuffer, Subchunk,
    SubchunkConnectivity, VertexBufferManager, VertexListBuffer,
};

// const RAY_RESULT_BYTE_SIZE: usize = 16;
// const BASE_NUM_RAYS: usize = 8;
// const PROBE_BYTE_SIZE: usize = RAY_RESULT_BYTE_SIZE * BASE_NUM_RAYS;
// const NUM_CASCADES: usize = 1;
// const SUBCHUNK_BYTE_SIZE: usize = NUM_CASCADES * PROBE_BYTE_SIZE * 16 * 16 * 16;
// const INITIAL_NUM_SUBCHUNKS: usize = 256;
const CASCADE_0_RAY_LENGTH: f64 = 1.25;

#[derive(Clone, Copy, Debug)]
struct BufferArea {
    pub usage: BufferAreaUsage,
    pub num_chunks: u64,
}

impl BufferArea {
    pub fn belongs_to(&self, subchunk_coords: [i32; 3]) -> bool {
        matches!(self.usage, BufferAreaUsage::Used(coords) if coords == subchunk_coords)
    }

    pub fn is_free(&self) -> bool {
        matches!(self.usage, BufferAreaUsage::Free)
    }
}

#[derive(Clone, Copy, Debug)]
enum BufferAreaUsage {
    Free,
    Used([i32; 3]),
}

// TODO:
// - Convert generics to `CHUNK_SIZE` and `INITIAL_NUM_CHUNKS`
// - When the buffer fills up, allocate a new buffer increased by `INITIAL_NUM_CHUNKS`
// - Copy all old buffer contents over to new buffer
// - Expand `usage_map` with the new free space
// - Retry allocation
#[derive(Debug)]
pub struct InstanceBufferManager<
    T: bytemuck::Pod,
    const ITEMS_PER_CHUNK: usize,
    const NUM_CHUNKS: usize,
> {
    instance_buffer: Buffer,
    lightmap_buffers: [Buffer; 2],
    usage_map: Vec<BufferArea>,
    phantom: PhantomData<[[T; ITEMS_PER_CHUNK]; NUM_CHUNKS]>,
}

type LightMap = [u32; 128];

impl<T: bytemuck::Pod, const ITEMS_PER_CHUNK: usize, const NUM_CHUNKS: usize>
    InstanceBufferManager<T, ITEMS_PER_CHUNK, NUM_CHUNKS>
{
    pub fn new(device: &wgpu::Device, debug_name: &str) -> Self {
        Self {
            instance_buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("{debug_name} Instance Buffer")),
                size: std::mem::size_of::<T>() as u64 * ITEMS_PER_CHUNK as u64 * NUM_CHUNKS as u64,
                usage: wgpu::BufferUsages::VERTEX
                    | wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            lightmap_buffers: [
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("{debug_name} Cascade 0 Lightmap Buffer")),
                    size: std::mem::size_of::<LightMap>() as u64
                        * ITEMS_PER_CHUNK as u64
                        * NUM_CHUNKS as u64,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("{debug_name} Cascade 1 Lightmap Buffer")),
                    size: std::mem::size_of::<LightMap>() as u64
                        * ITEMS_PER_CHUNK as u64
                        * NUM_CHUNKS as u64,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
            ],
            usage_map: vec![BufferArea {
                usage: BufferAreaUsage::Free,
                num_chunks: NUM_CHUNKS as u64,
            }],
            phantom: PhantomData,
        }
    }

    pub fn alloc_area(
        &mut self,
        queue: &wgpu::Queue,
        subchunk_coords: [i32; 3],
        items: &[T],
    ) -> u32 {
        debug_assert!(!items.is_empty());
        let items_byte_slice: &[u8] = bytemuck::cast_slice(items);
        let byte_len = items_byte_slice.len();
        let num_chunks_needed = (items.len() as u64).div_ceil(ITEMS_PER_CHUNK as u64);
        // Find free area large enough to hold items
        let mut current_start_chunk: u64 = 0;
        for i in 0..self.usage_map.len() {
            let area = &mut self.usage_map[i];
            if area.is_free() {
                use std::cmp::Ordering;
                match area.num_chunks.cmp(&num_chunks_needed) {
                    Ordering::Greater => {
                        // Split area into used portion and leftover free protion
                        let new_free_area = BufferArea {
                            usage: BufferAreaUsage::Free,
                            num_chunks: area.num_chunks - num_chunks_needed,
                        };
                        area.num_chunks = num_chunks_needed;
                        area.usage = BufferAreaUsage::Used(subchunk_coords);
                        self.usage_map.insert(i + 1, new_free_area);
                    }
                    Ordering::Equal => {
                        // Mark entire area as used
                        area.usage = BufferAreaUsage::Used(subchunk_coords);
                    }
                    Ordering::Less => {
                        current_start_chunk += area.num_chunks;
                        continue;
                    }
                }
                // Write items to buffer
                let buffer_offset =
                    std::mem::size_of::<T>() as u64 * ITEMS_PER_CHUNK as u64 * current_start_chunk;
                let mut buffer_window = queue
                    .write_buffer_with(
                        &self.instance_buffer,
                        buffer_offset,
                        (byte_len as u64).try_into().unwrap(),
                    )
                    .unwrap();
                buffer_window.copy_from_slice(items_byte_slice);
                return (buffer_offset / std::mem::size_of::<T>() as u64)
                    .try_into()
                    .unwrap();
            } else {
                current_start_chunk += area.num_chunks;
            }
        }
        unimplemented!("Buffer pool growing");
    }

    pub fn free_subchunk_areas(&mut self, subchunk_coords: [i32; 3]) {
        // Mark all subchunk owned areas as free
        for area in &mut self.usage_map {
            if area.belongs_to(subchunk_coords) {
                area.usage = BufferAreaUsage::Free;
            }
        }
        // Merge free areas
        let mut current_area_i: usize = 1;
        while current_area_i < self.usage_map.len() {
            if self.usage_map[current_area_i - 1].is_free()
                && self.usage_map[current_area_i].is_free()
            {
                self.usage_map[current_area_i - 1].num_chunks +=
                    self.usage_map[current_area_i].num_chunks;
                self.usage_map.remove(current_area_i);
            } else {
                current_area_i += 1;
            }
        }
    }

    pub fn get_slice(&self) -> BufferSlice {
        self.instance_buffer.slice(..)
    }

    pub fn get_entire_binding(&self) -> wgpu::BindingResource {
        self.instance_buffer.as_entire_binding()
    }

    pub fn get_lightmaps(&self) -> &[wgpu::Buffer; 2] {
        &self.lightmap_buffers
    }

    pub fn size(&self) -> wgpu::BufferAddress {
        self.instance_buffer.size()
    }

    pub fn usage_fraction(&self) -> f64 {
        let mut total_chunks: u64 = 0;
        let mut used_chunks: u64 = 0;
        for area in &self.usage_map {
            total_chunks += area.num_chunks;
            if !area.is_free() {
                used_chunks += area.num_chunks;
            }
        }
        used_chunks as f64 / total_chunks as f64
    }
}

pub mod compute {
    use super::*;

    pub fn create_cascade_update_pipelines(
        device: &Device,
        layout: &PipelineLayout,
    ) -> [ComputePipeline; 2] {
        let cascade_0_pipeline = {
            let shader =
                device.create_shader_module(include_wgsl!("shaders/radiance_probe_update.wgsl"));
            device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some("Cascade 0 Update Compute Pipeline"),
                layout: Some(layout),
                module: &shader,
                entry_point: "update_cascade",
                compilation_options: PipelineCompilationOptions::default(),
                cache: None,
            })
        };
        let cascade_1_pipeline = {
            let shader = device
                .create_shader_module(include_wgsl!("shaders/radiance_cascade_1_update.wgsl"));
            device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some("Cascade 1 Update Compute Pipeline"),
                layout: Some(layout),
                module: &shader,
                entry_point: "update_cascade",
                compilation_options: PipelineCompilationOptions::default(),
                cache: None,
            })
        };
        [cascade_0_pipeline, cascade_1_pipeline]
    }

    pub fn create_raytracing_debug_pipeline(
        device: &Device,
        layout: &PipelineLayout,
    ) -> ComputePipeline {
        let shader = device.create_shader_module(include_wgsl!("shaders/rc_test.wgsl"));
        device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Raytrace Testing Compute Pipeline"),
            layout: Some(layout),
            module: &shader,
            entry_point: "render_raytraced",
            compilation_options: PipelineCompilationOptions {
                constants: &HashMap::from([(
                    String::from("cascade_0_ray_length"),
                    CASCADE_0_RAY_LENGTH,
                )]),
                ..Default::default()
            },
            cache: None,
        })
    }
}

pub mod block_face {
    use super::*;

    pub fn create_render_pipeline(
        device: &Device,
        config: &SurfaceConfiguration,
        layout: &PipelineLayout,
    ) -> RenderPipeline {
        let shader =
            device.create_shader_module(include_wgsl!("shaders/block_face_radiance_cascades.wgsl"));
        device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Block Face (Radiance Cascades) Render Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                compilation_options: Default::default(),
                buffers: &[Vertex::desc(), Instance::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::GreaterEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        })
    }

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

        pub fn generate_array() -> [[[f32; 4]; 3]; 6] {
            // Alignment of each row in a mat3x3 is same as vec4, so we pad up to size
            rotations()
                .map(Matrix3::from)
                .map(|matrix| matrix.into())
                .map(|matrix: [[f32; 3]; 3]| matrix.map(|[x, y, z]| [x, y, z, 0.0]))
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

    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct Vertex {
        subchunk_start_coords: [f32; 3],
        face_matrix_index: u32,
    }

    impl Vertex {
        const ATTRIBUTES: &'static [VertexAttribute] = &vertex_attr_array![
            // subchunk_start_coords
            0 => Float32x3,
            // face_matrix_index
            1 => Uint32,
        ];

        pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: Self::ATTRIBUTES,
            }
        }

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

    // NOTE: Keep definitions in radiance probe update shaders in sync with this.
    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct Instance {
        uvs: [u16; 4],
        /// 0-3: X offset
        /// 4-7: Y offset
        /// 8-11: Z offset
        /// 12-13: UV rotation
        /// 14: Emits light?
        /// 15-19: Unused
        /// 20-23: Sky light level
        /// 24-27: Block light level
        /// 28-31: Unused
        packed_fields: u32,
    }

    impl Instance {
        const ATTRIBUTES: &'static [VertexAttribute] = &vertex_attr_array![
            // uvs
            10 => Uint16x4,
            // packed_fields
            11 => Uint32,
        ];

        pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: Self::ATTRIBUTES,
            }
        }

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
            Self {
                uvs,
                packed_fields: (subchunk_xyz[0] as u32)
                    | ((subchunk_xyz[1] as u32) << 4)
                    | ((subchunk_xyz[2] as u32) << 8)
                    | ((packed_uv_rotation as u32) << 12)
                    | ((emits_light as u32) << 14)
                    | ((light_levels[0] as u32) << 20)
                    | ((light_levels[1] as u32) << 24),
            }
        }
    }

    pub type BlockFaceVertexBufferManager = VertexBufferManager<
        Vertex,
        { std::mem::size_of::<[[Vertex; 4]; 1 << 20]>() },
        { std::mem::size_of::<[Vertex; 4]>() },
    >;

    pub type BlockFaceInstanceBufferManager = InstanceBufferManager<Instance, 4, { 1 << 18 }>;
}

pub mod tinted_block_face {
    use super::*;

    pub fn create_render_pipeline(
        device: &Device,
        config: &SurfaceConfiguration,
        layout: &PipelineLayout,
    ) -> RenderPipeline {
        let shader = device.create_shader_module(include_wgsl!(
            "shaders/tinted_block_face_radiance_cascades.wgsl"
        ));
        device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Tinted Block Face (Radiance Cascades) Render Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                compilation_options: Default::default(),
                buffers: &[Vertex::desc(), Instance::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::GreaterEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        })
    }

    pub use super::block_face::Vertex;

    // NOTE: Keep definitions in radiance probe update shaders in sync with this.
    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct Instance {
        uvs: [u16; 4],
        tint_color: [u8; 4],
        /// 0-3: X offset
        /// 4-7: Y offset
        /// 8-11: Z offset
        /// 12-13: UV rotation
        /// 14: Emits light?
        /// 15-19: Unused
        /// 20-23: Sky light level
        /// 24-27: Block light level
        /// 28-31: Unused
        packed_fields: u32,
    }

    impl Instance {
        const ATTRIBUTES: &'static [VertexAttribute] = &vertex_attr_array![
            // uvs
            10 => Uint16x4,
            // tint_color
            11 => Unorm8x4,
            // packed_fields
            12 => Uint32,
        ];

        pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: Self::ATTRIBUTES,
            }
        }

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
            Self {
                uvs,
                tint_color,
                packed_fields: (subchunk_xyz[0] as u32)
                    | ((subchunk_xyz[1] as u32) << 4)
                    | ((subchunk_xyz[2] as u32) << 8)
                    | ((packed_uv_rotation as u32) << 12)
                    | ((emits_light as u32) << 14)
                    | ((light_levels[0] as u32) << 20)
                    | ((light_levels[1] as u32) << 24),
            }
        }
    }

    pub type TintedBlockFaceVertexBufferManager = VertexBufferManager<
        Vertex,
        { std::mem::size_of::<[[Vertex; 4]; 1 << 20]>() },
        { std::mem::size_of::<[Vertex; 4]>() },
    >;

    pub type TintedBlockFaceInstanceBufferManager = InstanceBufferManager<Instance, 4, { 1 << 18 }>;
}

pub mod custom_block {
    use super::*;

    pub fn create_render_pipeline(
        device: &Device,
        config: &SurfaceConfiguration,
        layout: &PipelineLayout,
    ) -> RenderPipeline {
        let shader = device
            .create_shader_module(include_wgsl!("shaders/custom_block_radiance_cascades.wgsl"));
        device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Custom Block (Radiance Cascades) Render Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                compilation_options: Default::default(),
                buffers: &[Vertex::desc(), Instance::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::GreaterEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        })
    }

    pub type VertexList = VertexListBuffer<Vertex>;
    pub type IndexList = IndexListBuffer<u32>;

    // TODO:
    // - Right now VRAM usage is really high for vertex buffer (150MB)
    // - Switch to using vertex pulling
    // - Two storage buffers:
    //   - Vertex buffer replaced by cube buffer, stores packed 4x4 matrix
    //   - Index buffer replaced by face buffer, stores direction index and cube index
    // - During rendering, just divide vertex index by 6 to get face index
    // - Packed matrix could be 4x4 Snorm8
    // - Radiance Cascade raytracing just needs ray-OBB tests
    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct Vertex {
        pub pos: [f32; 3],
        pub uvs: [u16; 2],
        pub normal: [f32; 3],
        /// 0: Tinted?
        /// 1-31: Unused
        pub packed_fields: u32,
    }

    impl Vertex {
        const ATTRIBUTES: &'static [VertexAttribute] = &vertex_attr_array![
            // pos
            0 => Float32x3,
            // uvs
            1 => Uint16x2,
            // normal
            2 => Float32x3,
            // packed_fields
            3 => Uint32,
        ];

        pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: Self::ATTRIBUTES,
            }
        }

        pub fn new(pos: [f32; 3], uvs: [u16; 2], normal: [f32; 3], is_tinted: bool) -> Self {
            Self {
                pos,
                uvs,
                normal,
                packed_fields: is_tinted as u32,
            }
        }
    }

    pub type InstanceList = VertexListBuffer<Instance>;

    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct Instance {
        pos: [f32; 3],
        tint_color: [u8; 4],
        /// Light levels for surrounding blocks in order:
        /// 1: Centre
        /// 2: Above
        /// 3: Below
        /// 4: North
        /// 5: South
        /// 6: East
        /// 7: West
        light_level_pairs: [u8; 7],
        /// 0: Emits light?
        /// 1-7: Unused
        packed_fields: u8,
    }

    impl Instance {
        const ATTRIBUTES: &'static [VertexAttribute] = &vertex_attr_array![
            // pos
            10 => Float32x3,
            // tint_color
            11 => Unorm8x4,
            // light_level_pairs (first half)
            12 => Uint8x4,
            // light_level_pairs (second half) and packed_fields
            13 => Uint8x4,
        ];

        pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: Self::ATTRIBUTES,
            }
        }

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
            Self {
                pos,
                tint_color,
                light_level_pairs: converted_light_level_pairs,
                packed_fields: emits_light as u8,
            }
        }
    }

    pub type CustomBlockInstanceBufferManager = super::super::chunk::InstanceBufferManager<
        Instance,
        { std::mem::size_of::<[[Instance; 4]; 1 << 20]>() },
        { std::mem::size_of::<[Instance; 4]>() },
    >;
}
