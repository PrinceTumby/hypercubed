use super::render::{render_egui_mesh, ClipRect};
use ahash::AHashMap;
use image::RgbaImage;
use nalgebra::Point2;

pub struct Renderer {
    images: AHashMap<egui::TextureId, ImageData>,
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
    pub fn sample(&self, u: f32, v: f32) -> image::Rgba<u8> {
        use egui::TextureFilter::*;
        use egui::TextureWrapMode::*;
        let [u, v] = match self.options.wrap_mode {
            ClampToEdge => [u, v].map(|n| n.clamp(0.0, 1.0)),
            Repeat => [u, v].map(|mut n| {
                while n < 0.0 {
                    n += 1.0;
                }
                while n > 1.0 {
                    n -= 1.0;
                }
                n
            }),
            MirroredRepeat => [u, v].map(|mut n| {
                let mut num_flips = 0;
                while n < 0.0 {
                    n += 1.0;
                    num_flips += 1;
                }
                while n > 1.0 {
                    n -= 1.0;
                    num_flips += 1;
                }
                if num_flips % 2 == 1 {
                    1.0 - n
                } else {
                    n
                }
            }),
        };
        image::imageops::sample_nearest(&self.image, u, v)
            .unwrap_or(image::Rgba([0x55, 0x55, 0xFF, 0xFF]))
        // match self.options.magnification {
        //     Nearest => image::imageops::sample_nearest(&self.image, u, v)
        //         .unwrap_or(image::Rgba([0x55, 0x55, 0xFF, 0xFF])),
        //     Linear => image::imageops::sample_bilinear(&self.image, u, v)
        //         .unwrap_or(image::Rgba([0x55, 0x55, 0xFF, 0xFF])),
        // }
    }
}

struct RenderMeshInfo {
    clip_rect: ClipRect,
    vertices: Vec<egui::epaint::Vertex>,
    indices: Vec<u32>,
    texture_id: egui::TextureId,
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

    pub fn render(
        &mut self,
        out_render: &mut impl image::GenericImage<Pixel = image::Rgba<u8>>,
        physical_size: &winit::dpi::PhysicalSize<u32>,
        texture_updates: Vec<(egui::TextureId, egui::epaint::image::ImageDelta)>,
        primitives: Vec<egui::ClippedPrimitive>,
        pixels_per_point: f32,
    ) {
        let width = physical_size.width as f32;
        let height = physical_size.height as f32;
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
            meshes.push(RenderMeshInfo {
                clip_rect: ClipRect {
                    min: Point2::new(
                        (clip_rect.min.x * pixels_per_point).min(width - 1.0),
                        (clip_rect.min.y * pixels_per_point).min(height - 1.0),
                    ),
                    max: Point2::new(
                        (clip_rect.max.x * pixels_per_point).min(width - 1.0),
                        (clip_rect.max.y * pixels_per_point).min(height - 1.0),
                    ),
                },
                vertices: mesh.vertices
                    .into_iter()
                    .map(|v| egui::epaint::Vertex {
                        pos: v.pos * pixels_per_point,
                        ..v
                    })
                    .collect(),
                indices: mesh.indices,
                texture_id: mesh.texture_id,
            });
        }
        // println!("Rendering {} triangles", meshes.iter().map(|mesh| mesh.indices.len() / 3).sum::<usize>());
        for mesh in meshes {
            render_egui_mesh(
                out_render,
                &self.images[&mesh.texture_id],
                mesh.clip_rect,
                &mesh.vertices,
                &mesh.indices,
            );
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
