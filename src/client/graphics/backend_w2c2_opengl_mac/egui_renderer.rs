use super::gl;
use crate::platform::libs::winit;
use ahash::AHashMap;
use image::RgbaImage;
use nalgebra::Matrix4;
use portable_std::FastHashMap;

// TODO: Use texture buffer objects, update textures using OpenGL.

pub struct Renderer {
    images: FastHashMap<egui::TextureId, ImageData>,
    next_user_texture_id: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ScreenSize {
    width: f32,
    height: f32,
}

pub struct ImageData {
    pub image: RgbaImage,
    pub options: egui::TextureOptions,
}

impl ImageData {
    pub unsafe fn set_as_current_texture(&self) {
        unsafe {
            fn convert_filter(filter: egui::TextureFilter) -> gl::TexFilterMode {
                match filter {
                    egui::TextureFilter::Nearest => gl::TexFilterMode::Nearest,
                    egui::TextureFilter::Linear => gl::TexFilterMode::Linear,
                }
            }
            let options = self.options;
            let wrap_mode = match options.wrap_mode {
                egui::TextureWrapMode::Repeat => gl::TexWrapMode::Repeat,
                egui::TextureWrapMode::ClampToEdge => gl::TexWrapMode::ClampToEdge,
                egui::TextureWrapMode::MirroredRepeat => gl::TexWrapMode::MirroredRepeat,
            };
            gl::tex_wrap_s(gl::TexTarget::Texture2d, wrap_mode);
            gl::tex_wrap_t(gl::TexTarget::Texture2d, wrap_mode);
            gl::tex_mag_filter(
                gl::TexTarget::Texture2d,
                convert_filter(options.magnification),
            );
            gl::tex_min_filter(
                gl::TexTarget::Texture2d,
                convert_filter(options.minification),
            );
            gl::tex_image_2d(
                gl::Texture2dTarget::Texture,
                0,
                gl::TextureInternalFormat::Rgba,
                self.image.width() as usize,
                self.image.height() as usize,
                0,
                gl::Texture2dFormat::Rgba,
                gl::TextureDataType::U8,
                self.image.as_ptr(),
            );
        }
    }
}

struct RenderMeshInfo {
    raw_clip_rect: egui::Rect,
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    texture_id: egui::TextureId,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pub pos: [f32; 2],
    pub uvs: [f32; 2],
    pub color: [u8; 4],
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            images: AHashMap::new(),
            next_user_texture_id: 0,
        }
    }

    pub fn free_textures(&mut self, texture_ids: &[egui::TextureId]) {
        for id in texture_ids {
            self.images.remove(id);
        }
    }

    fn update_textures(
        &mut self,
        textures: Vec<(egui::TextureId, egui::epaint::image::ImageDelta)>,
    ) {
        for (texture_id, texture_data) in textures {
            let [width, height] = texture_data.image.size();
            let size = texture_data.image.size().map(|n| n as u32);
            let pixels = match &texture_data.image {
                egui::ImageData::Color(image) => {
                    assert_eq!(width * height, image.pixels.len());
                    image
                        .pixels
                        .iter()
                        .map(|&color| color.to_array())
                        .flatten()
                        .collect()
                }
                egui::ImageData::Font(image) => {
                    assert_eq!(width * height, image.pixels.len());
                    image
                        .srgba_pixels(None)
                        .map(|color| color.to_array())
                        .flatten()
                        .collect()
                }
            };
            let new_image = RgbaImage::from_raw(width as u32, height as u32, pixels).unwrap();
            if let Some(pos) = texture_data.pos {
                // Update existing image with new data
                let current_image_data = self.images.get_mut(&texture_id).unwrap();
                let current_image = &mut current_image_data.image;
                let origin: [u32; 2] = pos.map(|n| n as u32);
                for y in 0..size[1] {
                    for x in 0..size[0] {
                        current_image[(origin[0] + x, origin[1] + y)] = new_image[(x, y)];
                    }
                }
            } else {
                // Register new image
                self.images.insert(
                    texture_id,
                    ImageData {
                        image: new_image,
                        options: texture_data.options,
                    },
                );
            }
        }
    }

    pub unsafe fn render(
        &mut self,
        physical_size: &winit::dpi::PhysicalSize<u32>,
        texture_updates: Vec<(egui::TextureId, egui::epaint::image::ImageDelta)>,
        primitives: Vec<egui::ClippedPrimitive>,
        pixels_per_point: f32,
    ) {
        unsafe {
            let (width, height) = (physical_size.width, physical_size.height);
            let width_f32 = width as f32;
            let height_f32 = height as f32;
            self.update_textures(texture_updates);
            // Generate mesh data
            let mut meshes: Vec<RenderMeshInfo> = Vec::with_capacity(primitives.len());
            for egui::ClippedPrimitive {
                clip_rect,
                primitive,
            } in primitives
            {
                if clip_rect.area() == 0.0 {
                    continue;
                }
                let egui::epaint::Primitive::Mesh(mesh) = primitive else {
                    unimplemented!("egui custom callbacks");
                };
                if mesh.vertices.is_empty() {
                    continue;
                }
                meshes.push(RenderMeshInfo {
                    raw_clip_rect: clip_rect,
                    vertices: mesh
                        .vertices
                        .into_iter()
                        .map(|v| Vertex {
                            pos: (v.pos * pixels_per_point).into(),
                            uvs: v.uv.into(),
                            color: v.color.to_array(),
                        })
                        .collect(),
                    indices: mesh.indices,
                    texture_id: mesh.texture_id,
                });
            }
            // Setup OpenGL state
            gl::disable(gl::EnableComponent::DepthTest);
            gl::enable(gl::EnableComponent::ScissorTest);
            gl::enable(gl::EnableComponent::Blending);
            gl::client_active_texture(0);
            gl::tex_env_mode(gl::TexEnvTarget::TextureEnv, gl::TexEnvMode::Modulate);
            gl::enable_client_state(gl::ClientArrayType::VertexArray);
            gl::enable_client_state(gl::ClientArrayType::ColorArray);
            gl::enable_client_state(gl::ClientArrayType::TextureCoordArray);
            gl::matrix_mode(gl::MatrixMode::Projection);
            // Set blending mode
            gl::blend_equation(gl::BlendEquationFunc::Add);
            gl::blend_function(gl::BlendFactor::One, gl::BlendFactor::OneMinusSrcAlpha);
            // Vertex coordinates are in screen coordinates, so correct them with a projection
            // matrix.
            // Y coordinates also require flipping.
            let screen_projection_matrix =
                nalgebra::Orthographic3::new(0.0, width_f32 - 1.0, height_f32 - 1.0, 0.0, 0.0, 1.0);
            gl::load_matrix_f32(screen_projection_matrix.as_matrix().as_ref());
            // Render meshes
            for mesh in meshes {
                let texture = &self.images[&mesh.texture_id];
                // Set texture
                texture.set_as_current_texture();
                // Set clipping rect
                {
                    // Adapted from the code for `egui_glow`'s `set_clip_rect` function.
                    // Transform clip rect to physical pixels
                    let clip_min_x = pixels_per_point * mesh.raw_clip_rect.min.x;
                    let clip_min_y = pixels_per_point * mesh.raw_clip_rect.min.y;
                    let clip_max_x = pixels_per_point * mesh.raw_clip_rect.max.x;
                    let clip_max_y = pixels_per_point * mesh.raw_clip_rect.max.y;
                    // Round to integer
                    let clip_min_x = clip_min_x.round() as i32;
                    let clip_min_y = clip_min_y.round() as i32;
                    let clip_max_x = clip_max_x.round() as i32;
                    let clip_max_y = clip_max_y.round() as i32;
                    // Clamp
                    let clip_min_x = clip_min_x.clamp(0, width as i32);
                    let clip_min_y = clip_min_y.clamp(0, height as i32);
                    let clip_max_x = clip_max_x.clamp(clip_min_x, width as i32);
                    let clip_max_y = clip_max_y.clamp(clip_min_y, height as i32);
                    gl::scissor(
                        clip_min_x as u32,
                        height - clip_max_y as u32,
                        (clip_max_x - clip_min_x) as usize,
                        (clip_max_y - clip_min_y) as usize,
                    );
                }
                // gl::scissor(
                //     width - 1 - mesh.clip_rect.max.x as u32,
                //     height - 1 - mesh.clip_rect.max.y as u32,
                //     (mesh.clip_rect.max.x - mesh.clip_rect.min.x) as usize,
                //     (mesh.clip_rect.max.y - mesh.clip_rect.min.y) as usize,
                // );
                // Set vertex attributes
                gl::vertex_pointer(
                    2, // 2D vertices
                    gl::VertexPointerType::F32,
                    core::mem::size_of::<Vertex>(), // Stride
                    &raw const mesh.vertices[0].pos as *const u8, // Pointer to first element
                );
                gl::color_pointer(
                    4, // RGBA colours
                    gl::ColorPointerType::U8,
                    core::mem::size_of::<Vertex>(), // Stride
                    &raw const mesh.vertices[0].color as *const u8, // Pointer to first element
                );
                gl::texture_coord_pointer(
                    2, // 2D texture coordinates
                    gl::TextureCoordPointerType::F32,
                    core::mem::size_of::<Vertex>(), // Stride
                    &raw const mesh.vertices[0].uvs as *const u8, // Pointer to first element
                );
                // Render mesh
                gl::draw_elements(
                    gl::ShapeMode::Triangles,
                    mesh.indices.len(),
                    gl::IndexType::U32,
                    mesh.indices.as_ptr() as *const u8,
                );
            }
            // Cleanup
            gl::flush();
            gl::disable_client_state(gl::ClientArrayType::VertexArray);
            gl::disable_client_state(gl::ClientArrayType::ColorArray);
            gl::disable_client_state(gl::ClientArrayType::TextureCoordArray);
            gl::disable(gl::EnableComponent::ScissorTest);
            gl::enable(gl::EnableComponent::DepthTest);
        }
    }

    pub fn register_user_image(
        &mut self,
        image: RgbaImage,
        options: egui::TextureOptions,
    ) -> anyhow::Result<egui::TextureId> {
        let texture_id = egui::TextureId::User(self.next_user_texture_id);
        self.next_user_texture_id += 1;
        self.images.insert(texture_id, ImageData { image, options });
        Ok(texture_id)
    }
}
