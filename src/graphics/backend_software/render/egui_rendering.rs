use super::{
    RENDER_MICRO_TILE_DIM, RENDER_MICRO_TILE_PIXEL_DIM, RENDER_PIXEL_GROUP_DIM, RENDER_TILE_DIM,
    RENDER_TILE_PIXEL_DIM, RenderMicroTileRgba, RenderPixelGroupRgba, RenderTileBins,
    RenderTileRgba, Rgba, TiledTextureRgba, Unorm8x16, f32x16_to_unorm8x16,
    rgba_4xunorm8x16_to_u32x16, rgba_u32x16_to_unorm8x16,
};
use bitfield::bitfield;
use core::simd::prelude::*;
use image::RgbaImage;
use portable_std::FastHashMap;
use std::sync::Arc;

// TODO: Figure out why this is rendering too dark.
// - Pre-multiplied vs unmultiplied alpha?

#[derive(Default)]
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
    pub image: TiledTextureRgba,
    pub options: egui::TextureOptions,
}

impl ImageData {
    #[inline(always)]
    pub fn sample_simd16(&self, us: f32x16, vs: f32x16) -> [Unorm8x16; 4] {
        // use egui::TextureFilter::*;
        use egui::TextureWrapMode::*;
        let [us, vs] = match self.options.wrap_mode {
            // Sample function already clamps to edge of texture, so no need to clamp here.
            ClampToEdge => [us, vs],
            Repeat => {
                [us, vs].map(|ns| f32x16::from_array(ns.as_array().map(|n| n.rem_euclid(1.0))))
            }
            MirroredRepeat => [us, vs].map(|ns| {
                f32x16::splat(1.0) - (f32x16::splat(1.0) - (ns.abs() % f32x16::splat(2.0))).abs()
            }),
        };
        self.image.sample_nearest_simd16(us, vs)
    }

    #[inline(always)]
    pub fn sample_simd16_masked(
        &self,
        us: f32x16,
        vs: f32x16,
        enable: mask32x16,
    ) -> [Unorm8x16; 4] {
        // use egui::TextureFilter::*;
        use egui::TextureWrapMode::*;
        let [us, vs] = match self.options.wrap_mode {
            // Sample function already clamps to edge of texture, so no need to clamp here.
            ClampToEdge => [us, vs],
            Repeat => {
                [us, vs].map(|ns| f32x16::from_array(ns.as_array().map(|n| n.rem_euclid(1.0))))
            }
            MirroredRepeat => [us, vs].map(|ns| {
                f32x16::splat(1.0) - (f32x16::splat(1.0) - (ns.abs() % f32x16::splat(2.0))).abs()
            }),
        };
        self.image.sample_nearest_simd16_masked(us, vs, enable)
    }
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            images: FastHashMap::new(),
            next_user_texture_id: 0,
        }
    }

    #[tracing::instrument(skip_all)]
    pub fn free_textures(&mut self, texture_ids: &[egui::TextureId]) {
        for id in texture_ids {
            self.images.remove(id);
        }
    }

    #[tracing::instrument(skip_all)]
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
                        .flat_map(|&color| color.to_array())
                        .collect()
                }
            };
            let new_image = RgbaImage::from_raw(width as u32, height as u32, pixels).unwrap();
            if let Some(pos) = texture_data.pos {
                // Update existing image with new data.
                let current_image_data = self.images.get_mut(&texture_id).unwrap();
                let current_image = &mut current_image_data.image;
                let origin: [u32; 2] = pos.map(|n| n as u32);
                for y in 0..size[1] {
                    for x in 0..size[0] {
                        current_image[(origin[0] + x, origin[1] + y)] =
                            u32::from_ne_bytes(new_image[(x, y)].0);
                    }
                }
            } else {
                // Register new image.
                self.images.insert(
                    texture_id,
                    ImageData {
                        image: TiledTextureRgba::from_image(&new_image).unwrap(),
                        options: texture_data.options,
                    },
                );
            }
        }
    }

    #[tracing::instrument(skip_all)]
    pub fn bin_to_tiles(
        &mut self,
        out_bins: &mut RenderTileBins,
        physical_size: &winit::dpi::PhysicalSize<u32>,
        texture_updates: Vec<(egui::TextureId, egui::epaint::image::ImageDelta)>,
        clipped_primitives: Vec<egui::ClippedPrimitive>,
        pixels_per_point: f32,
    ) {
        let width = physical_size.width as f32;
        let height = physical_size.height as f32;
        self.update_textures(texture_updates);
        bin_egui_meshes(
            out_bins,
            clipped_primitives
                .into_iter()
                .filter_map(|clipped_primitive| {
                    let egui::ClippedPrimitive {
                        clip_rect,
                        primitive,
                    } = clipped_primitive;
                    if clip_rect.area() == 0.0 {
                        return None;
                    }
                    let egui::epaint::Primitive::Mesh(mesh) = primitive else {
                        unimplemented!("egui custom callbacks");
                    };
                    Some(Arc::new(RenderMeshInfo::new(
                        clip_rect,
                        mesh,
                        (width, height),
                        pixels_per_point,
                    )))
                }),
        )
    }

    #[tracing::instrument(skip_all)]
    pub fn render_tile(
        &self,
        out_tile: &mut RenderTileRgba,
        (tile_x, tile_y): (usize, usize),
        draw_cmds: impl IntoIterator<Item = TileDrawCommand>,
    ) {
        render_egui_tile(out_tile, (tile_x, tile_y), &self.images, draw_cmds)
    }

    pub fn register_user_image(
        &mut self,
        image: TiledTextureRgba,
        options: egui::TextureOptions,
    ) -> anyhow::Result<egui::TextureId> {
        let texture_id = egui::TextureId::User(self.next_user_texture_id);
        self.next_user_texture_id += 1;
        self.images.insert(texture_id, ImageData { image, options });
        Ok(texture_id)
    }
}

#[derive(Clone, Debug)]
struct RenderMeshInfo {
    pub clip_rect: egui::Rect,
    pub clip_rect_edges: [ClipEdge; 4],
    pub vertices: Box<[EguiRenderVertex]>,
    pub indices: Box<[u32]>,
    pub texture_id: egui::TextureId,
}

impl RenderMeshInfo {
    pub fn new(
        clip_rect: egui::Rect,
        mesh: egui::Mesh,
        screen_size: (f32, f32),
        pixels_per_point: f32,
    ) -> Self {
        let (width, height) = screen_size;
        // Scale clip rect from logical size to physical size, limit to screen bounds.
        let clip_rect = egui::Rect {
            min: egui::Pos2::new(
                (clip_rect.min.x * pixels_per_point)
                    .max(0.0)
                    .min(width - 1.0),
                (clip_rect.min.y * pixels_per_point)
                    .max(0.0)
                    .min(height - 1.0),
            ),
            max: egui::Pos2::new(
                (clip_rect.max.x * pixels_per_point)
                    .max(0.0)
                    .min(width - 1.0),
                (clip_rect.max.y * pixels_per_point)
                    .max(0.0)
                    .min(height - 1.0),
            ),
        };
        // Generate clipping edges.
        let clip_rect_edges: [ClipEdge; 4] = [
            // Left
            ClipEdge {
                half_plane_params: [-1.0, 0.0, -clip_rect.min.x],
                flags: {
                    let mut flags = ClipEdgeFlags(0);
                    flags.set_trivial_reject_offset_x(1);
                    flags.set_trivial_reject_offset_y(0);
                    flags.set_trivial_accept_offset_x(0);
                    flags.set_trivial_accept_offset_y(0);
                    flags.set_is_half_plane_closed(true);
                    flags
                },
            },
            // Right
            ClipEdge {
                half_plane_params: [1.0, 0.0, clip_rect.max.x + 1.0],
                flags: {
                    let mut flags = ClipEdgeFlags(0);
                    flags.set_trivial_reject_offset_x(0);
                    flags.set_trivial_reject_offset_y(0);
                    flags.set_trivial_accept_offset_x(1);
                    flags.set_trivial_accept_offset_y(0);
                    flags.set_is_half_plane_closed(false);
                    flags
                },
            },
            // Top
            ClipEdge {
                half_plane_params: [0.0, -1.0, -clip_rect.min.y],
                flags: {
                    let mut flags = ClipEdgeFlags(0);
                    flags.set_trivial_reject_offset_x(0);
                    flags.set_trivial_reject_offset_y(1);
                    flags.set_trivial_accept_offset_x(0);
                    flags.set_trivial_accept_offset_y(0);
                    flags.set_is_half_plane_closed(true);
                    flags
                },
            },
            // Bottom
            ClipEdge {
                half_plane_params: [0.0, 1.0, clip_rect.max.y + 1.0],
                flags: {
                    let mut flags = ClipEdgeFlags(0);
                    flags.set_trivial_reject_offset_x(0);
                    flags.set_trivial_reject_offset_y(0);
                    flags.set_trivial_accept_offset_x(0);
                    flags.set_trivial_accept_offset_y(1);
                    flags.set_is_half_plane_closed(false);
                    flags
                },
            },
        ];
        Self {
            clip_rect,
            clip_rect_edges,
            vertices: mesh
                .vertices
                .into_iter()
                .map(|v| EguiRenderVertex {
                    pos: v.pos * pixels_per_point,
                    uv: v.uv,
                    colour: v.color.into(),
                })
                .collect(),
            indices: mesh.indices.into_boxed_slice(),
            texture_id: mesh.texture_id,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EguiRenderVertex {
    pub pos: egui::Pos2,
    pub uv: egui::Pos2,
    pub colour: Rgba,
}

// TODO:
// - Switch to fully deferring all rendering.
// - Split this into a binning stage and a single-tile rendering stage.

#[derive(Clone, Debug)]
pub struct TileDrawCommand {
    tri_info: Arc<TriInfo>,
    ty: TileDrawCommandType,
}

#[derive(Clone, Debug)]
struct TriInfo {
    mesh_info: Arc<RenderMeshInfo>,
    tri_indices: [u32; 3],
    tri_edges: [ClipEdge; 3],
    texture_id: egui::TextureId,
}

#[derive(Clone, Debug)]
enum TileDrawCommandType {
    /// The entire tile should be rasterised, so no per-micro-tile, per-pixel-group, or
    /// per-pixel rasterisation checks are required.
    WholeTile,
    /// Only part of the tile should be rasterised, so sub-tile rasterisation checks are
    /// required.
    PartialTile,
}

/// An edge that restricts rasterisation to the "inside" half of the edge.
///
/// For `egui` meshes, a triangle is rasterised using seven clip edges: the three triangle edges
/// that restrict rasterisation to the inside of the triangle, and the four clipping rectangle
/// edges that restrict rasterisation to the inside of the clipping rectangle.
#[derive(Clone, Copy)]
struct ClipEdge {
    /// The A, B, and C parameters in the equation for the half-plane, which is either
    /// `Ax + By < C`, or `Ax + By <= C` (if `is_half_plane_closed` is true).
    pub half_plane_params: [f32; 3],
    pub flags: ClipEdgeFlags,
}

impl core::fmt::Debug for ClipEdge {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ClipEdge")
            .field(
                "half_plane_equation",
                &format_args!(
                    "ClipEdge({}x + {}y {} {})",
                    self.half_plane_params[0],
                    self.half_plane_params[1],
                    if self.flags.is_half_plane_closed() {
                        "<="
                    } else {
                        "<"
                    },
                    self.half_plane_params[2],
                ),
            )
            .field(
                "trivial_reject_corner",
                &[
                    self.flags.trivial_reject_offset_x(),
                    self.flags.trivial_reject_offset_y(),
                ],
            )
            .field(
                "trivial_accept_corner",
                &[
                    self.flags.trivial_accept_offset_x(),
                    self.flags.trivial_accept_offset_y(),
                ],
            )
            .finish()
    }
}

bitfield! {
    #[derive(Clone, Copy)]
    struct ClipEdgeFlags(u8);
    impl Debug;
    /// X offset of trivial reject corner on unit square.
    pub trivial_reject_offset_x, set_trivial_reject_offset_x: 0, 0;
    /// Y offset of trivial reject corner on unit square.
    pub trivial_reject_offset_y, set_trivial_reject_offset_y: 1, 1;
    /// X offset of trivial accept corner on unit square.
    pub trivial_accept_offset_x, set_trivial_accept_offset_x: 2, 2;
    /// Y offset of trivial accept corner on unit square.
    pub trivial_accept_offset_y, set_trivial_accept_offset_y: 3, 3;
    /// Whether the half-plane determined by this edge is a closed half-plane, where
    /// closed half-planes include points exactly on the edge.
    ///
    /// Determines whether the comparison used in the equation for the half-plane (see
    /// `half_plane_params`) is inclusive.
    pub is_half_plane_closed, set_is_half_plane_closed: 4;
}

#[multiversion::multiversion(targets(
    "x86_64+sse4.2+bmi1+bmi2+fma+lzcnt+movbe+avx512f+avx512vl+avx512dq+avx512bw",
    "x86_64+sse4.2+bmi1+bmi2+fma+lzcnt+movbe+avx2",
    "x86/i686+sse2",
    "arm+neon",
))]
#[tracing::instrument(skip_all)]
fn bin_egui_meshes(
    out_bins: &mut RenderTileBins,
    meshes: impl IntoIterator<Item = Arc<RenderMeshInfo>>,
) {
    let tiles_per_row = out_bins.tiles_per_row.get() as usize;
    // Go through meshes, and bin all mesh triangles into tile bins.
    for mesh in meshes.into_iter() {
        for tri_indices in mesh.indices.chunks(3) {
            let &[mut idx_0, mut idx_1, idx_2] = tri_indices else {
                unreachable!();
            };
            // Load triangle vertices.
            let mut tri: [&EguiRenderVertex; 3] =
                [idx_0, idx_1, idx_2].map(|i| &mesh.vertices[i as usize]);
            // Fix triangle winding order, so that calculated edge equations will be consistent.
            // Winding order should then be preserved during clipping.
            {
                // Calculate Z component of cross product (triangle normal "out of the screen").
                let a = tri[1].pos - tri[0].pos;
                let b = tri[2].pos - tri[0].pos;
                let z = (a.x * b.y) - (a.y * b.x);
                // If the normal's pointing the wrong way, swap the first two vertices to fix the
                // winding order.
                if z < 0.0 {
                    tri.swap(0, 1);
                    core::mem::swap(&mut idx_0, &mut idx_1);
                }
            }
            let tri_edge_points = [
                (tri[0].pos, tri[1].pos),
                (tri[1].pos, tri[2].pos),
                (tri[2].pos, tri[0].pos),
            ];
            let tri_clip_edges: [ClipEdge; 3] = tri_edge_points.map(|(p1, p2)| {
                // Calculate the base X and Y coordinates of the trivial reject corner.
                // This is the corner of a unit square most likely to reside within the inside
                // half-plane of the edge.
                // Our screen coordinate system has +X going right, and +Y going down, so if we
                // have the top-left corner of a 64x64 pixel tile starting at some `tile_xy`, then
                // the trivial reject corner of the tile would be at
                // `tile_xy + 0.5 + (63.0 * trivial_reject_offset_base_xy)`.
                // The 0.5 offset is due to the actual bounding box that we want to use for a tile
                // being the bounding box of the pixels within a tile, and we currently use a
                // sample position inside each pixel of (0.5, 0.5), although MSAA support would
                // mean multiple sample positions per pixel (random(?), although definitely the
                // same for each pixel in a frame).
                // If the trivial reject corner resides in the closed half-plane outside the
                // triangle edge, then we know the entire tile is definitely outside the edge.
                // This then means that we can use this as a test, where if any of the tile's
                // trivial reject corners lies outside the corner's corresponding edge, then the
                // entire tile is definitely outside the triangle, and so we can skip it.
                let trivial_reject_offset_x: u8 = match p1.y > p2.y {
                    false => 0,
                    true => 1,
                };
                let trivial_reject_offset_y: u8 = match p1.x > p2.x {
                    false => 1,
                    true => 0,
                };
                // Calculate the base X and Y coordinates of the trivial accept corner.
                // This is the corner of a unit square most likely to reside within the outside
                // half-plane of the edge, and is opposite to the trivial reject corner.
                // The calculations for finding the corner of a 64x64 pixel tile are practically
                // identical (`tile_xy + 0.5 + (63.0 * trivial_accept_offset_base_xy)`).
                // If the trivial accept corner resides in the half-plane inside the triangle edge,
                // then we know that the entire tile is definitely inside the edge.
                // This then means that if all of the tile's trivial accept corners lie inside the
                // corner's corresponding edge, then the entire triangle is definitely inside the
                // triangle, and we can just rasterise the entire tile.
                let trivial_accept_offset_x: u8 = match p1.y > p2.y {
                    false => 1,
                    true => 0,
                };
                let trivial_accept_offset_y: u8 = match p1.x > p2.x {
                    false => 0,
                    true => 1,
                };
                // Calcluate the half-plane equation parameters `A`, `B`, and `c`.
                let param_a = p2.y - p1.y;
                let param_b = p1.x - p2.x;
                let param_c = (param_a * p1.x) + (param_b * p1.y);
                let half_plane_params = [param_a, param_b, param_c];
                // Determine whether the edge should be inclusive of points exactly on the edge.
                // If we were to imagine that we were rasterising an octagon (for demonstration
                // purposes):
                //
                // /---\
                // |   |
                // \___/
                //
                // The sides we want to rasterise using an inclusive bounds test would be:
                //
                // /---
                // |
                // \
                //
                // These are the "top", "top-left", "left", and "bottom-left" edges.
                let is_half_plane_closed =
                    (p1.x >= p2.x && p1.y <= p2.y) || (p1.x < p2.x && p1.y < p2.y);
                ClipEdge {
                    half_plane_params,
                    flags: {
                        let mut flags = ClipEdgeFlags(0);
                        flags.set_trivial_reject_offset_x(trivial_reject_offset_x);
                        flags.set_trivial_reject_offset_y(trivial_reject_offset_y);
                        flags.set_trivial_accept_offset_x(trivial_accept_offset_x);
                        flags.set_trivial_accept_offset_y(trivial_accept_offset_y);
                        flags.set_is_half_plane_closed(is_half_plane_closed);
                        flags
                    },
                }
            });
            // Package up triangle info for use in tile bin draw commands.
            let tri_info = Arc::new(TriInfo {
                mesh_info: mesh.clone(),
                tri_indices: [idx_0, idx_1, idx_2],
                tri_edges: tri_clip_edges,
                texture_id: mesh.texture_id,
            });
            // Combine clipping rect and triangle clip edges.
            let combined_clip_edges: [ClipEdge; 7] = unsafe {
                let mut out = [core::mem::MaybeUninit::uninit(); 7];
                for (i, edge) in mesh
                    .clip_rect_edges
                    .iter()
                    .chain(&tri_clip_edges)
                    .enumerate()
                {
                    out[i].write(*edge);
                }
                out.map(|edge| edge.assume_init())
            };
            // Calculate triangle bounding box.
            let mut tri_bb_min_x = f32::INFINITY;
            let mut tri_bb_min_y = f32::INFINITY;
            let mut tri_bb_max_x = f32::NEG_INFINITY;
            let mut tri_bb_max_y = f32::NEG_INFINITY;
            for v in &tri {
                tri_bb_min_x = f32::min(tri_bb_min_x, v.pos.x);
                tri_bb_min_y = f32::min(tri_bb_min_y, v.pos.y);
                tri_bb_max_x = f32::max(tri_bb_max_x, v.pos.x);
                tri_bb_max_y = f32::max(tri_bb_max_y, v.pos.y);
            }
            // The actual bounding box used for rasterisation will be the intersection of the
            // clipping rect and triangle bounding boxes.
            let bb_min_x = f32::max(mesh.clip_rect.min.x, tri_bb_min_x);
            let bb_min_y = f32::max(mesh.clip_rect.min.y, tri_bb_min_y);
            let bb_max_x = f32::min(mesh.clip_rect.max.x, tri_bb_max_x);
            let bb_max_y = f32::min(mesh.clip_rect.max.y, tri_bb_max_y);
            // Convert bounding box from pixel coordinates to integer tile coordinates.
            let tile_bb_min_x = (bb_min_x / RENDER_TILE_PIXEL_DIM as f32).floor() as usize;
            let tile_bb_min_y = (bb_min_y / RENDER_TILE_PIXEL_DIM as f32).floor() as usize;
            let tile_bb_max_x = (bb_max_x / RENDER_TILE_PIXEL_DIM as f32).floor() as usize;
            let tile_bb_max_y = (bb_max_y / RENDER_TILE_PIXEL_DIM as f32).floor() as usize;
            for tile_y in tile_bb_min_y..=tile_bb_max_y {
                let tile_start_y = (tile_y * RENDER_TILE_PIXEL_DIM) as f32;
                'tile_loop: for tile_x in tile_bb_min_x..=tile_bb_max_x {
                    let tile_start_x = (tile_x * RENDER_TILE_PIXEL_DIM) as f32;
                    // Whether the tile passes every edge's trivial accept test, meaning that the
                    // entire tile can be rasterised without testing inner micro-tiles.
                    let mut whole_tile_accepted = true;
                    for edge in &combined_clip_edges {
                        // Calculate trivial reject and accept corner positions for this
                        // (tile, edge) combination.
                        let tile_trivial_reject_x = tile_start_x
                            + (RENDER_TILE_PIXEL_DIM
                                * edge.flags.trivial_reject_offset_x() as usize)
                                as f32;
                        let tile_trivial_reject_y = tile_start_y
                            + (RENDER_TILE_PIXEL_DIM
                                * edge.flags.trivial_reject_offset_y() as usize)
                                as f32;
                        let tile_trivial_accept_x = tile_start_x
                            + (RENDER_TILE_PIXEL_DIM
                                * edge.flags.trivial_accept_offset_x() as usize)
                                as f32;
                        let tile_trivial_accept_y = tile_start_y
                            + (RENDER_TILE_PIXEL_DIM
                                * edge.flags.trivial_accept_offset_y() as usize)
                                as f32;
                        // Test the trivial reject corner.
                        // If this corner lies outside of the edge's half-plane, then we can be
                        // certain that the entire tile lies outside the triangle, and can be
                        // skipped.
                        let [a, b, c] = edge.half_plane_params;
                        let reject_test_val =
                            (a * tile_trivial_reject_x) + (b * tile_trivial_reject_y);
                        let should_reject_tile = if edge.flags.is_half_plane_closed() {
                            reject_test_val > c
                        } else {
                            reject_test_val >= c
                        };
                        if should_reject_tile {
                            continue 'tile_loop;
                        }
                        // Test the trivial accept corner.
                        let accept_test_val =
                            (a * tile_trivial_accept_x) + (b * tile_trivial_accept_y);
                        let whole_tile_still_accepted = if edge.flags.is_half_plane_closed() {
                            accept_test_val <= c
                        } else {
                            accept_test_val < c
                        };
                        whole_tile_accepted &= whole_tile_still_accepted;
                    }
                    // Add draw command to tile bin.
                    let out_bin = &mut out_bins.bins[(tile_y * tiles_per_row) + tile_x];
                    out_bin.egui_draw_cmds.push(TileDrawCommand {
                        tri_info: tri_info.clone(),
                        ty: if whole_tile_accepted {
                            TileDrawCommandType::WholeTile
                        } else {
                            TileDrawCommandType::PartialTile
                        },
                    });
                }
            }
        }
    }
}

#[multiversion::multiversion(targets(
    "x86_64+sse4.2+bmi1+bmi2+fma+lzcnt+movbe+avx512f+avx512vl+avx512dq+avx512bw",
    "x86_64+sse4.2+bmi1+bmi2+fma+lzcnt+movbe+avx2",
    "x86/i686+sse2",
    "arm+neon",
))]
#[tracing::instrument(skip_all)]
fn render_egui_tile(
    out_tile: &mut RenderTileRgba,
    (tile_x, tile_y): (usize, usize),
    images: &FastHashMap<egui::TextureId, ImageData>,
    draw_cmds: impl IntoIterator<Item = TileDrawCommand>,
) {
    let tile_start_x = tile_x * RENDER_TILE_PIXEL_DIM;
    let tile_start_y = tile_y * RENDER_TILE_PIXEL_DIM;
    for draw_cmd in draw_cmds {
        let tri_info = &draw_cmd.tri_info;
        let mesh = tri_info.mesh_info.as_ref();
        let tri = tri_info.tri_indices.map(|idx| &mesh.vertices[idx as usize]);
        let image = &images[&tri_info.texture_id];
        match draw_cmd.ty {
            TileDrawCommandType::WholeTile => {
                for micro_tile_i in 0..RENDER_TILE_DIM.pow(2) {
                    let micro_tile_y = micro_tile_i / RENDER_TILE_DIM;
                    let micro_tile_x = micro_tile_i % RENDER_TILE_DIM;
                    let micro_tile: &mut RenderMicroTileRgba =
                        &mut out_tile[micro_tile_y][micro_tile_x];
                    let micro_tile_start_x =
                        tile_start_x + (micro_tile_x * RENDER_MICRO_TILE_PIXEL_DIM);
                    let micro_tile_start_y =
                        tile_start_y + (micro_tile_y * RENDER_MICRO_TILE_PIXEL_DIM);
                    for pixel_group_i in 0..RENDER_MICRO_TILE_DIM.pow(2) {
                        let pixel_group_y = pixel_group_i / RENDER_MICRO_TILE_DIM;
                        let pixel_group_x = pixel_group_i % RENDER_MICRO_TILE_DIM;
                        let pixel_group: &mut RenderPixelGroupRgba =
                            &mut micro_tile[pixel_group_y][pixel_group_x];
                        let pixel_group_start_x =
                            micro_tile_start_x + (pixel_group_x * RENDER_PIXEL_GROUP_DIM);
                        let pixel_group_start_y =
                            micro_tile_start_y + (pixel_group_y * RENDER_PIXEL_GROUP_DIM);
                        // Collect individual pixel X and Y coords into vectors, so we can
                        // process up to 16 pixels in parallel (i.e. if the hardware
                        // supports 512-bit SIMD, then an entire pixel group can be
                        // processed).
                        #[rustfmt::skip]
                        let pixel_x_offsets = f32x16::from_array([
                            0., 1., 2., 3.,
                            0., 1., 2., 3.,
                            0., 1., 2., 3.,
                            0., 1., 2., 3.,
                        ]);
                        #[rustfmt::skip]
                        let pixel_y_offsets = f32x16::from_array([
                            0., 0., 0., 0.,
                            1., 1., 1., 1.,
                            2., 2., 2., 2.,
                            3., 3., 3., 3.,
                        ]);
                        let sample_offsets = f32x16::splat(0.5);
                        let pixel_xs: f32x16 = pixel_x_offsets
                            + f32x16::splat(pixel_group_start_x as f32)
                            + sample_offsets;
                        let pixel_ys: f32x16 = pixel_y_offsets
                            + f32x16::splat(pixel_group_start_y as f32)
                            + sample_offsets;
                        // Calculate barycentrics per pixel.
                        let (bary_us, bary_vs, bary_ws) = {
                            // Based on this implementation:
                            // <https://gamedev.stackexchange.com/a/63203>
                            let v0: egui::Vec2 = tri[1].pos - tri[0].pos;
                            let v1: egui::Vec2 = tri[2].pos - tri[0].pos;
                            let v2_xs: f32x16 = pixel_xs - f32x16::splat(tri[0].pos.x);
                            let v2_ys: f32x16 = pixel_ys - f32x16::splat(tri[0].pos.y);
                            let denom_recip: f32 = 1.0 / ((v0.x * v1.y) - (v1.x * v0.y));
                            let vs: f32x16 = ((v2_xs * f32x16::splat(v1.y))
                                - (v2_ys * f32x16::splat(v1.x)))
                                * f32x16::splat(denom_recip);
                            let ws: f32x16 = ((v2_ys * f32x16::splat(v0.x))
                                - (v2_xs * f32x16::splat(v0.y)))
                                * f32x16::splat(denom_recip);
                            let us: f32x16 = f32x16::splat(1.0) - vs - ws;
                            // // Based on this implementation:
                            // // <https://gamedev.stackexchange.com/a/23745>
                            // let v0: egui::Vec2 = tri[1].pos - tri[0].pos;
                            // let v1: egui::Vec2 = tri[2].pos - tri[0].pos;
                            // let v2_xs: f32x16 = pixel_xs - tri[0].pos.x;
                            // let v2_ys: f32x16 = pixel_ys - tri[0].pos.y;
                            // let d00: f32 = v0.dot(v0);
                            // let d01: f32 = v0.dot(v1);
                            // let d11: f32 = v1.dot(v1);
                            // let d20: f32x16 = (v2_xs * v0.x) + (v2_ys * v0.y);
                            // let d21: f32x16 = (v2_xs * v1.x) + (v2_ys * v1.y);
                            // let denom_recip: f32 = 1.0 / ((d00 * d11) - (d01 * d01));
                            // let vs: f32x16 = ((d20 * d11) - (d21 * d01)) * denom_recip;
                            // let ws: f32x16 = ((d21 * d00) - (d20 * d01)) * denom_recip;
                            // let us: f32x16 = f32x16::splat(1.0) - vs - ws;
                            (us, vs, ws)
                        };
                        // Interpolate vertex RGBAs using barycentrics.
                        let pixel_vert_reds_f32 = (f32x16::splat(tri[0].colour.r()) * bary_us)
                            + (f32x16::splat(tri[1].colour.r()) * bary_vs)
                            + (f32x16::splat(tri[2].colour.r()) * bary_ws);
                        let pixel_vert_greens_f32 = (f32x16::splat(tri[0].colour.g()) * bary_us)
                            + (f32x16::splat(tri[1].colour.g()) * bary_vs)
                            + (f32x16::splat(tri[2].colour.g()) * bary_ws);
                        let pixel_vert_blues_f32 = (f32x16::splat(tri[0].colour.b()) * bary_us)
                            + (f32x16::splat(tri[1].colour.b()) * bary_vs)
                            + (f32x16::splat(tri[2].colour.b()) * bary_ws);
                        let pixel_vert_alphas_f32 = (f32x16::splat(tri[0].colour.a()) * bary_us)
                            + (f32x16::splat(tri[1].colour.a()) * bary_vs)
                            + (f32x16::splat(tri[2].colour.a()) * bary_ws);
                        // Interpolate UVs.
                        let pixel_us = (f32x16::splat(tri[0].uv.x) * bary_us)
                            + (f32x16::splat(tri[1].uv.x) * bary_vs)
                            + (f32x16::splat(tri[2].uv.x) * bary_ws);
                        let pixel_vs = (f32x16::splat(tri[0].uv.y) * bary_us)
                            + (f32x16::splat(tri[1].uv.y) * bary_vs)
                            + (f32x16::splat(tri[2].uv.y) * bary_ws);
                        // Convert vertex RGBAs to Unorm8s.
                        let pixel_vert_reds = f32x16_to_unorm8x16(pixel_vert_reds_f32);
                        let pixel_vert_greens = f32x16_to_unorm8x16(pixel_vert_greens_f32);
                        let pixel_vert_blues = f32x16_to_unorm8x16(pixel_vert_blues_f32);
                        let pixel_vert_alphas = f32x16_to_unorm8x16(pixel_vert_alphas_f32);
                        // Sample image for each pixel.
                        let [
                            pixel_tex_reds,
                            pixel_tex_greens,
                            pixel_tex_blues,
                            pixel_tex_alphas,
                        ] = image.sample_simd16(pixel_us, pixel_vs);
                        // Calculate final pixel colours to be blended.
                        let src_pixel_reds = pixel_vert_reds * pixel_tex_reds;
                        let src_pixel_greens = pixel_vert_greens * pixel_tex_greens;
                        let src_pixel_blues = pixel_vert_blues * pixel_tex_blues;
                        let src_pixel_alphas = pixel_vert_alphas * pixel_tex_alphas;
                        // Blend the source and destination pixel group colours.
                        let [
                            dst_pixel_reds,
                            dst_pixel_greens,
                            dst_pixel_blues,
                            dst_pixel_alphas,
                        ] = rgba_u32x16_to_unorm8x16(*pixel_group);
                        let one_minus_src_alphas = Unorm8x16::ONES - src_pixel_alphas;
                        let new_pixel_reds =
                            src_pixel_reds + (dst_pixel_reds * one_minus_src_alphas);
                        let new_pixel_greens =
                            src_pixel_greens + (dst_pixel_greens * one_minus_src_alphas);
                        let new_pixel_blues =
                            src_pixel_blues + (dst_pixel_blues * one_minus_src_alphas);
                        let new_pixel_alphas =
                            (src_pixel_alphas * one_minus_src_alphas) + dst_pixel_alphas;
                        // Convert blended pixel colours to RGBA8, and write back.
                        *pixel_group = rgba_4xunorm8x16_to_u32x16([
                            new_pixel_reds,
                            new_pixel_greens,
                            new_pixel_blues,
                            new_pixel_alphas,
                        ]);
                    }
                }
            }
            TileDrawCommandType::PartialTile => {
                // Recombine clipping rect and triangle clip edges.
                let combined_clip_edges: [ClipEdge; 7] = unsafe {
                    let mut out = [core::mem::MaybeUninit::uninit(); 7];
                    for (i, edge) in mesh
                        .clip_rect_edges
                        .iter()
                        .chain(&tri_info.tri_edges)
                        .enumerate()
                    {
                        out[i].write(*edge);
                    }
                    out.map(|edge| edge.assume_init())
                };
                #[rustfmt::skip]
                let x_offsets = u32x16::from_array([
                    0, 1, 2, 3,
                    0, 1, 2, 3,
                    0, 1, 2, 3,
                    0, 1, 2, 3,
                ]);
                #[rustfmt::skip]
                let y_offsets = u32x16::from_array([
                    0, 0, 0, 0,
                    1, 1, 1, 1,
                    2, 2, 2, 2,
                    3, 3, 3, 3,
                ]);
                let micro_tile_start_xs = u32x16::splat(tile_start_x as u32)
                    + (x_offsets * u32x16::splat(RENDER_MICRO_TILE_PIXEL_DIM as u32));
                let micro_tile_start_ys = u32x16::splat(tile_start_y as u32)
                    + (y_offsets * u32x16::splat(RENDER_MICRO_TILE_PIXEL_DIM as u32));
                // Find micro-tiles partially or fully inside every clip edge.
                // TODO: Might be worth doing trivial accept checks at each level as well.
                let mut visible_micro_tiles = mask32x16::splat(true);
                for edge in &combined_clip_edges {
                    // Calculate trivial reject corner positions for this (micro-tile, edge)
                    // combination.
                    let micro_tile_trivial_reject_xs = micro_tile_start_xs
                        + u32x16::splat(
                            RENDER_MICRO_TILE_PIXEL_DIM as u32
                                * edge.flags.trivial_reject_offset_x() as u32,
                        );
                    let micro_tile_trivial_reject_ys = micro_tile_start_ys
                        + u32x16::splat(
                            RENDER_MICRO_TILE_PIXEL_DIM as u32
                                * edge.flags.trivial_reject_offset_y() as u32,
                        );
                    // Test the trivial reject corner.
                    // If this corner lies outside of the edge's half-plane, then we can be certain
                    // that the entire micro-tile lies outside the triangle, and can be skipped.
                    let [a, b, c] = edge.half_plane_params;
                    let reject_test_vals = (f32x16::splat(a)
                        * micro_tile_trivial_reject_xs.cast::<f32>())
                        + (f32x16::splat(b) * micro_tile_trivial_reject_ys.cast::<f32>());
                    let edge_visible_micro_tiles = if edge.flags.is_half_plane_closed() {
                        reject_test_vals.simd_le(f32x16::splat(c))
                    } else {
                        reject_test_vals.simd_lt(f32x16::splat(c))
                    };
                    visible_micro_tiles &= edge_visible_micro_tiles;
                }
                // Iterate through visible micro-tiles.
                while let Some(micro_tile_i) = visible_micro_tiles.first_set() {
                    visible_micro_tiles.set(micro_tile_i, false);
                    let micro_tile_y = micro_tile_i / RENDER_MICRO_TILE_DIM;
                    let micro_tile_x = micro_tile_i % RENDER_MICRO_TILE_DIM;
                    let micro_tile: &mut RenderMicroTileRgba =
                        &mut out_tile[micro_tile_y][micro_tile_x];
                    let micro_tile_start_x = micro_tile_start_xs[micro_tile_i];
                    let micro_tile_start_y = micro_tile_start_ys[micro_tile_i];
                    let pixel_group_start_xs = u32x16::splat(micro_tile_start_x)
                        + (x_offsets * u32x16::splat(RENDER_PIXEL_GROUP_DIM as u32));
                    let pixel_group_start_ys = u32x16::splat(micro_tile_start_y)
                        + (y_offsets * u32x16::splat(RENDER_PIXEL_GROUP_DIM as u32));
                    // Find pixel groups partially or fully inside every clip edge.
                    let mut visible_pixel_groups = mask32x16::splat(true);
                    for edge in &combined_clip_edges {
                        // Calculate trivial reject corner positions for this (pixel group, edge)
                        // combination.
                        let pixel_group_trivial_reject_xs = pixel_group_start_xs
                            + u32x16::splat(
                                RENDER_PIXEL_GROUP_DIM as u32
                                    * edge.flags.trivial_reject_offset_x() as u32,
                            );
                        let pixel_group_trivial_reject_ys = pixel_group_start_ys
                            + u32x16::splat(
                                RENDER_PIXEL_GROUP_DIM as u32
                                    * edge.flags.trivial_reject_offset_y() as u32,
                            );
                        // Test the trivial reject corner.
                        // If this corner lies outside of the edge's half-plane, then we can be
                        // certain that the entire pixel group lies outside the triangle, and can
                        // be skipped.
                        let [a, b, c] = edge.half_plane_params;
                        let reject_test_vals = (f32x16::splat(a)
                            * pixel_group_trivial_reject_xs.cast::<f32>())
                            + (f32x16::splat(b) * pixel_group_trivial_reject_ys.cast::<f32>());
                        let edge_visible_pixel_groups = if edge.flags.is_half_plane_closed() {
                            reject_test_vals.simd_le(f32x16::splat(c))
                        } else {
                            reject_test_vals.simd_lt(f32x16::splat(c))
                        };
                        visible_pixel_groups &= edge_visible_pixel_groups;
                    }
                    // Iterate through visible pixel groups.
                    while let Some(pixel_group_i) = visible_pixel_groups.first_set() {
                        visible_pixel_groups.set(pixel_group_i, false);
                        let pixel_group_y = pixel_group_i / RENDER_PIXEL_GROUP_DIM;
                        let pixel_group_x = pixel_group_i % RENDER_PIXEL_GROUP_DIM;
                        let pixel_group: &mut RenderPixelGroupRgba =
                            &mut micro_tile[pixel_group_y][pixel_group_x];
                        let pixel_group_start_x = pixel_group_start_xs[pixel_group_i];
                        let pixel_group_start_y = pixel_group_start_ys[pixel_group_i];
                        let pixel_xs = (u32x16::splat(pixel_group_start_x) + x_offsets)
                            .cast::<f32>()
                            + f32x16::splat(0.5);
                        let pixel_ys = (u32x16::splat(pixel_group_start_y) + y_offsets)
                            .cast::<f32>()
                            + f32x16::splat(0.5);
                        // Find pixels inside all clip edges.
                        let mut visible_pixels = mask32x16::splat(true);
                        for edge in &combined_clip_edges {
                            // Test the trivial reject corner.
                            // If this corner lies outside of the edge's half-plane, then we can be
                            // certain that the entire pixel lies outside the triangle, and can be
                            // skipped.
                            let [a, b, c] = edge.half_plane_params;
                            let reject_test_vals =
                                (f32x16::splat(a) * pixel_xs) + (f32x16::splat(b) * pixel_ys);
                            let edge_visible_pixels = if edge.flags.is_half_plane_closed() {
                                reject_test_vals.simd_le(f32x16::splat(c))
                            } else {
                                reject_test_vals.simd_lt(f32x16::splat(c))
                            };
                            visible_pixels &= edge_visible_pixels;
                        }
                        // Calculate barycentrics per pixel.
                        let (bary_us, bary_vs, bary_ws) = {
                            // Based on this implementation:
                            // <https://gamedev.stackexchange.com/a/63203>
                            let v0: egui::Vec2 = tri[1].pos - tri[0].pos;
                            let v1: egui::Vec2 = tri[2].pos - tri[0].pos;
                            let v2_xs: f32x16 = pixel_xs - f32x16::splat(tri[0].pos.x);
                            let v2_ys: f32x16 = pixel_ys - f32x16::splat(tri[0].pos.y);
                            let denom_recip: f32 = 1.0 / ((v0.x * v1.y) - (v1.x * v0.y));
                            let vs: f32x16 = ((v2_xs * f32x16::splat(v1.y))
                                - (v2_ys * f32x16::splat(v1.x)))
                                * f32x16::splat(denom_recip);
                            let ws: f32x16 = ((v2_ys * f32x16::splat(v0.x))
                                - (v2_xs * f32x16::splat(v0.y)))
                                * f32x16::splat(denom_recip);
                            let us: f32x16 = f32x16::splat(1.0) - vs - ws;
                            (us, vs, ws)
                        };
                        // Interpolate vertex RGBAs using barycentrics.
                        let pixel_vert_reds_f32 = (f32x16::splat(tri[0].colour.r()) * bary_us)
                            + (f32x16::splat(tri[1].colour.r()) * bary_vs)
                            + (f32x16::splat(tri[2].colour.r()) * bary_ws);
                        let pixel_vert_greens_f32 = (f32x16::splat(tri[0].colour.g()) * bary_us)
                            + (f32x16::splat(tri[1].colour.g()) * bary_vs)
                            + (f32x16::splat(tri[2].colour.g()) * bary_ws);
                        let pixel_vert_blues_f32 = (f32x16::splat(tri[0].colour.b()) * bary_us)
                            + (f32x16::splat(tri[1].colour.b()) * bary_vs)
                            + (f32x16::splat(tri[2].colour.b()) * bary_ws);
                        let pixel_vert_alphas_f32 = (f32x16::splat(tri[0].colour.a()) * bary_us)
                            + (f32x16::splat(tri[1].colour.a()) * bary_vs)
                            + (f32x16::splat(tri[2].colour.a()) * bary_ws);
                        // Interpolate UVs.
                        let pixel_us = (f32x16::splat(tri[0].uv.x) * bary_us)
                            + (f32x16::splat(tri[1].uv.x) * bary_vs)
                            + (f32x16::splat(tri[2].uv.x) * bary_ws);
                        let pixel_vs = (f32x16::splat(tri[0].uv.y) * bary_us)
                            + (f32x16::splat(tri[1].uv.y) * bary_vs)
                            + (f32x16::splat(tri[2].uv.y) * bary_ws);
                        // Convert vertex RGBAs to Unorm8s.
                        let pixel_vert_reds = f32x16_to_unorm8x16(pixel_vert_reds_f32);
                        let pixel_vert_greens = f32x16_to_unorm8x16(pixel_vert_greens_f32);
                        let pixel_vert_blues = f32x16_to_unorm8x16(pixel_vert_blues_f32);
                        let pixel_vert_alphas = f32x16_to_unorm8x16(pixel_vert_alphas_f32);
                        // Sample image for each pixel.
                        let [
                            pixel_tex_reds,
                            pixel_tex_greens,
                            pixel_tex_blues,
                            pixel_tex_alphas,
                        ] = image.sample_simd16_masked(pixel_us, pixel_vs, visible_pixels);
                        // Calculate final pixel colours to be blended.
                        let src_pixel_reds = pixel_vert_reds * pixel_tex_reds;
                        let src_pixel_greens = pixel_vert_greens * pixel_tex_greens;
                        let src_pixel_blues = pixel_vert_blues * pixel_tex_blues;
                        let src_pixel_alphas = pixel_vert_alphas * pixel_tex_alphas;
                        // Blend the source and destination pixel group colours.
                        let dst_pixel_group = *pixel_group;
                        let [
                            dst_pixel_reds,
                            dst_pixel_greens,
                            dst_pixel_blues,
                            dst_pixel_alphas,
                        ] = rgba_u32x16_to_unorm8x16(dst_pixel_group);
                        let one_minus_src_alphas = Unorm8x16::ONES - src_pixel_alphas;
                        let new_pixel_reds =
                            src_pixel_reds + (dst_pixel_reds * one_minus_src_alphas);
                        let new_pixel_greens =
                            src_pixel_greens + (dst_pixel_greens * one_minus_src_alphas);
                        let new_pixel_blues =
                            src_pixel_blues + (dst_pixel_blues * one_minus_src_alphas);
                        let new_pixel_alphas =
                            (src_pixel_alphas * one_minus_src_alphas) + dst_pixel_alphas;
                        // Convert blended pixel colours to RGBA8, and write back enabled pixels.
                        let new_pixel_group = rgba_4xunorm8x16_to_u32x16([
                            new_pixel_reds,
                            new_pixel_greens,
                            new_pixel_blues,
                            new_pixel_alphas,
                        ]);
                        *pixel_group = visible_pixels.select(new_pixel_group, dst_pixel_group);
                    }
                }
            }
        }
    }
}
