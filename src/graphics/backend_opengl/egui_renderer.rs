use super::gl;
use crate::platform::libs::winit;
use ahash::AHashMap;
use image::RgbaImage;
use portable_std::FastHashMap;

use gl::array::{ColorPointerType, IndexType, TextureCoordPointerType, VertexPointerType};
use gl::client_state::ClientArrayType;
use gl::fragment::{BlendEquationFunc, BlendFactor};
use gl::matrix::MatrixMode;
use gl::texture::{
    TexEnvMode, TexEnvTarget, TexFilterMode, TexTarget, TexWrapMode, Texture2dFormat,
    Texture2dTarget, TextureDataType, TextureInternalFormat,
};
use gl::{GLint, GLsizei, ShapeMode};

// TODO: Use texture buffer objects, update textures using OpenGL.

pub struct Renderer {
    images: FastHashMap<egui::TextureId, ImageData>,
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ScreenSize {
    width: f32,
    height: f32,
}

pub struct ImageData {
    pub image: RgbaImage,
    gl_texture: gl::texture::batch_collected::Texture,
}

impl ImageData {
    /// # SAFETY
    ///
    /// The main OpenGL context must be current.
    pub unsafe fn new(image: RgbaImage, options: egui::TextureOptions) -> Self {
        unsafe {
            let [gl_texture] = gl::texture::batch_collected::Texture::make_array();
            gl_texture.bind(TexTarget::Texture2D);
            Self::apply_egui_texture_options_to_current(options);
            gl::texture::set_image_2d(
                Texture2dTarget::Texture,
                0,
                TextureInternalFormat::Rgba,
                image.width().try_into().unwrap(),
                image.height().try_into().unwrap(),
                0,
                Texture2dFormat::Rgba,
                TextureDataType::U8,
                image.as_ptr() as *const (),
            );
            Self { image, gl_texture }
        }
    }

    /// # SAFETY
    ///
    /// The main OpenGL context must be current.
    pub unsafe fn bind(&self) {
        unsafe {
            self.gl_texture.bind(TexTarget::Texture2D);
        }
    }

    /// # SAFETY
    ///
    /// The main OpenGL context must be current.
    pub unsafe fn update_gl_texture(&self) {
        unsafe {
            self.gl_texture.bind(TexTarget::Texture2D);
            gl::texture::set_image_2d(
                Texture2dTarget::Texture,
                0,
                TextureInternalFormat::Rgba,
                self.image.width().try_into().unwrap(),
                self.image.height().try_into().unwrap(),
                0,
                Texture2dFormat::Rgba,
                TextureDataType::U8,
                self.image.as_ptr() as *const (),
            );
        }
    }

    /// # SAFETY
    ///
    /// The main OpenGL context must be current.
    unsafe fn apply_egui_texture_options_to_current(options: egui::TextureOptions) {
        unsafe {
            fn convert_filter(filter: egui::TextureFilter) -> TexFilterMode {
                match filter {
                    egui::TextureFilter::Nearest => TexFilterMode::Nearest,
                    egui::TextureFilter::Linear => TexFilterMode::Linear,
                }
            }
            let wrap_mode = match options.wrap_mode {
                egui::TextureWrapMode::Repeat => TexWrapMode::Repeat,
                egui::TextureWrapMode::ClampToEdge => TexWrapMode::ClampToEdge,
                egui::TextureWrapMode::MirroredRepeat => TexWrapMode::MirroredRepeat,
            };
            gl::texture::set_wrap_s(TexTarget::Texture2D, wrap_mode);
            gl::texture::set_wrap_t(TexTarget::Texture2D, wrap_mode);
            gl::texture::set_mag_filter(
                TexTarget::Texture2D,
                convert_filter(options.magnification),
            );
            gl::texture::set_min_filter(TexTarget::Texture2D, convert_filter(options.minification));
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
        }
    }

    pub fn free_textures(&mut self, texture_ids: &[egui::TextureId]) {
        for id in texture_ids {
            self.images.remove(id);
        }
    }

    /// # SAFETY
    ///
    /// The main OpenGL context must be current.
    #[tracing::instrument(skip_all)]
    unsafe fn update_textures(
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
                        .flat_map(|&color| color.to_array())
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
                unsafe {
                    current_image_data.update_gl_texture();
                }
            } else {
                // Register new image.
                self.images.insert(texture_id, unsafe {
                    ImageData::new(new_image, texture_data.options)
                });
            }
        }
    }

    #[tracing::instrument(skip_all)]
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
            gl::disable(gl::EnableComponent::AlphaTesting);
            gl::disable(gl::EnableComponent::FaceCulling);
            gl::enable(gl::EnableComponent::ScissorTest);
            gl::enable(gl::EnableComponent::Blending);
            gl::enable(gl::EnableComponent::Texture2D);
            gl::texture::set_env_mode(TexEnvTarget::TextureEnv, TexEnvMode::Modulate);
            gl::client_state::enable(ClientArrayType::VertexArray);
            gl::client_state::enable(ClientArrayType::ColorArray);
            gl::client_state::enable(ClientArrayType::TextureCoordArray);
            // Set blending mode.
            gl::fragment::set_blend_equation(BlendEquationFunc::Add);
            gl::fragment::set_blend_function(BlendFactor::One, BlendFactor::OneMinusSrcAlpha);
            // Reset matrices.
            gl::matrix::switch_mode(MatrixMode::ModelView);
            gl::matrix::load_identity();
            gl::matrix::switch_mode(MatrixMode::Texture);
            gl::matrix::load_identity();
            // Vertex coordinates are in screen coordinates, so correct them with a projection
            // matrix.
            // Y coordinates also require flipping.
            let screen_projection_matrix =
                nalgebra::Orthographic3::new(0.0, width_f32, height_f32, 0.0, 0.0, 1.0);
            gl::matrix::switch_mode(MatrixMode::Projection);
            gl::matrix::load_f32_matrix(screen_projection_matrix.as_matrix().as_ref());
            // Render meshes.
            for mesh in meshes {
                let texture = &self.images[&mesh.texture_id];
                // Set texture.
                texture.bind();
                // Set clipping rect.
                {
                    // Adapted from the code for `egui_glow`'s `set_clip_rect` function.
                    // Transform clip rect to physical pixels.
                    let clip_min_x = pixels_per_point * mesh.raw_clip_rect.min.x;
                    let clip_min_y = pixels_per_point * mesh.raw_clip_rect.min.y;
                    let clip_max_x = pixels_per_point * mesh.raw_clip_rect.max.x;
                    let clip_max_y = pixels_per_point * mesh.raw_clip_rect.max.y;
                    // Round to integer.
                    let clip_min_x = clip_min_x.round() as GLint;
                    let clip_min_y = clip_min_y.round() as GLint;
                    let clip_max_x = clip_max_x.round() as GLint;
                    let clip_max_y = clip_max_y.round() as GLint;
                    // Clamp.
                    let clip_min_x = clip_min_x.clamp(0, width as GLint);
                    let clip_min_y = clip_min_y.clamp(0, height as GLint);
                    let clip_max_x = clip_max_x.clamp(clip_min_x, width as GLint);
                    let clip_max_y = clip_max_y.clamp(clip_min_y, height as GLint);
                    gl::fragment::set_scissor(
                        clip_min_x,
                        height as GLint - clip_max_y,
                        (clip_max_x - clip_min_x) as GLsizei,
                        (clip_max_y - clip_min_y) as GLsizei,
                    );
                }
                // Unbind array buffer.
                gl::buffer::bind(gl::buffer::BufferType::ArrayBuffer, None);
                // Set vertex attributes.
                gl::array::vertex_pointer(
                    2, // 2D vertices
                    VertexPointerType::F32,
                    core::mem::size_of::<Vertex>().try_into().unwrap(), // Stride
                    (&raw const mesh.vertices[0].pos).addr(),           // Pointer to first element
                );
                gl::array::color_pointer(
                    4, // RGBA colours
                    ColorPointerType::U8,
                    core::mem::size_of::<Vertex>().try_into().unwrap(), // Stride
                    (&raw const mesh.vertices[0].color).addr(),         // Pointer to first element
                );
                gl::array::texture_coord_pointer(
                    2, // 2D texture coordinates
                    TextureCoordPointerType::F32,
                    core::mem::size_of::<Vertex>().try_into().unwrap(), // Stride
                    (&raw const mesh.vertices[0].uvs).addr(),           // Pointer to first element
                );
                // Render mesh.
                gl::array::draw_elements(
                    ShapeMode::Triangles,
                    mesh.indices.len().try_into().unwrap(),
                    IndexType::U32,
                    mesh.indices.as_ptr().addr(),
                );
            }
            // Cleanup.
            gl::texture::bind(TexTarget::Texture2D, None);
            gl::client_state::disable(ClientArrayType::VertexArray);
            gl::client_state::disable(ClientArrayType::ColorArray);
            gl::client_state::disable(ClientArrayType::TextureCoordArray);
            gl::disable(gl::EnableComponent::ScissorTest);
            gl::enable(gl::EnableComponent::DepthTest);
        }
    }
}
