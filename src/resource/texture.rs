use super::Identifier;
use crate::resource::manager::{get_resource_file, ResourceType};
use ahash::AHashMap;
use anyhow::Context;
use image::error::ImageError;
use image::{GenericImage, GenericImageView, ImageFormat, RgbaImage};
use thiserror::Error;

#[derive(Debug)]
pub struct Atlas {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

#[derive(Clone, Debug)]
pub struct AtlasBuilder {
    texture: RgbaImage,
    square_length: u16,
    usage_bitmap: UsageBitmap2d,
    stored_textures: AHashMap<Identifier, TextureInfo>,
}

#[derive(Clone, Copy, Debug)]
pub struct TextureInfo {
    pub uvs: [u16; 4],
    pub space_dims: [u16; 2],
}

#[derive(Debug, Error)]
pub enum StitchError {
    #[error("invalid texture size")]
    InvalidTextureSize,
    #[error("out of space")]
    OutOfSpace,
    #[error("image error: `{0}`")]
    ImageError(#[from] ImageError),
}

impl AtlasBuilder {
    pub fn new(pixel_width: u32, pixel_height: u32, square_length: u16) -> Self {
        assert!(pixel_width < 65536);
        assert!(pixel_height < 65536);
        assert_eq!(pixel_width % square_length as u32, 0);
        assert_eq!(pixel_height % square_length as u32, 0);
        assert_eq!(65536 % pixel_width, 0);
        assert_eq!(65536 % pixel_height, 0);
        Self {
            texture: RgbaImage::new(pixel_width, pixel_height),
            square_length,
            usage_bitmap: UsageBitmap2d::new(
                (pixel_width / square_length as u32).try_into().unwrap(),
                (pixel_height / square_length as u32).try_into().unwrap(),
            ),
            stored_textures: AHashMap::new(),
        }
    }

    #[inline]
    pub fn square_length(&self) -> u16 {
        self.square_length
    }

    pub fn get_or_load_texture(&mut self, location: &Identifier) -> anyhow::Result<TextureInfo> {
        if let Some(&texture_info) = self.stored_textures.get(location) {
            Ok(texture_info)
        } else {
            let texture_bytes =
                get_resource_file(&ResourceType::Texture, location).with_context(|| {
                    format!("Failed to read raw image texture data for {location:?}")
                })?;
            let texture = image::load_from_memory_with_format(&texture_bytes, ImageFormat::Png)
                .with_context(|| format!("Failed to parse image texture for {location:?}"))?
                .into_rgba8();
            let texture_info = self.stitch_in(&texture)?;
            self.stored_textures.insert(location.clone(), texture_info);
            Ok(texture_info)
        }
    }

    /// Stitches a texture into the atlas, returning UV fractions `[1/U, 1/V]`
    fn stitch_in<O>(&mut self, texture: &O) -> Result<TextureInfo, StitchError>
    where
        O: GenericImageView<Pixel = <RgbaImage as GenericImageView>::Pixel>,
    {
        let texture_width: u16 = texture.width().try_into().unwrap();
        let texture_height: u16 = texture.height().try_into().unwrap();
        if texture.width() > self.texture.width() || texture_width % self.square_length != 0 {
            return Err(StitchError::InvalidTextureSize);
        }
        if texture.height() > self.texture.height() || texture_height % self.square_length != 0 {
            return Err(StitchError::InvalidTextureSize);
        }
        let space_width = texture_width / self.square_length;
        let space_height = texture_height / self.square_length;
        let (space_x, space_y) = self
            .usage_bitmap
            .reserve_space(space_width, space_height)
            .ok_or(StitchError::OutOfSpace)?;
        let (start_x, start_y) = (space_x * self.square_length, space_y * self.square_length);
        let end_x = start_x + texture_width;
        let end_y = start_y + texture_height;
        self.texture
            .copy_from(texture, start_x as u32, start_y as u32)?;
        Ok(TextureInfo {
            uvs: [start_x, start_y, end_x, end_y],
            space_dims: [space_width, space_height],
        })
    }

    pub fn build(self, device: &wgpu::Device, queue: &wgpu::Queue, label: Option<&str>) -> Atlas {
        let (width, height) = (self.texture.width(), self.texture.height());
        let bytes = self.texture.into_vec();
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::ImageCopyTexture {
                aspect: wgpu::TextureAspect::All,
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            &bytes,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            size,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
        Atlas {
            texture,
            view,
            sampler,
        }
    }
}

#[derive(Clone, Debug)]
struct UsageBitmap2d {
    bytes: Vec<u8>,
    width: u16,
    height: u16,
}

impl UsageBitmap2d {
    pub fn new(width: u16, height: u16) -> Self {
        // Currently only works with multiple of 8 dimensions
        assert_eq!(width % 8, 0);
        assert_eq!(height % 8, 0);
        Self {
            bytes: vec![0; (width as usize / 8) * height as usize],
            width,
            height,
        }
    }

    pub fn reserve_space(&mut self, width: u16, height: u16) -> Option<(u16, u16)> {
        assert!(0 < width && width <= self.width);
        assert!(0 < height && height <= self.height);
        // Specialize strategy based on space dimensions
        match (width, height) {
            (0, 0) => unreachable!(),
            (1, 1) => {
                let coords = self.bytes.iter().enumerate().find_map(|(i, &byte)| {
                    match byte.count_zeros() {
                        0 => None,
                        1.. => {
                            let bit_idx = byte.trailing_ones() as u16;
                            let row_stride = self.width / 8;
                            Some((
                                ((i as u16 % row_stride) * 8) + bit_idx,
                                i as u16 / row_stride,
                            ))
                        }
                    }
                })?;
                self.set(coords.0, coords.1, true);
                Some(coords)
            }
            (2, 1) => {
                let start_coords =
                    self.bytes.iter().enumerate().find_map(|(i, &byte)| {
                        match byte.count_zeros() {
                            2.. => {
                                let start_bit_idx = match byte {
                                    _ if byte & 0b00000011 == 0b00000011 => 0,
                                    _ if byte & 0b00000110 == 0b00000110 => 1,
                                    _ if byte & 0b00001100 == 0b00001100 => 2,
                                    _ if byte & 0b00011000 == 0b00011000 => 3,
                                    _ if byte & 0b00110000 == 0b00110000 => 4,
                                    _ if byte & 0b01100000 == 0b01100000 => 5,
                                    _ if byte & 0b11000000 == 0b11000000 => 6,
                                    _ => return None,
                                };
                                let row_stride = self.width / 8;
                                Some((
                                    ((i as u16 % row_stride) * 8) + start_bit_idx,
                                    i as u16 / row_stride,
                                ))
                            }
                            _ => None,
                        }
                    })?;
                self.set(start_coords.0, start_coords.1, true);
                self.set(start_coords.0 + 1, start_coords.1, true);
                Some(start_coords)
            }
            (_, 1) => todo!(),
            (_, _) => {
                // Loop through all possible rectangles
                for start_x in 0..self.width - width {
                    'outer: for start_y in 0..self.height - height {
                        // Check each space in rectangle
                        for rect_x in start_x..start_x + width {
                            for rect_y in start_y..start_y + height {
                                if self.get(rect_x, rect_y) {
                                    continue 'outer;
                                }
                            }
                        }
                        // Found suitable rectangle, reserve and return coordinates
                        for rect_x in start_x..start_x + width {
                            for rect_y in start_y..start_y + height {
                                self.set(rect_x, rect_y, true);
                            }
                        }
                        return Some((start_x, start_y));
                    }
                }
                unreachable!()
            }
        }
    }

    #[inline]
    fn get(&self, x: u16, y: u16) -> bool {
        let byte = self.bytes[((y * self.width) / 8 + (x / 8)) as usize];
        let bit_idx = x % 8;
        byte & (1 << bit_idx) != 0
    }

    /// Panics if the coordinates are out of range
    #[inline]
    fn set(&mut self, x: u16, y: u16, value: bool) {
        let byte = &mut self.bytes[((y * self.width) / 8 + (x / 8)) as usize];
        let bit_idx = x % 8;
        let bit_mask = (value as u8) << bit_idx;
        *byte &= !bit_mask;
        *byte |= bit_mask;
    }
}
