use anyhow::{Context, ensure};
use hypercubed_core::types::PercentageF32;
use guillotiere::SimpleAtlasAllocator;
use portable_std::prelude::*;
use portable_std::{Arc, FastHashMap};
use serde::{Deserialize, Serialize};

use super::Identifier;

pub use guillotiere::{AllocatorOptions as AtlasAllocatorOptions, size2};

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
    /// RGBA8 format, linear colour (not sRGB).
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

impl Atlas {
    pub fn get_texture<'a>(&'a self, identifier: &Identifier) -> Option<&'a TextureInfo> {
        self.stored_textures.get(identifier)
    }
}

#[cfg(feature = "std")]
pub struct AtlasBuilder {
    texture: RgbaImage,
    space_allocator: SimpleAtlasAllocator,
    stored_textures: FastHashMap<Identifier, TextureInfo>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum TextureInfo {
    Basic { uvs: [u16; 4] },
    Animated(Arc<AnimatedTextureInfo>),
}

impl TextureInfo {
    pub fn basic_or_first_frame(&self) -> [u16; 4] {
        match self {
            Self::Basic { uvs } => *uvs,
            Self::Animated(info) => info.frame_uvs[0],
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AnimatedTextureInfo {
    pub frame_uvs: Box<[[u16; 4]]>,
    pub ticks_per_frame: u16,
    pub smooth_interpolation: bool,
    pub frame_order: Option<Box<[u16]>>,
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

    /// XXX: DEBUG
    const _MAX_DIM_ASSERT_TEST: () = assert!(Self::MAX_DIM < 4096);
    const _MAX_DIM_ASSERT_1: () = assert!(Self::MAX_DIM < i32::MAX as u32);
    const _MAX_DIM_ASSERT_2: () = assert!(Self::MAX_DIM < u16::MAX as u32);

    pub fn new(initial_dims: [u32; 2], options: AtlasAllocatorOptions) -> Self {
        let [width, height] = initial_dims;
        assert!(width <= Self::MAX_DIM);
        assert!(height <= Self::MAX_DIM);
        Self {
            texture: RgbaImage::new(width, height),
            space_allocator: SimpleAtlasAllocator::with_options(
                size2(width.try_into().unwrap(), height.try_into().unwrap()),
                &options,
            ),
            stored_textures: FastHashMap::new(),
        }
    }

    pub fn get_or_load_texture(&mut self, identifier: &Identifier) -> anyhow::Result<TextureInfo> {
        if let Some(texture_info) = self.stored_textures.get(identifier) {
            Ok(texture_info.clone())
        } else {
            let texture_bytes =
                get_resource_file(ResourceType::Texture, identifier).with_context(|| {
                    format!("Failed to read raw image texture data for {identifier:?}")
                })?;
            let raw_texture = image::load_from_memory_with_format(&texture_bytes, ImageFormat::Png)
                .with_context(|| format!("Failed to parse image texture for {identifier:?}"))?
                .into_rgba8();
            ensure!(raw_texture.width() > 0, "Invalid texture size");
            ensure!(raw_texture.height() > 0, "Invalid texture size");
            let meta_file_bytes = get_resource_file(ResourceType::TextureMeta, identifier);
            if let Ok(meta_file_bytes) = meta_file_bytes {
                #[derive(Clone, serde::Deserialize)]
                struct TextureMeta {
                    pub animation: AnimationInfo,
                }
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
                let meta_info = serde_json::from_slice::<TextureMeta>(&meta_file_bytes)
                    .with_context(|| {
                        format!("Failed to parse texture meta info for {identifier:?}")
                    })?;
                let frame_uvs: Box<[[u16; 4]]> = if raw_texture.width() < raw_texture.height() {
                    ensure!(
                        raw_texture.height().is_multiple_of(raw_texture.width()),
                        "Invalid texture size",
                    );
                    let num_frames = raw_texture.height() / raw_texture.width();
                    (0..num_frames)
                        .into_iter()
                        .map(|frame_i| {
                            let frame_width = raw_texture.width();
                            let frame_height = frame_width;
                            let frame_texture_view = raw_texture.view(
                                0,
                                frame_i * frame_height,
                                frame_width,
                                frame_height,
                            );
                            self.stitch_in(&*frame_texture_view)
                        })
                        .collect::<Result<_, StitchError>>()
                        .context("Error while adding animated texture frame")?
                } else {
                    ensure!(
                        raw_texture.width().is_multiple_of(raw_texture.height()),
                        "Invalid texture size",
                    );
                    let num_frames = raw_texture.width() / raw_texture.height();
                    (0..num_frames)
                        .into_iter()
                        .map(|frame_i| {
                            let frame_height = raw_texture.height();
                            let frame_width = frame_height;
                            let frame_texture_view = raw_texture.view(
                                frame_i * frame_width,
                                0,
                                frame_width,
                                frame_height,
                            );
                            self.stitch_in(&*frame_texture_view)
                        })
                        .collect::<Result<_, StitchError>>()
                        .context("Error while adding animated texture frame")?
                };
                let texture_info = TextureInfo::Animated(Arc::new(AnimatedTextureInfo {
                    frame_uvs,
                    ticks_per_frame: meta_info.animation.frametime,
                    smooth_interpolation: meta_info.animation.interpolate,
                    frame_order: meta_info.animation.frames.map(Vec::into_boxed_slice),
                }));
                self.stored_textures
                    .insert(identifier.clone(), texture_info.clone());
                Ok(texture_info)
            } else {
                let uvs = self.stitch_in(&raw_texture)?;
                let texture_info = TextureInfo::Basic { uvs };
                self.stored_textures
                    .insert(identifier.clone(), texture_info.clone());
                Ok(texture_info)
            }
        }
    }

    #[tracing::instrument(skip(self, parts))]
    pub fn load_texture_parts(
        &mut self,
        file_identifier: &Identifier,
        parts: impl IntoIterator<Item = (Identifier, [PercentageF32; 4])>,
    ) -> anyhow::Result<()> {
        let texture_bytes =
            get_resource_file(ResourceType::Texture, file_identifier).with_context(|| {
                format!("Failed to read raw image texture data for {file_identifier:?}")
            })?;
        let raw_texture = image::load_from_memory_with_format(&texture_bytes, ImageFormat::Png)
            .with_context(|| format!("Failed to parse image texture for {file_identifier:?}"))?
            .into_rgba8();
        ensure!(raw_texture.width() > 0, "Invalid texture size");
        ensure!(raw_texture.height() > 0, "Invalid texture size");
        for (part_identifier, part_uv_percentages) in parts {
            ensure!(
                !self.stored_textures.contains_key(&part_identifier),
                "Part already exists in stored textures",
            );
            let start_x = raw_texture.width() * part_uv_percentages[0];
            let start_y = raw_texture.height() * part_uv_percentages[1];
            let end_x = raw_texture.width() * part_uv_percentages[2];
            let end_y = raw_texture.height() * part_uv_percentages[3];
            let texture_part = raw_texture.view(
                start_x,
                start_y,
                end_x - start_x,
                end_y - start_y,
            );
            let part_uvs = self.stitch_in(&*texture_part).context("Error while stitching in texture part")?;
            self.stored_textures.insert(part_identifier, TextureInfo::Basic { uvs: part_uvs });
        }
        Ok(())
    }

    /// Stitches a new texture into the atlas.
    /// Attempts to expand the atlas if a suitable space could not be found.
    #[tracing::instrument(skip_all)]
    fn stitch_in<O: GenericImageView<Pixel = <RgbaImage as GenericImageView>::Pixel>>(
        &mut self,
        texture: &O,
    ) -> Result<[u16; 4], StitchError> {
        let texture_width_i32: i32 = texture
            .width()
            .try_into()
            .map_err(|_| StitchError::InvalidTextureSize)?;
        let texture_height_i32: i32 = texture
            .height()
            .try_into()
            .map_err(|_| StitchError::InvalidTextureSize)?;
        // Find a space, or repeatedly expand until we can find a space.
        let (start_x, start_y) = loop {
            if let Some(rect) = self
                .space_allocator
                .allocate(size2(texture_width_i32, texture_height_i32))
            {
                let start = rect.to_u32().min;
                break (start.x, start.y);
            };
            // Double size, width before height.
            let (old_width, old_height) = self.texture.dimensions();
            let (new_width, new_height) = if old_width <= old_height {
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
            self.space_allocator
                .grow(size2(new_width, new_height).to_i32());
        };
        let end_x = start_x + texture.width();
        let end_y = start_y + texture.height();
        self.texture
            .copy_from(texture, start_x, start_y)?;
        Ok([start_x, start_y, end_x, end_y].map(|v| v as u16))
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
