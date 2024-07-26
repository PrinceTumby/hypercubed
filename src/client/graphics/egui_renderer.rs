use super::{GraphicsResources, Texture};
use ahash::AHashMap;
use egui::{epaint, TextureId};
use wgpu::util::DeviceExt as _;
use wgpu::{include_wgsl, vertex_attr_array};

pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    screen_size_buffer: wgpu::Buffer,
    screen_size_bind_group: wgpu::BindGroup,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    textures: AHashMap<TextureId, TextureData>,
    sampler_cache: AHashMap<epaint::textures::TextureOptions, wgpu::Sampler>,
}

struct TextureData {
    pub texture: wgpu::Texture,
    pub bind_group: wgpu::BindGroup,
}

pub struct RenderData {
    meshes: Vec<RenderMeshInfo>,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 2],
    pub uvs: [f32; 2],
    pub color: [u8; 4],
}

impl Vertex {
    const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &vertex_attr_array![
        // pos
        0 => Float32x2,
        // uvs
        1 => Float32x2,
        // color
        2 => Unorm8x4,
    ];

    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: Self::ATTRIBUTES,
        }
    }
}

pub struct RenderMeshInfo {
    pub scissor_rect: ScissorRect,
    pub base_vertex: i32,
    pub index_slice: std::ops::Range<u32>,
    pub texture_id: TextureId,
}

#[derive(Clone, Copy)]
pub struct ScissorRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ScreenSize {
    width: f32,
    height: f32,
}

impl Renderer {
    pub fn new(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> Self {
        let screen_size_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Screen Size Bind Group Layout"),
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
        let screen_size_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("egui Screen Size Buffer"),
            contents: bytemuck::cast_slice(&[ScreenSize {
                width: 0.0,
                height: 0.0,
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let screen_size_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("egui Screen Size Bind Group"),
            layout: &screen_size_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &screen_size_buffer,
                    offset: 0,
                    size: None,
                }),
            }],
        });
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Texture Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        // Render pipeline
        let shader = device.create_shader_module(include_wgsl!("shaders/egui.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("egui Pipeline Layout"),
            bind_group_layouts: &[&screen_size_bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("egui Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                compilation_options: Default::default(),
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
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
        });
        Self {
            pipeline,
            screen_size_buffer,
            screen_size_bind_group,
            texture_bind_group_layout,
            textures: AHashMap::new(),
            sampler_cache: AHashMap::new(),
        }
    }

    /// Call after rendering.
    pub fn free_textures(&mut self, texture_ids: &[egui::TextureId]) {
        for id in texture_ids {
            self.textures.remove(id);
        }
    }

    fn update_textures(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        textures: Vec<(egui::TextureId, epaint::image::ImageDelta)>,
    ) {
        for (texture_id, texture_data) in textures {
            let [width, height] = texture_data.image.size();
            let size = wgpu::Extent3d {
                width: width as u32,
                height: height as u32,
                depth_or_array_layers: 1,
            };
            let rgba_pixels = match &texture_data.image {
                epaint::ImageData::Color(image) => {
                    assert_eq!(width * height, image.pixels.len());
                    std::borrow::Cow::Borrowed(&image.pixels)
                }
                epaint::ImageData::Font(image) => {
                    assert_eq!(width * height, image.pixels.len());
                    std::borrow::Cow::Owned(
                        image.srgba_pixels(None).collect::<Vec<egui::Color32>>(),
                    )
                }
            };
            let rgba_bytes: &[u8] = bytemuck::cast_slice(rgba_pixels.as_slice());
            if let Some(pos) = texture_data.pos {
                // Update existing texture
                let current_texture = &self.textures[&texture_id];
                let origin = wgpu::Origin3d {
                    x: pos[0] as u32,
                    y: pos[1] as u32,
                    z: 0,
                };
                queue.write_texture(
                    wgpu::ImageCopyTexture {
                        texture: &current_texture.texture,
                        mip_level: 0,
                        origin,
                        aspect: wgpu::TextureAspect::All,
                    },
                    rgba_bytes,
                    wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * width as u32),
                        rows_per_image: Some(height as u32),
                    },
                    size,
                );
            } else {
                // Register new texture
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: None,
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[wgpu::TextureFormat::Rgba8UnormSrgb],
                });
                let sampler = self
                    .sampler_cache
                    .entry(texture_data.options)
                    .or_insert_with(|| {
                        use epaint::textures::{TextureFilter, TextureWrapMode};
                        fn convert_filter(filter: TextureFilter) -> wgpu::FilterMode {
                            match filter {
                                TextureFilter::Nearest => wgpu::FilterMode::Nearest,
                                TextureFilter::Linear => wgpu::FilterMode::Linear,
                            }
                        }
                        let address_mode = match texture_data.options.wrap_mode {
                            TextureWrapMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
                            TextureWrapMode::Repeat => wgpu::AddressMode::Repeat,
                            TextureWrapMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
                        };
                        device.create_sampler(&wgpu::SamplerDescriptor {
                            label: None,
                            mag_filter: convert_filter(texture_data.options.magnification),
                            min_filter: convert_filter(texture_data.options.minification),
                            address_mode_u: address_mode,
                            address_mode_v: address_mode,
                            ..Default::default()
                        })
                    });
                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &self.texture_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(
                                &texture.create_view(&wgpu::TextureViewDescriptor::default()),
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(sampler),
                        },
                    ],
                });
                queue.write_texture(
                    wgpu::ImageCopyTexture {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    rgba_bytes,
                    wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * width as u32),
                        rows_per_image: Some(height as u32),
                    },
                    size,
                );
                self.textures.insert(
                    texture_id,
                    TextureData {
                        texture,
                        bind_group,
                    },
                );
            }
        }
    }

    pub fn prepare(
        &mut self,
        graphics_resources: &GraphicsResources,
        physical_size: &winit::dpi::PhysicalSize<u32>,
        texture_updates: Vec<(egui::TextureId, epaint::image::ImageDelta)>,
        primitives: Vec<egui::ClippedPrimitive>,
        pixels_per_point: f32,
    ) -> RenderData {
        let device = &graphics_resources.device;
        let queue = &graphics_resources.queue;
        let width = physical_size.width as f32;
        let height = physical_size.height as f32;
        graphics_resources.queue.write_buffer(
            &self.screen_size_buffer,
            0,
            bytemuck::cast_slice(&[ScreenSize { width, height }]),
        );
        self.update_textures(device, queue, texture_updates);
        let mut vertices: Vec<Vertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        let mut meshes: Vec<RenderMeshInfo> = Vec::with_capacity(primitives.len());
        for egui::ClippedPrimitive {
            clip_rect,
            primitive,
        } in primitives
        {
            if clip_rect.area() == 0.0 {
                continue;
            }
            let epaint::Primitive::Mesh(mesh) = primitive else {
                continue;
            };
            let base_vertex = vertices.len() as i32;
            let indices_start = indices.len() as u32;
            vertices.extend(mesh.vertices.into_iter().map(|v| Vertex {
                pos: [v.pos.x * pixels_per_point, v.pos.y * pixels_per_point],
                uvs: [v.uv.x, v.uv.y],
                color: v.color.to_array(),
            }));
            indices.extend(mesh.indices.into_iter());
            let indices_end = indices.len() as u32;
            meshes.push(RenderMeshInfo {
                scissor_rect: ScissorRect {
                    x: (clip_rect.min.x * pixels_per_point).min(width) as u32,
                    y: (clip_rect.min.y * pixels_per_point).min(height) as u32,
                    width: ((clip_rect.max.x - clip_rect.min.x) * pixels_per_point).min(width)
                        as u32,
                    height: ((clip_rect.max.y - clip_rect.min.y) * pixels_per_point).min(height)
                        as u32,
                },
                base_vertex,
                index_slice: indices_start..indices_end,
                texture_id: mesh.texture_id,
            });
            //vertices.extend_from_slice(&[
            //    Vertex {
            //        pos: [0., 0.],
            //        uvs: [0., 0.],
            //        color: [0xFF, 0xFF, 0xFF, 0xFF],
            //    },
            //    Vertex {
            //        pos: [400., 0.],
            //        uvs: [0., 0.],
            //        color: [0xFF, 0xFF, 0xFF, 0xFF],
            //    },
            //    Vertex {
            //        pos: [0., 400.],
            //        uvs: [0., 0.],
            //        color: [0xFF, 0xFF, 0xFF, 0xFF],
            //    },
            //    Vertex {
            //        pos: [400., 400.],
            //        uvs: [0., 0.],
            //        color: [0xFF, 0xFF, 0xFF, 0xFF],
            //    },
            //]);
            //indices.extend_from_slice(&[0, 1, 2, 2, 1, 3]);
            //meshes.push(RenderMeshInfo {
            //    scissor_rect: ScissorRect {
            //        x: 0,
            //        y: 0,
            //        width: physical_size.width,
            //        height: physical_size.height,
            //    },
            //    base_vertex: 0,
            //    index_slice: 0..indices.len() as u32,
            //    texture_id: mesh.texture_id,
            //});
            //break;
        }
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("egui Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices.as_slice()),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("egui Index Buffer"),
            contents: bytemuck::cast_slice(indices.as_slice()),
            usage: wgpu::BufferUsages::INDEX,
        });
        RenderData {
            meshes,
            vertex_buffer,
            index_buffer,
        }
    }

    /// Clobbers the render pass scissor rect.
    pub fn render<'a: 'pass, 'pass>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'pass>,
        render_data: &'pass RenderData,
    ) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.screen_size_bind_group, &[]);
        render_pass.set_vertex_buffer(0, render_data.vertex_buffer.slice(..));
        render_pass.set_index_buffer(
            render_data.index_buffer.slice(..),
            wgpu::IndexFormat::Uint32,
        );
        for mesh in &render_data.meshes {
            render_pass.set_bind_group(1, &self.textures[&mesh.texture_id].bind_group, &[]);
            render_pass.set_scissor_rect(
                mesh.scissor_rect.x,
                mesh.scissor_rect.y,
                mesh.scissor_rect.width,
                mesh.scissor_rect.height,
            );
            render_pass.draw_indexed(mesh.index_slice.clone(), mesh.base_vertex, 0..1);
        }
    }
}
