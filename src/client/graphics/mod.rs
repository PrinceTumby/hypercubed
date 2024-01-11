pub mod chunk;

use crate::resource;
use nalgebra::{Isometry3, Matrix4, Point3, UnitQuaternion};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::window::Window;

#[derive(Debug)]
pub struct Camera {
    pub pos: Point3<f32>,
    pub proj_matrix: Matrix4<f32>,
    /// Represented in degrees
    pub yaw: f32,
    /// Represented in degrees
    pub pitch: f32,
    /// Represented in degrees
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

    pub fn generate_view_matrix_slice(&self) -> [[f32; 4]; 4] {
        let translate = Isometry3::new(self.pos.coords, nalgebra::zero())
            .inverse()
            .to_matrix();
        let rotate = self.get_rot().inverse().to_homogeneous();
        let view_matrix = self.proj_matrix * rotate * translate;
        *view_matrix.as_ref()
    }
}

#[derive(Debug)]
pub struct GraphicsResources {
    pub surface: wgpu::Surface,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

#[derive(Debug)]
pub struct GraphicsState {
    pub resources: Arc<GraphicsResources>,
    pub config: wgpu::SurfaceConfiguration,
    pub block_render_pipeline: wgpu::RenderPipeline,
    pub tinted_block_render_pipeline: wgpu::RenderPipeline,
    pub depth_texture: Texture,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub instance_buffer: wgpu::Buffer,
    pub num_indices: u32,
    pub block_item_atlas_bind_group: wgpu::BindGroup,
    pub block_registry: resource::block::Registry,
    pub camera: Camera,
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group: wgpu::BindGroup,
    pub camera_bind_group_layout: wgpu::BindGroupLayout,
    pub matrices_bind_group: wgpu::BindGroup,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub clear_color: wgpu::Color,
}

impl GraphicsState {
    const DEFAULT_FOV: f32 = 70.0;
    const DEFAULT_ZNEAR: f32 = 0.01;
    const DEFAULT_ZFAR: f32 = 10000.0;

    #[tracing::instrument(skip_all)]
    pub async fn new<F>(window: &Window, register_blocks: F) -> anyhow::Result<Self>
    where
        F: FnOnce(
            &mut resource::block::Registry,
            &mut resource::block::model::ModelCache,
            &mut resource::texture::AtlasBuilder,
        ) -> anyhow::Result<()>,
    {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(Default::default());
        let surface = unsafe { instance.create_surface(window)? };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::PUSH_CONSTANTS,
                    limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                    label: None,
                },
                None, // Trace path
            )
            .await
            .unwrap();
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
        };
        surface.configure(&device, &config);
        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("camera_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let matrices_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("matrices_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
        let atlas_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("atlas_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let (block_item_texture_atlas, block_item_atlas_size, block_registry) = {
            use crate::resource;
            let size = [1024; 2];
            let square_length = 16;
            let mut atlas_builder =
                resource::texture::AtlasBuilder::new(size[0], size[1], square_length);
            let mut model_cache = resource::block::model::ModelCache::new();
            let mut block_registry = resource::block::Registry::new();
            register_blocks(&mut block_registry, &mut model_cache, &mut atlas_builder)?;
            let atlas = atlas_builder.build(&device, &queue, Some("Block and Item Atlas"));
            (atlas, size, block_registry)
        };
        let block_item_atlas_size_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Block and Item Atlas Size Buffer"),
                contents: bytemuck::cast_slice(&block_item_atlas_size.map(|x| x as f32)),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let block_item_atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("block_item_atlas_bind_group"),
            layout: &atlas_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: block_item_atlas_size_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&block_item_texture_atlas.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&block_item_texture_atlas.sampler),
                },
            ],
        });
        let depth_texture = Texture::create_depth_texture(&device, &config, "depth_texture");
        let generic_block_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Chunk Block Pipeline Layout"),
                bind_group_layouts: &[
                    &camera_bind_group_layout,
                    &atlas_bind_group_layout,
                    &matrices_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });
        let block_render_pipeline = chunk::block_face::create_render_pipeline(
            &device,
            &config,
            &generic_block_pipeline_layout,
        );
        let tinted_block_render_pipeline = chunk::tinted_block_face::create_render_pipeline(
            &device,
            &config,
            &generic_block_pipeline_layout,
        );
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertex_buffer"),
            contents: bytemuck::cast_slice(chunk::block_face::VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("instance_buffer"),
            contents: bytemuck::cast_slice(&[] as &[chunk::block_face::Instance]),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("index_buffer"),
            contents: bytemuck::cast_slice(chunk::block_face::INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });
        let proj_matrix = Matrix4::new_perspective(
            (size.width as f32) / (size.height as f32),
            f32::to_radians(GraphicsState::DEFAULT_FOV),
            GraphicsState::DEFAULT_ZNEAR,
            GraphicsState::DEFAULT_ZFAR,
        );
        let camera = Camera {
            pos: Point3::new(-2.0, 1.0, 2.0),
            proj_matrix,
            yaw: 45.0,
            pitch: -10.0,
            roll: 0.0,
        };
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&camera.generate_view_matrix_slice()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera_bind_group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });
        let face_matrices_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Face Matrices Buffer"),
            contents: bytemuck::cast_slice(&chunk::block_face::face_matrices::generate_array()),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let x_rotation_matrices_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("X Rotation Matrices Buffer"),
                contents: bytemuck::cast_slice(
                    &chunk::block_face::rotation_matrices::generate_x_rotation_array(),
                ),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let y_rotation_matrices_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Y Rotation Matrices Buffer"),
                contents: bytemuck::cast_slice(
                    &chunk::block_face::rotation_matrices::generate_y_rotation_array(),
                ),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let matrices_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("matrices_bind_group"),
            layout: &matrices_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: face_matrices_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: x_rotation_matrices_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: y_rotation_matrices_buffer.as_entire_binding(),
                },
            ],
        });
        Ok(Self {
            resources: Arc::new(GraphicsResources {
                surface,
                device,
                queue,
            }),
            config,
            block_render_pipeline,
            tinted_block_render_pipeline,
            depth_texture,
            vertex_buffer,
            index_buffer,
            instance_buffer,
            num_indices: (chunk::block_face::INDICES.len() * 3) as u32,
            block_item_atlas_bind_group,
            block_registry,
            camera,
            camera_buffer,
            camera_bind_group,
            camera_bind_group_layout,
            matrices_bind_group,
            size,
            // Minecraft plains biome sky color
            clear_color: wgpu::Color {
                r: 0.471,
                g: 0.655,
                b: 1.0,
                a: 1.0,
            },
        })
    }

    #[tracing::instrument(skip(self))]
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.resources
                .surface
                .configure(&self.resources.device, &self.config);
            self.depth_texture = Texture::create_depth_texture(
                &self.resources.device,
                &self.config,
                "depth_texture",
            );
            self.camera.proj_matrix = Matrix4::new_perspective(
                (new_size.width as f32) / (new_size.height as f32),
                f32::to_radians(GraphicsState::DEFAULT_FOV),
                GraphicsState::DEFAULT_ZNEAR,
                GraphicsState::DEFAULT_ZFAR,
            );
        }
    }

    pub fn render(
        &self,
        block_faces: &chunk::block_face::InstanceList,
        tinted_block_faces: &chunk::tinted_block_face::InstanceList,
    ) -> Result<(), wgpu::SurfaceError> {
        self.resources.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&self.camera.generate_view_matrix_slice()),
        );
        let output = self.resources.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder =
            self.resources
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });
        // Main render pass
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            // Base block faces
            {
                render_pass.set_pipeline(&self.block_render_pipeline);
                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                render_pass.set_bind_group(1, &self.block_item_atlas_bind_group, &[]);
                render_pass.set_bind_group(2, &self.matrices_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                render_pass
                    .set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.set_vertex_buffer(1, block_faces.get_instances_slice());
                render_pass.draw_indexed(0..self.num_indices, 0, 0..block_faces.num_instances());
            }
            // Tinted block faces
            {
                render_pass.set_pipeline(&self.tinted_block_render_pipeline);
                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                render_pass.set_bind_group(1, &self.block_item_atlas_bind_group, &[]);
                render_pass.set_bind_group(2, &self.matrices_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                render_pass
                    .set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.set_vertex_buffer(1, tinted_block_faces.get_instances_slice());
                render_pass.draw_indexed(
                    0..self.num_indices,
                    0,
                    0..tinted_block_faces.num_instances(),
                );
            }
        }
        self.resources
            .queue
            .submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }
}

#[derive(Debug)]
pub struct Texture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl Texture {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    #[tracing::instrument(skip(device, config))]
    pub fn create_depth_texture(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        label: &str,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let texture = device.create_texture(&desc);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            lod_min_clamp: 0.0,
            lod_max_clamp: 100.0,
            ..Default::default()
        });
        Self {
            texture,
            view,
            sampler,
        }
    }
}
