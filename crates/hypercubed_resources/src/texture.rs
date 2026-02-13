use super::Identifier;
use anyhow::Context;
use portable_std::FastHashMap;
use portable_std::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "std")]
use std_imports::*;
#[cfg(feature = "std")]
mod std_imports {
    pub use crate::manager::{ResourceType, get_resource_file};
    pub use image::error::ImageError;
    pub use image::{GenericImage, GenericImageView, ImageFormat, RgbaImage};
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawTexture {
    pub width: u32,
    pub height: u32,
    /// RGBA8 format.
    pub texture_bytes: Box<[u8]>,
}

#[cfg(feature = "std")]
impl RawTexture {
    pub fn from_image(img: RgbaImage) -> Self {
        Self {
            width: img.width(),
            height: img.height(),
            texture_bytes: img.into_vec().into_boxed_slice(),
        }
    }

    pub fn load_from_resource(identifier: &Identifier) -> anyhow::Result<Self> {
        let texture_bytes = get_resource_file(ResourceType::Texture, identifier)
            .with_context(|| format!("Failed to read raw image texture data for {identifier:?}"))?;
        let texture = image::load_from_memory_with_format(&texture_bytes, ImageFormat::Png)
            .with_context(|| format!("Failed to parse image texture for {identifier:?}"))?
            .into_rgba8();
        Ok(Self::from_image(texture))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Atlas {
    pub width: u32,
    pub height: u32,
    /// RGBA8 format.
    pub texture_bytes: Box<[u8]>,
    pub stored_textures: FastHashMap<Identifier, TextureInfo>,
}

impl core::ops::Index<(u32, u32)> for Atlas {
    type Output = [u8; 4];

    fn index(&self, (x, y): (u32, u32)) -> &[u8; 4] {
        let start_pixel_idx = (y * self.width) + x;
        let start_byte_idx = start_pixel_idx * 4;
        self.texture_bytes[start_byte_idx as usize..][..4]
            .as_array()
            .unwrap()
    }
}

#[cfg(feature = "std")]
#[derive(Clone, Debug)]
pub struct AtlasBuilder {
    texture: RgbaImage,
    square_length: u16,
    usage_bitmap: UsageBitmap2d,
    stored_textures: FastHashMap<Identifier, TextureInfo>,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct TextureInfo {
    pub uvs: [u16; 4],
    pub space_dims: [u16; 2],
}

#[cfg(feature = "std")]
#[derive(Debug, thiserror::Error)]
pub enum StitchError {
    #[error("invalid texture size")]
    InvalidTextureSize,
    #[error("error during expansion - `{0}`")]
    ExpandError(ImageError),
    #[error("out of texture space")]
    OutOfSpace,
    #[error("image error - `{0}`")]
    ImageError(#[from] ImageError),
}

#[cfg(feature = "std")]
impl AtlasBuilder {
    pub const MAX_DIM: u32 = 32768;

    pub fn new(square_length: u16) -> Self {
        assert!(square_length.is_power_of_two());
        let initial_pixel_dim = square_length as u32 * 8;
        assert!(initial_pixel_dim <= Self::MAX_DIM);
        let initial_usage_bitmap_dim: u16 = (initial_pixel_dim / square_length as u32)
            .try_into()
            .unwrap();
        Self {
            texture: RgbaImage::new(initial_pixel_dim, initial_pixel_dim),
            square_length,
            usage_bitmap: UsageBitmap2d::new(initial_usage_bitmap_dim, initial_usage_bitmap_dim),
            stored_textures: FastHashMap::new(),
        }
    }

    #[inline]
    pub fn square_length(&self) -> u16 {
        self.square_length
    }

    pub fn get_or_load_texture(&mut self, identifier: &Identifier) -> anyhow::Result<TextureInfo> {
        if let Some(&texture_info) = self.stored_textures.get(identifier) {
            Ok(texture_info)
        } else {
            let texture_bytes =
                get_resource_file(ResourceType::Texture, identifier).with_context(|| {
                    format!("Failed to read raw image texture data for {identifier:?}")
                })?;
            let texture = image::load_from_memory_with_format(&texture_bytes, ImageFormat::Png)
                .with_context(|| format!("Failed to parse image texture for {identifier:?}"))?
                .into_rgba8();
            let texture_info = self.stitch_in(&texture)?;
            let animation_bytes = get_resource_file(ResourceType::TextureMeta, identifier);
            if let Ok(animation_bytes) = animation_bytes {
                #[expect(unused)]
                #[derive(Clone, serde::Deserialize)]
                struct TextureMeta {
                    pub animation: AnimationInfo,
                }
                #[expect(unused)]
                #[derive(Clone, serde::Deserialize)]
                struct AnimationInfo {
                    #[serde(default = "default_frametime")]
                    pub frametime: u16,
                    #[serde(default)]
                    pub interpolate: bool,
                    pub frames: Option<Vec<u16>>,
                }
                fn default_frametime() -> u16 {
                    20
                }
                let _info =
                    serde_json::from_slice::<TextureMeta>(&animation_bytes).with_context(|| {
                        format!("Failed to parse texture meta info for {identifier:?}")
                    })?;
                // TODO: Support texture animation
                let old_dims = texture_info.space_dims;
                let uvs = texture_info.uvs;
                let new_texture_info = if old_dims[0] < old_dims[1] {
                    let uv_diff = uvs[3] - uvs[1];
                    let new_uv = uvs[1] + (uv_diff / (old_dims[1] / old_dims[0]));
                    TextureInfo {
                        uvs: [uvs[0], uvs[1], uvs[2], new_uv],
                        space_dims: old_dims,
                    }
                } else {
                    let uv_diff = uvs[2] - uvs[0];
                    let new_uv = uvs[0] + (uv_diff / (old_dims[0] / old_dims[1]));
                    TextureInfo {
                        uvs: [uvs[0], uvs[1], new_uv, uvs[3]],
                        space_dims: old_dims,
                    }
                };
                self.stored_textures
                    .insert(identifier.clone(), new_texture_info);
                Ok(new_texture_info)
            } else {
                self.stored_textures
                    .insert(identifier.clone(), texture_info);
                Ok(texture_info)
            }
        }
    }

    /// Stitches a new texture into the atlas.
    /// Attempts to expand the atlas if a suitable space could not be found.
    fn stitch_in<O: GenericImageView<Pixel = <RgbaImage as GenericImageView>::Pixel>>(
        &mut self,
        texture: &O,
    ) -> Result<TextureInfo, StitchError> {
        let texture_width: u16 = texture.width().try_into().unwrap();
        let texture_height: u16 = texture.height().try_into().unwrap();
        if !texture_width.is_multiple_of(self.square_length)
            || !texture_height.is_multiple_of(self.square_length)
        {
            return Err(StitchError::InvalidTextureSize);
        }
        let space_width = texture_width / self.square_length;
        let space_height = texture_height / self.square_length;
        // Find a space, or repeatedly expand until we can find a space.
        let (space_x, space_y) = loop {
            if let Some((space_x, space_y)) = self
                .usage_bitmap
                .try_reserve_space(space_width, space_height)
            {
                break (space_x, space_y);
            };
            // Double size, height before width.
            // We expand height first as a number of animated textures use vertically stacked
            // textures.
            let (old_width, old_height) = self.texture.dimensions();
            let (new_width, new_height) = if old_width < old_height {
                (old_width * 2, old_height)
            } else {
                (old_width, old_height * 2)
            };
            if new_width > Self::MAX_DIM || new_height > Self::MAX_DIM {
                return Err(StitchError::OutOfSpace);
            }
            let mut new_texture = RgbaImage::new(new_width, new_height);
            new_texture
                .copy_from(&self.texture, 0, 0)
                .map_err(StitchError::ExpandError)?;
            self.texture = new_texture;
            self.usage_bitmap.expand(
                (new_width / self.square_length as u32).try_into().unwrap(),
                (new_height / self.square_length as u32).try_into().unwrap(),
            );
        };
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

    pub fn finish(self) -> Atlas {
        Atlas {
            width: self.texture.width(),
            height: self.texture.height(),
            texture_bytes: self.texture.into_vec().into_boxed_slice(),
            stored_textures: self.stored_textures,
        }
    }
}

#[cfg(feature = "std")]
#[derive(Clone, Debug)]
struct UsageBitmap2d {
    bytes: Vec<u8>,
    width: u16,
    height: u16,
}

#[cfg(feature = "std")]
impl UsageBitmap2d {
    pub fn new(width: u16, height: u16) -> Self {
        // Currently only works with multiple of 8 dimensions
        assert!(
            width.is_multiple_of(8),
            "usage bitmap create width {width} is not a multiple of 8"
        );
        assert!(
            height.is_multiple_of(8),
            "usage bitmap create height {height} is not a multiple of 8"
        );
        Self {
            bytes: vec![0; (width as usize / 8) * height as usize],
            width,
            height,
        }
    }

    pub fn expand(&mut self, new_width: u16, new_height: u16) {
        assert!(new_width.is_multiple_of(8));
        assert!(new_height.is_multiple_of(8));
        assert!(new_width >= self.width);
        assert!(new_height >= self.height);
        let mut new_self = Self::new(new_width, new_height);
        // Copy usage info to new bitmap.
        for y in 0..self.height {
            for x in 0..self.width {
                new_self.set(x, y, self.get(x, y));
            }
        }
        *self = new_self;
    }

    pub fn try_reserve_space(&mut self, width: u16, height: u16) -> Option<(u16, u16)> {
        assert!(0 < width);
        assert!(0 < height);
        // If we're trying to reserve a space larger than our dimensions, then it's definitely not
        // going to fit until we've expanded.
        if width > self.width || height > self.height {
            return None;
        }
        // Specialize strategy based on space dimensions.
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
            (_, _) => {
                // Loop through all possible rectangles.
                for start_x in 0..self.width - width {
                    'outer: for start_y in 0..self.height - height {
                        // Check each space in rectangle.
                        for rect_x in start_x..start_x + width {
                            for rect_y in start_y..start_y + height {
                                if self.get(rect_x, rect_y) {
                                    continue 'outer;
                                }
                            }
                        }
                        // If we find a suitable rectangle, reserve and return coordinates.
                        for rect_x in start_x..start_x + width {
                            for rect_y in start_y..start_y + height {
                                self.set(rect_x, rect_y, true);
                            }
                        }
                        return Some((start_x, start_y));
                    }
                }
                // Report failure if we couldn't find a free rectangle.
                None
            }
        }
    }

    #[inline(always)]
    fn get(&self, x: u16, y: u16) -> bool {
        let byte = self.bytes[((y * self.width) / 8 + (x / 8)) as usize];
        let bit_idx = x % 8;
        byte & (1 << bit_idx) != 0
    }

    /// Panics if the coordinates are out of range.
    #[inline(always)]
    fn set(&mut self, x: u16, y: u16, value: bool) {
        let byte = &mut self.bytes[((y * self.width) / 8 + (x / 8)) as usize];
        let bit_idx = x % 8;
        let bit_mask = (value as u8) << bit_idx;
        *byte &= !bit_mask;
        *byte |= bit_mask;
    }
}
