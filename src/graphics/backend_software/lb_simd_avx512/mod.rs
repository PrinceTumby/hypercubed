pub mod chunk;
pub mod egui_rendering;
pub mod types;

use super::TextureAtlas;
use core::marker::PhantomData;
use core::num::NonZeroU32;
use core::num::{NonZeroU8, NonZeroU16};
use rayon::prelude::*;
use resources::texture::Atlas;
use types::*;

/// RGBA8 pixel stored as `u32::from_ne_bytes([r, g, b, a])`.
type Rgba8Ne = u32;

/// Dimension length of a pixel group in pixels.
const RENDER_PIXEL_GROUP_DIM: usize = 4;
/// Dimension length of a micro-tile in pixel group lengths.
const RENDER_MICRO_TILE_DIM: usize = 4;
/// Dimension length of a micro-tile in pixels.
const RENDER_MICRO_TILE_PIXEL_DIM: usize = RENDER_MICRO_TILE_DIM * RENDER_PIXEL_GROUP_DIM;
/// Dimension length of a tile in micro-tile lengths.
const RENDER_TILE_DIM: usize = 4;
/// Dimension length of a tile in pixels.
const RENDER_TILE_PIXEL_DIM: usize = RENDER_TILE_DIM * RENDER_MICRO_TILE_PIXEL_DIM;

// type RenderPixelGroupRgba = [Rgba8Ne; RENDER_PIXEL_GROUP_DIM * RENDER_PIXEL_GROUP_DIM];
type RenderPixelGroupRgba = u32x16;
type RenderMicroTileRgba = [[RenderPixelGroupRgba; RENDER_MICRO_TILE_DIM]; RENDER_MICRO_TILE_DIM];
type RenderTileRgba = [[RenderMicroTileRgba; RENDER_TILE_DIM]; RENDER_TILE_DIM];

/// Flattened 4x4 pixel group of pixel depths.
type RenderPixelGroupDepth = f32x16;
/// Flattened 4x4 micro-tile of pixel groups.
type RenderMicroTileDepth = [RenderPixelGroupDepth; RENDER_MICRO_TILE_DIM * RENDER_MICRO_TILE_DIM];
/// Flattened 4x4 tile of micro-tiles.
type RenderTileDepth = [RenderMicroTileDepth; RENDER_TILE_DIM * RENDER_TILE_DIM];

#[repr(align(16))]
struct RenderTileHiZChain {
    pub pixel: RenderTileDepth,
    pub pixel_group: RenderMicroTileDepth,
    pub micro_tile: RenderPixelGroupDepth,
    pub tile: f32,
}

impl RenderTileHiZChain {
    pub const fn zeroed() -> Self {
        Self {
            pixel: [[f32x16::from_array([0.0; 16]); 16]; 16],
            pixel_group: [f32x16::from_array([0.0; 16]); 16],
            micro_tile: f32x16::from_array([0.0; 16]),
            tile: 0.0,
        }
    }
}

#[inline(always)]
fn rgba_u32x16_to_unorm8x16(rgbas: u32x16) -> [unorm8x16; 4] {
    // x86 is always little-endian.
    let [reds, greens, blues, alphas]: [u8x16; 4] = [
        (rgbas & 0xFF).cast_u8(),
        ((rgbas >> 8) & 0xFF).cast_u8(),
        ((rgbas >> 16) & 0xFF).cast_u8(),
        ((rgbas >> 24) & 0xFF).cast_u8(),
    ];
    [reds, greens, blues, alphas].map(unorm8x16)
}

#[inline(always)]
fn rgba_4xunorm8x16_to_u32x16(rgbas: [unorm8x16; 4]) -> u32x16 {
    let [reds_u32, greens_u32, blues_u32, alphas_u32] = rgbas.map(|unorms| unorms.0.cast_u32());
    // x86 is always little-endian.
    reds_u32 | (greens_u32 << 8) | (blues_u32 << 16) | (alphas_u32 << 24)
}

pub struct LinearFramebufferRgba<'a> {
    data: *mut u8,
    marker: PhantomData<&'a mut [u8]>,
    width: NonZeroU32,
    height: NonZeroU32,
}

unsafe impl Sync for LinearFramebufferRgba<'_> {}

unsafe impl Send for LinearFramebufferRgba<'_> {}

impl<'a> LinearFramebufferRgba<'a> {
    pub fn from_raw(data: &'a mut [u8], width: u32, height: u32) -> Self {
        Self {
            data: data.as_mut_ptr(),
            marker: PhantomData,
            width: width.try_into().unwrap(),
            height: height.try_into().unwrap(),
        }
    }
}

pub struct RenderTileBins {
    bins: Box<[RenderTileBin]>,
    pub(super) tiles_per_row: NonZeroU32,
    pub(super) width: NonZeroU32,
    pub(super) height: NonZeroU32,
}

#[derive(Clone, Debug)]
pub struct RenderTileBin {
    egui_draw_cmds: Vec<egui_rendering::TileDrawCommand>,
    chunk_draw_cmds: Vec<chunk::TileDrawCommand>,
}

impl RenderTileBin {
    pub const fn empty() -> Self {
        Self {
            egui_draw_cmds: Vec::new(),
            chunk_draw_cmds: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.egui_draw_cmds.clear();
        self.chunk_draw_cmds.clear();
    }
}

impl RenderTileBins {
    #[tracing::instrument(name = "RenderTileBins::new")]
    pub fn new(width: NonZeroU32, height: NonZeroU32) -> Self {
        let width_tiles = width.div_ceil(NonZeroU32::new(RENDER_TILE_PIXEL_DIM as u32).unwrap());
        let height_tiles = height.div_ceil(NonZeroU32::new(RENDER_TILE_PIXEL_DIM as u32).unwrap());
        let num_tiles = (width_tiles.get() * height_tiles.get()) as usize;
        Self {
            bins: vec![RenderTileBin::empty(); num_tiles].into_boxed_slice(),
            tiles_per_row: width_tiles,
            width,
            height,
        }
    }

    pub fn clear(&mut self) {
        for bin in &mut self.bins {
            bin.clear();
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct Rgba(pub f32x4);

impl Rgba {
    #[inline(always)]
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self(f32x4::from_array([r, g, b, a]))
    }

    #[inline(always)]
    pub fn from_array(array: [f32; 4]) -> Self {
        Self(f32x4::from_array(array))
    }

    #[inline(always)]
    pub fn r(&self) -> f32 {
        self.0.to_array()[0]
    }

    #[inline(always)]
    pub fn g(&self) -> f32 {
        self.0.to_array()[1]
    }

    #[inline(always)]
    pub fn b(&self) -> f32 {
        self.0.to_array()[2]
    }

    #[inline(always)]
    pub fn a(&self) -> f32 {
        self.0.to_array()[3]
    }

    #[inline(always)]
    pub fn half_blend(&self, rgb_coef: f32, a_coef: f32) -> Self {
        let coefs = f32x4::from_array([rgb_coef, rgb_coef, rgb_coef, a_coef]);
        Self(self.0 * coefs)
    }

    #[inline(always)]
    pub fn from_rgb8(value: [u8; 3]) -> Self {
        Self::from_rgba8([value[0], value[1], value[2], 0xFF])
    }

    #[inline(always)]
    pub fn from_rgba8(value: [u8; 4]) -> Self {
        Self::from_array(value.map(|n| (n as f32) * (1.0 / 255.0)))
    }

    #[inline(always)]
    pub fn from_image_rgba8(value: image::Rgba<u8>) -> Self {
        Self::from_rgba8(value.0)
    }

    #[inline(always)]
    pub fn to_image_rgba8(self) -> image::Rgba<u8> {
        image::Rgba(self.0.to_array().map(|n| (n.clamp(0.0, 1.0) * 255.0) as u8))
    }

    #[inline(always)]
    pub fn to_rgba8ne(self) -> Rgba8Ne {
        u32::from_ne_bytes(
            #[allow(clippy::manual_clamp)]
            self.0
                .to_array()
                .map(|n| (n.max(0.0).min(1.0) * 255.0) as u8),
        )
    }

    #[inline(always)]
    pub fn mul_scalar_then_add(self, m: f32, a: Self) -> Self {
        Self(self.0.mul_add(f32x4::splat(m), a.0))
    }

    #[inline(always)]
    pub fn mul_scalar_then_neg_add(self, m: f32, a: Self) -> Self {
        Self(self.0.mul_neg_add(f32x4::splat(m), a.0))
    }
}

impl core::ops::Add for Rgba {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl core::ops::Sub for Rgba {
    type Output = Self;

    #[inline(always)]
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl core::ops::Mul for Rgba {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}

impl core::ops::Mul<f32> for Rgba {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: f32) -> Self::Output {
        Self(self.0 * f32x4::splat(rhs))
    }
}

impl core::ops::Div<f32> for Rgba {
    type Output = Self;

    #[inline(always)]
    fn div(self, rhs: f32) -> Self::Output {
        Self(self.0 / f32x4::splat(rhs))
    }
}

impl core::ops::AddAssign for Rgba {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

/// Dimension length of a micro-tile in pixels.
const TEXTURE_MICRO_TILE_DIM: usize = 4;
/// Dimension length of a tile in micro-tile lengths.
const TEXTURE_TILE_DIM: usize = 8;
/// Dimension length of a tile in pixels.
const TEXTURE_TILE_PIXEL_DIM: usize = TEXTURE_TILE_DIM * TEXTURE_MICRO_TILE_DIM;

type TextureMicroTileRgba = [[Rgba8Ne; TEXTURE_MICRO_TILE_DIM]; TEXTURE_MICRO_TILE_DIM];
type TextureTileRgba = [[TextureMicroTileRgba; TEXTURE_TILE_DIM]; TEXTURE_TILE_DIM];

const MICRO_TILE_BLACK: TextureMicroTileRgba = [[0; _]; _];
const TILE_BLACK: TextureTileRgba = [[MICRO_TILE_BLACK; _]; _];

pub struct TiledTextureRgba {
    tiles: Box<[TextureTileRgba]>,
    power_of_two_tiles_per_row: NonZeroU8,
    width: NonZeroU16,
    height: NonZeroU16,
}

impl TiledTextureRgba {
    pub fn from_image(image_buffer: &image::RgbaImage) -> anyhow::Result<Self> {
        anyhow::ensure!(
            (1..32768).contains(&image_buffer.width())
                && (1..32768).contains(&image_buffer.height()),
            "Unsupported texture dims - {}x{}",
            image_buffer.width(),
            image_buffer.height(),
        );
        let width_tiles = (image_buffer.width() as usize)
            .div_ceil(TEXTURE_TILE_PIXEL_DIM)
            .next_power_of_two() as u16;
        let height_tiles = (image_buffer.height() as usize)
            .div_ceil(TEXTURE_TILE_PIXEL_DIM)
            .next_power_of_two() as u16;
        let mut tiles = vec![TILE_BLACK; width_tiles as usize * height_tiles as usize];
        for tile_y in 0..height_tiles {
            for tile_x in 0..width_tiles {
                let tile = &mut tiles[tile_y as usize * width_tiles as usize + tile_x as usize];
                for (micro_tile_y, micro_tile_row) in tile.iter_mut().enumerate() {
                    for (micro_tile_x, micro_tile) in micro_tile_row.iter_mut().enumerate() {
                        for (micro_pixel_y, micro_pixel_row) in micro_tile.iter_mut().enumerate() {
                            for (micro_pixel_x, micro_pixel) in
                                micro_pixel_row.iter_mut().enumerate()
                            {
                                let pixel_x = (tile_x as usize * TEXTURE_TILE_PIXEL_DIM)
                                    + (micro_tile_x * TEXTURE_MICRO_TILE_DIM)
                                    + micro_pixel_x;
                                let pixel_y = (tile_y as usize * TEXTURE_TILE_PIXEL_DIM)
                                    + (micro_tile_y * TEXTURE_MICRO_TILE_DIM)
                                    + micro_pixel_y;
                                let pixel_x_u32: u32 = pixel_x.try_into().unwrap();
                                let pixel_y_u32: u32 = pixel_y.try_into().unwrap();
                                let colour = image_buffer[(pixel_x_u32, pixel_y_u32)];
                                *micro_pixel = u32::from_ne_bytes(colour.0);
                            }
                        }
                    }
                }
            }
        }
        Ok(Self {
            tiles: tiles.into_boxed_slice(),
            // NOTE: The safety of `Self::sample_nearest` depends on this being accurate.
            power_of_two_tiles_per_row: NonZeroU8::new(width_tiles.ilog2().try_into().unwrap())
                .unwrap(),
            width: NonZeroU16::new(image_buffer.width().try_into().unwrap()).unwrap(),
            height: NonZeroU16::new(image_buffer.height().try_into().unwrap()).unwrap(),
        })
    }

    pub fn from_atlas(atlas: &Atlas) -> anyhow::Result<Self> {
        anyhow::ensure!(
            (1..32768).contains(&atlas.width) && (1..32768).contains(&atlas.height),
            "Unsupported texture dims - {}x{}",
            atlas.width,
            atlas.height,
        );
        let width_tiles = (atlas.width as usize)
            .div_ceil(TEXTURE_TILE_PIXEL_DIM)
            .next_power_of_two() as u16;
        let height_tiles = (atlas.height as usize)
            .div_ceil(TEXTURE_TILE_PIXEL_DIM)
            .next_power_of_two() as u16;
        let mut tiles = vec![TILE_BLACK; width_tiles as usize * height_tiles as usize];
        for tile_y in 0..height_tiles {
            for tile_x in 0..width_tiles {
                let tile = &mut tiles[tile_y as usize * width_tiles as usize + tile_x as usize];
                for (micro_tile_y, micro_tile_row) in tile.iter_mut().enumerate() {
                    for (micro_tile_x, micro_tile) in micro_tile_row.iter_mut().enumerate() {
                        for (micro_pixel_y, micro_pixel_row) in micro_tile.iter_mut().enumerate() {
                            for (micro_pixel_x, micro_pixel) in
                                micro_pixel_row.iter_mut().enumerate()
                            {
                                let pixel_x = (tile_x as usize * TEXTURE_TILE_PIXEL_DIM)
                                    + (micro_tile_x * TEXTURE_MICRO_TILE_DIM)
                                    + micro_pixel_x;
                                let pixel_y = (tile_y as usize * TEXTURE_TILE_PIXEL_DIM)
                                    + (micro_tile_y * TEXTURE_MICRO_TILE_DIM)
                                    + micro_pixel_y;
                                let pixel_x_u32: u32 = pixel_x.try_into().unwrap();
                                let pixel_y_u32: u32 = pixel_y.try_into().unwrap();
                                let colour = atlas[(pixel_x_u32, pixel_y_u32)];
                                *micro_pixel = u32::from_ne_bytes(colour);
                            }
                        }
                    }
                }
            }
        }
        Ok(Self {
            tiles: tiles.into_boxed_slice(),
            // NOTE: The safety of `Self::sample_nearest` depends on this being accurate.
            power_of_two_tiles_per_row: NonZeroU8::new(width_tiles.ilog2().try_into().unwrap())
                .unwrap(),
            width: NonZeroU16::new(atlas.width.try_into().unwrap()).unwrap(),
            height: NonZeroU16::new(atlas.height.try_into().unwrap()).unwrap(),
        })
    }

    #[inline(always)]
    pub fn sample_nearest(&self, u: f32, v: f32) -> Rgba {
        // Convert UV coords to texel coords.
        // Using `min` and `max` instead of `clamp` converts NaNs.
        let x = (self.width.get() as f32 * u - 0.5)
            .round()
            .max(0.0)
            .min((self.width.get() - 1) as f32) as usize;
        let y = (self.height.get() as f32 * v - 0.5)
            .round()
            .max(0.0)
            .min((self.height.get() - 1) as f32) as usize;
        let tile_x = x / TEXTURE_TILE_PIXEL_DIM;
        let tile_y = y / TEXTURE_TILE_PIXEL_DIM;
        let x_in_tile = x % TEXTURE_TILE_PIXEL_DIM;
        let y_in_tile = y % TEXTURE_TILE_PIXEL_DIM;
        let power_of_two_tiles_per_row = self.power_of_two_tiles_per_row.get() as usize;
        cfg_if::cfg_if! {
            if #[cfg(debug_assertions)] {
                let tile = &self.tiles[(tile_y << power_of_two_tiles_per_row) | tile_x];
            } else {
                // SAFETY: The tile X and Y coordinates are clamped, and
                // `power_of_two_tiles_per_row` must be set to be accurate by `Self::from_image`,
                // so the calculated index will never be out of bounds.
                let tile = unsafe {
                    self.tiles
                        .get_unchecked((tile_y << power_of_two_tiles_per_row) | tile_x)
                };
            }
        }
        let micro_tile_x = x_in_tile / TEXTURE_MICRO_TILE_DIM;
        let micro_tile_y = y_in_tile / TEXTURE_MICRO_TILE_DIM;
        let x_in_micro_tile = x_in_tile % TEXTURE_MICRO_TILE_DIM;
        let y_in_micro_tile = y_in_tile % TEXTURE_MICRO_TILE_DIM;
        let micro_tile = &tile[micro_tile_y][micro_tile_x];
        let rgba_u32 = micro_tile[y_in_micro_tile][x_in_micro_tile];
        Rgba::from_array(rgba_u32.to_ne_bytes().map(|n| n as f32 / 255.0))
    }

    #[inline(always)]
    pub fn sample_bilinear(&self, u: f32, v: f32) -> Rgba {
        // Convert UV coords to texel coords.
        // Using `min` and `max` instead of `clamp` converts NaNs.
        let x_f32 = (self.width.get() as f32 * u - 0.5)
            .max(0.0)
            .min((self.width.get() - 1) as f32);
        let y_f32 = (self.height.get() as f32 * v - 0.5)
            .max(0.0)
            .min((self.height.get() - 1) as f32);
        let (x_floor_diff, x_ceil_diff) = if x_f32.floor() == x_f32.ceil() {
            (0.0, 1.0)
        } else {
            (x_f32 - x_f32.floor(), x_f32.ceil() - x_f32)
        };
        let (y_floor_diff, y_ceil_diff) = if y_f32.floor() == y_f32.ceil() {
            (0.0, 1.0)
        } else {
            (y_f32 - y_f32.floor(), y_f32.ceil() - y_f32)
        };
        let sample_weights = [
            x_ceil_diff * y_ceil_diff,
            x_floor_diff * y_ceil_diff,
            x_ceil_diff * y_floor_diff,
            x_floor_diff * y_floor_diff,
        ];
        let sample_coords = [
            (x_f32.floor() as u16, y_f32.floor() as u16),
            (x_f32.ceil() as u16, y_f32.floor() as u16),
            (x_f32.floor() as u16, y_f32.ceil() as u16),
            (x_f32.ceil() as u16, y_f32.ceil() as u16),
        ];
        let rgba_samples = sample_coords.map(|(x, y)| {
            let tile_x = x as usize / TEXTURE_TILE_PIXEL_DIM;
            let tile_y = y as usize / TEXTURE_TILE_PIXEL_DIM;
            let x_in_tile = x as usize % TEXTURE_TILE_PIXEL_DIM;
            let y_in_tile = y as usize % TEXTURE_TILE_PIXEL_DIM;
            let power_of_two_tiles_per_row = self.power_of_two_tiles_per_row.get() as usize;
            cfg_if::cfg_if! {
                if #[cfg(debug_assertions)] {
                    let tile = &self.tiles[(tile_y << power_of_two_tiles_per_row) | tile_x];
                } else {
                    // SAFETY: The tile X and Y coordinates are clamped, and
                    // `power_of_two_tiles_per_row` must be set to be accurate by `Self::from_image`,
                    // so the calculated index will never be out of bounds.
                    let tile = unsafe {
                        self.tiles
                            .get_unchecked((tile_y << power_of_two_tiles_per_row) | tile_x)
                    };
                }
            }
            let micro_tile_x = x_in_tile / TEXTURE_MICRO_TILE_DIM;
            let micro_tile_y = y_in_tile / TEXTURE_MICRO_TILE_DIM;
            let x_in_micro_tile = x_in_tile % TEXTURE_MICRO_TILE_DIM;
            let y_in_micro_tile = y_in_tile % TEXTURE_MICRO_TILE_DIM;
            let micro_tile = &tile[micro_tile_y][micro_tile_x];
            let rgba_u32 = micro_tile[y_in_micro_tile][x_in_micro_tile];
            Rgba::from_array(rgba_u32.to_ne_bytes().map(|n| n as f32 * (1.0 / 255.0)))
        });
        let mut out = rgba_samples[0] * sample_weights[0];
        for i in 1..4 {
            out += rgba_samples[i] * sample_weights[i];
        }
        out
    }

    #[inline(always)]
    pub fn sample_nearest_simd16(&self, us: f32x16, vs: f32x16) -> [unorm8x16; 4] {
        // Convert UV coords to texel coords.
        let xs = us
            .mul_sub_round(f32x16::splat(self.width.get() as f32), f32x16::splat(0.5))
            .clamp_all(0.0, (self.width.get() - 1) as f32)
            .cast_u32();
        let ys = vs
            .mul_sub_round(f32x16::splat(self.height.get() as f32), f32x16::splat(0.5))
            .clamp_all(0.0, (self.height.get() - 1) as f32)
            .cast_u32();
        let tile_xs = xs >> 5;
        let tile_ys = ys >> 5;
        let xs_in_tile = xs & 0x1F;
        let ys_in_tile = ys & 0x1F;
        let power_of_two_tiles_per_row = self.power_of_two_tiles_per_row.get() as u32;
        let micro_tile_xs = xs_in_tile >> 2;
        let micro_tile_ys = ys_in_tile >> 2;
        let xs_in_micro_tile = xs_in_tile & 0x3;
        let ys_in_micro_tile = ys_in_tile & 0x3;
        // Traverse nested arrays to get to RGBA values stored in micro-tiles.
        let tile_idxs = (tile_ys << power_of_two_tiles_per_row) | tile_xs;
        let tile_pixel_offsets =
            tile_idxs * u32x16::splat(core::mem::size_of::<TextureTileRgba>() as u32 / 4);
        let micro_tile_pixel_offsets = (micro_tile_ys
            * u32x16::splat(
                core::mem::size_of::<[TextureMicroTileRgba; TEXTURE_TILE_DIM]>() as u32 / 4,
            ))
            + (micro_tile_xs
                * u32x16::splat(core::mem::size_of::<TextureMicroTileRgba>() as u32 / 4));
        let rgba_pixel_offsets = (ys_in_micro_tile
            * u32x16::splat(core::mem::size_of::<[Rgba8Ne; TEXTURE_MICRO_TILE_DIM]>() as u32 / 4))
            + (xs_in_micro_tile * u32x16::splat(core::mem::size_of::<Rgba8Ne>() as u32 / 4));
        let texture_pixel_offsets =
            tile_pixel_offsets + micro_tile_pixel_offsets + rgba_pixel_offsets;
        // Load RGBAs.
        let rgbas: u32x16 = unsafe {
            u32x16::gather_from_offsets(self.tiles.as_ptr().cast(), texture_pixel_offsets)
        };
        rgba_u32x16_to_unorm8x16(rgbas)
    }

    #[inline(always)]
    pub fn sample_nearest_simd16_masked(
        &self,
        us: f32x16,
        vs: f32x16,
        enable: mask16,
    ) -> [unorm8x16; 4] {
        // Convert UV coords to texel coords.
        let xs = us
            .mul_sub_round(f32x16::splat(self.width.get() as f32), f32x16::splat(0.5))
            .clamp_all(0.0, (self.width.get() - 1) as f32)
            .cast_u32();
        let ys = vs
            .mul_sub_round(f32x16::splat(self.height.get() as f32), f32x16::splat(0.5))
            .clamp_all(0.0, (self.height.get() - 1) as f32)
            .cast_u32();
        let tile_xs = xs >> 5;
        let tile_ys = ys >> 5;
        let xs_in_tile = xs & 0x1F;
        let ys_in_tile = ys & 0x1F;
        let micro_tile_xs = xs_in_tile >> 2;
        let micro_tile_ys = ys_in_tile >> 2;
        let xs_in_micro_tile = xs_in_tile & 0x3;
        let ys_in_micro_tile = ys_in_tile & 0x3;
        // Traverse nested arrays to get to RGBA values stored in micro-tiles.
        let power_of_two_tiles_per_row = self.power_of_two_tiles_per_row.get() as u32;
        let tile_idxs = (tile_ys << power_of_two_tiles_per_row) | tile_xs;
        let tile_pixel_offsets =
            tile_idxs * u32x16::splat(core::mem::size_of::<TextureTileRgba>() as u32 / 4);
        let micro_tile_pixel_offsets = (micro_tile_ys
            * u32x16::splat(
                core::mem::size_of::<[TextureMicroTileRgba; TEXTURE_TILE_DIM]>() as u32 / 4,
            ))
            + (micro_tile_xs
                * u32x16::splat(core::mem::size_of::<TextureMicroTileRgba>() as u32 / 4));
        let rgba_pixel_offsets = (ys_in_micro_tile
            * u32x16::splat(core::mem::size_of::<[Rgba8Ne; TEXTURE_MICRO_TILE_DIM]>() as u32 / 4))
            + (xs_in_micro_tile * u32x16::splat(core::mem::size_of::<Rgba8Ne>() as u32 / 4));
        let texture_pixel_offsets =
            tile_pixel_offsets + micro_tile_pixel_offsets + rgba_pixel_offsets;
        // Load RGBAs.
        let rgbas: u32x16 = unsafe {
            u32x16::gather_from_offsets_masked(
                self.tiles.as_ptr().cast(),
                texture_pixel_offsets,
                enable,
            )
        };
        rgba_u32x16_to_unorm8x16(rgbas)
    }
}

impl core::ops::Index<(u32, u32)> for TiledTextureRgba {
    type Output = Rgba8Ne;

    #[inline(always)]
    fn index(&self, (x, y): (u32, u32)) -> &Self::Output {
        let tile_x = x as usize / TEXTURE_TILE_PIXEL_DIM;
        let tile_y = y as usize / TEXTURE_TILE_PIXEL_DIM;
        let x_in_tile = x as usize % TEXTURE_TILE_PIXEL_DIM;
        let y_in_tile = y as usize % TEXTURE_TILE_PIXEL_DIM;
        let power_of_two_tiles_per_row = self.power_of_two_tiles_per_row.get() as usize;
        let tile = &self.tiles[(tile_y << power_of_two_tiles_per_row) | tile_x];
        let micro_tile_x = x_in_tile / TEXTURE_MICRO_TILE_DIM;
        let micro_tile_y = y_in_tile / TEXTURE_MICRO_TILE_DIM;
        let x_in_micro_tile = x_in_tile % TEXTURE_MICRO_TILE_DIM;
        let y_in_micro_tile = y_in_tile % TEXTURE_MICRO_TILE_DIM;
        let micro_tile = &tile[micro_tile_y][micro_tile_x];
        &micro_tile[y_in_micro_tile][x_in_micro_tile]
    }
}

impl core::ops::IndexMut<(u32, u32)> for TiledTextureRgba {
    #[inline(always)]
    fn index_mut(&mut self, (x, y): (u32, u32)) -> &mut Self::Output {
        let tile_x = x as usize / TEXTURE_TILE_PIXEL_DIM;
        let tile_y = y as usize / TEXTURE_TILE_PIXEL_DIM;
        let x_in_tile = x as usize % TEXTURE_TILE_PIXEL_DIM;
        let y_in_tile = y as usize % TEXTURE_TILE_PIXEL_DIM;
        let power_of_two_tiles_per_row = self.power_of_two_tiles_per_row.get() as usize;
        let tile = &mut self.tiles[(tile_y << power_of_two_tiles_per_row) | tile_x];
        let micro_tile_x = x_in_tile / TEXTURE_MICRO_TILE_DIM;
        let micro_tile_y = y_in_tile / TEXTURE_MICRO_TILE_DIM;
        let x_in_micro_tile = x_in_tile % TEXTURE_MICRO_TILE_DIM;
        let y_in_micro_tile = y_in_tile % TEXTURE_MICRO_TILE_DIM;
        let micro_tile = &mut tile[micro_tile_y][micro_tile_x];
        &mut micro_tile[y_in_micro_tile][x_in_micro_tile]
    }
}

#[tracing::instrument(skip_all)]
pub fn render_tile_bins(
    out_framebuffer: &LinearFramebufferRgba,
    block_item_atlas: &TextureAtlas,
    egui_renderer: &egui_rendering::Renderer,
    tile_bins: &mut RenderTileBins,
    clear_colour: Rgba,
) {
    tile_bins
        .bins
        .par_iter_mut()
        .enumerate()
        .for_each(|(tile_i, tile_bin)| {
            let tiles_per_row = tile_bins.tiles_per_row.get() as usize;
            let tile_y = tile_i / tiles_per_row;
            let tile_x = tile_i % tiles_per_row;
            render_tile_bin(
                out_framebuffer,
                block_item_atlas,
                egui_renderer,
                tile_bin,
                (tile_x, tile_y),
                clear_colour,
            );
        });
}

#[tracing::instrument(skip_all)]
fn render_tile_bin(
    out_framebuffer: &LinearFramebufferRgba,
    block_item_atlas: &TextureAtlas,
    egui_renderer: &egui_rendering::Renderer,
    tile_bin: &mut RenderTileBin,
    (tile_x, tile_y): (usize, usize),
    clear_colour: Rgba,
) {
    // Create tile colour information and Hi-Z chain on the stack.
    // This is large (~33KB), but storing on stack should keep it in a consistent location for
    // subsequent calls, so should be mostly kept in data cache.
    let mut tile: RenderTileRgba = [[[[u32x16::splat(clear_colour.to_rgba8ne()); 4]; 4]; 4]; 4];
    let mut tile_hi_z = RenderTileHiZChain::zeroed();
    // Render chunk tile commands.
    chunk::render_tile(
        &mut tile,
        &mut tile_hi_z,
        (tile_x, tile_y),
        block_item_atlas,
        &mut tile_bin.chunk_draw_cmds,
    );
    // Render egui tile commands.
    egui_renderer.render_tile(
        &mut tile,
        (tile_x, tile_y),
        tile_bin.egui_draw_cmds.drain(..),
    );
    // Write tile out to framebuffer.
    {
        let span = tracing::trace_span!("write_tile_to_framebuffer");
        let _enter = span.enter();
        const PIXEL_BYTE_SIZE: usize = core::mem::size_of::<Rgba8Ne>();
        let row_byte_len = out_framebuffer.width.get() as usize * PIXEL_BYTE_SIZE;
        let start_x = tile_x * RENDER_TILE_PIXEL_DIM;
        let start_y = tile_y * RENDER_TILE_PIXEL_DIM;
        let actual_tile_width = usize::min(
            out_framebuffer.width.get() as usize - start_x,
            RENDER_TILE_PIXEL_DIM,
        );
        let actual_tile_height = usize::min(
            out_framebuffer.height.get() as usize - start_y,
            RENDER_TILE_PIXEL_DIM,
        );
        if actual_tile_width < RENDER_TILE_PIXEL_DIM || actual_tile_height < RENDER_TILE_PIXEL_DIM {
            // If we're at the edge of the screen, and we're not copying a full tile to the linear
            // framebuffer, then we just fall back to copying scanline by scanline.
            // This isn't very fast, but it's only for a small portion of the screen.
            for y_in_tile in 0..actual_tile_height {
                let row = unsafe {
                    core::slice::from_raw_parts_mut(
                        out_framebuffer.data.add(
                            (row_byte_len * (start_y + y_in_tile)) + (start_x * PIXEL_BYTE_SIZE),
                        ),
                        actual_tile_width * PIXEL_BYTE_SIZE,
                    )
                };
                let (pixels, remainder) = row.as_chunks_mut::<PIXEL_BYTE_SIZE>();
                debug_assert!(remainder.is_empty());
                for (x_in_tile, pixel) in pixels.iter_mut().enumerate() {
                    let micro_tile_x = x_in_tile / RENDER_MICRO_TILE_PIXEL_DIM;
                    let micro_tile_y = y_in_tile / RENDER_MICRO_TILE_PIXEL_DIM;
                    let x_in_micro_tile = x_in_tile % RENDER_MICRO_TILE_PIXEL_DIM;
                    let y_in_micro_tile = y_in_tile % RENDER_MICRO_TILE_PIXEL_DIM;
                    let pixel_group_x = x_in_micro_tile / RENDER_PIXEL_GROUP_DIM;
                    let pixel_group_y = y_in_micro_tile / RENDER_PIXEL_GROUP_DIM;
                    let x_in_pixel_group = x_in_micro_tile % RENDER_PIXEL_GROUP_DIM;
                    let y_in_pixel_group = y_in_micro_tile % RENDER_PIXEL_GROUP_DIM;
                    let micro_tile = &tile[micro_tile_y][micro_tile_x];
                    let pixel_group = &micro_tile[pixel_group_y][pixel_group_x];
                    let rgba8 = pixel_group.to_array()
                        [(y_in_pixel_group * RENDER_PIXEL_GROUP_DIM) + x_in_pixel_group];
                    *pixel = rgba8.to_ne_bytes();
                }
            }
        } else {
            // If we're not at the edge of the screen, then we know we're copying an entire tile to
            // the framebuffer.
            // This allows us to optimise for consecutive tile pixel memory reads, over consecutive
            // framebuffer memory writes.
            // TODO: This optimisation was much faster when we were copying an entire tiled render
            //       target to a linear framebuffer, but is it still an improvement when the tile
            //       is already mostly in data cache?
            let tile_start_byte = (start_y * row_byte_len) + (start_x * PIXEL_BYTE_SIZE);
            for (micro_tile_y, micro_tile_row) in tile.iter().enumerate() {
                for (micro_tile_x, micro_tile) in micro_tile_row.iter().enumerate() {
                    for (pixel_group_y, pixel_group_row) in micro_tile.iter().enumerate() {
                        for (pixel_group_x, pixel_group) in
                            pixel_group_row.iter().copied().enumerate()
                        {
                            let y_in_tile = (micro_tile_y * RENDER_MICRO_TILE_PIXEL_DIM)
                                + (pixel_group_y * RENDER_PIXEL_GROUP_DIM);
                            let x_in_tile = (micro_tile_x * RENDER_MICRO_TILE_PIXEL_DIM)
                                + (pixel_group_x * RENDER_PIXEL_GROUP_DIM);
                            let x_byte_offset = x_in_tile * PIXEL_BYTE_SIZE;
                            for (i, byte_group) in
                                pixel_group.to_ne_16_byte_groups().into_iter().enumerate()
                            {
                                unsafe {
                                    let out_slice = core::slice::from_raw_parts_mut(
                                        out_framebuffer.data.add(
                                            tile_start_byte
                                                + ((y_in_tile + i) * row_byte_len)
                                                + x_byte_offset,
                                        ),
                                        PIXEL_BYTE_SIZE * 4,
                                    );
                                    out_slice.copy_from_slice(&byte_group);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
