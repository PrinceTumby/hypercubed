use fast_srgb8::{f32x4_to_srgb8, srgb8_to_f32};
use nalgebra::Point2;

#[derive(Clone, Copy, Debug)]
pub struct ClipRect {
    pub min: Point2<f32>,
    pub max: Point2<f32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba {
    #[inline(always)]
    pub fn half_blend(&self, rgb_coef: f32, a_coef: f32) -> Self {
        Self {
            r: self.r * rgb_coef,
            g: self.g * rgb_coef,
            b: self.b * rgb_coef,
            a: self.a * a_coef,
        }
    }

    #[inline(always)]
    pub fn from_srgba8(value: image::Rgba<u8>) -> Self {
        let [r, g, b, a] = value.0;
        let [r, g, b] = [r, g, b].map(srgb8_to_f32);
        let a = (a as f32) / 255.0;
        Self { r, g, b, a }
    }

    #[inline(always)]
    pub fn from_linear_rgba8(value: image::Rgba<u8>) -> Self {
        let [r, g, b, a] = value.0.map(|n| (n as f32) / 255.0);
        Self { r, g, b, a }
    }

    #[inline(always)]
    pub fn to_linear_rgba8(self) -> image::Rgba<u8> {
        let array = [self.r, self.g, self.b, self.a];
        image::Rgba(array.map(|n| (n.clamp(0.0, 1.0) * 255.0) as u8))
    }

    #[inline(always)]
    pub fn to_srgba8(self) -> image::Rgba<u8> {
        let [r, g, b, _] = f32x4_to_srgb8([self.r, self.g, self.b, 0.0]);
        image::Rgba([r, g, b, (self.a.clamp(0.0, 1.0) * 255.0) as u8])
    }
}

impl From<egui::Color32> for Rgba {
    fn from(value: egui::Color32) -> Self {
        let [r, g, b, a] = value.to_srgba_unmultiplied();
        let [r, g, b] = [r, g, b].map(srgb8_to_f32);
        let a = (a as f32) / 255.0;
        Self { r, g, b, a }
    }
}

impl std::ops::Add for Rgba {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            r: self.r + rhs.r,
            g: self.g + rhs.g,
            b: self.b + rhs.b,
            a: self.a + rhs.a,
        }
    }
}

impl std::ops::Sub for Rgba {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            r: self.r - rhs.r,
            g: self.g - rhs.g,
            b: self.b - rhs.b,
            a: self.a - rhs.a,
        }
    }
}

impl std::ops::Mul for Rgba {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            r: self.r * rhs.r,
            g: self.g * rhs.g,
            b: self.b * rhs.b,
            a: self.a * rhs.a,
        }
    }
}

impl std::ops::Mul<f32> for Rgba {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self {
            r: self.r * rhs,
            g: self.g * rhs,
            b: self.b * rhs,
            a: self.a * rhs,
        }
    }
}

impl std::ops::Div<f32> for Rgba {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self {
            r: self.r / rhs,
            g: self.g / rhs,
            b: self.b / rhs,
            a: self.a / rhs,
        }
    }
}

impl std::ops::AddAssign for Rgba {
    fn add_assign(&mut self, rhs: Self) {
        self.r += rhs.r;
        self.g += rhs.g;
        self.b += rhs.b;
        self.a += rhs.a;
    }
}

// pub fn render_egui_mesh(
//     out_render: &mut impl image::GenericImage<Pixel = image::Rgba<u8>>,
//     texture: &super::egui_renderer::ImageData,
//     clip_rect: ClipRect,
//     vertices: &[egui::epaint::Vertex],
//     indices: &[u32],
// ) {
//     let mut working_tris = Vec::with_capacity(16);
//     let mut clipped_tris = Vec::with_capacity(16);
//     for tri_indices in indices.chunks(3) {
//         let &[idx_0, idx_1, idx_2] = tri_indices else {
//             unreachable!();
//         };
//         // Load triangle vertices
//         let unclipped_tri = [idx_0, idx_1, idx_2].map(|i| vertices[i as usize]);
//         // Clip vertices
//         working_tris.clear();
//         clipped_tris.clear();
//         working_tris.push(unclipped_tri);
//         fn calc_intersection_x(
//             v0: egui::epaint::Vertex,
//             v1: egui::epaint::Vertex,
//             edge: f32,
//         ) -> egui::epaint::Vertex {
//             let diff = v1.pos.x - v0.pos.x;
//             let gradient_y = (v1.pos.y - v0.pos.y) / diff;
//             let gradient_uvs = (v1.uv - v0.uv) / diff;
//             let v0_rgba = egui::Rgba::from(v0.color).to_array();
//             let v1_rgba = egui::Rgba::from(v1.color).to_array();
//             let gradient_rgba = [
//                 (v1_rgba[0] - v0_rgba[0]) / diff,
//                 (v1_rgba[1] - v0_rgba[1]) / diff,
//                 (v1_rgba[2] - v0_rgba[2]) / diff,
//                 (v1_rgba[3] - v0_rgba[3]) / diff,
//             ];
//             let grad_coef = v0.pos.x - edge;
//             return egui::epaint::Vertex {
//                 pos: egui::Pos2 {
//                     x: edge,
//                     y: v0.pos.y - (gradient_y * grad_coef),
//                 },
//                 uv: v0.uv - (gradient_uvs * grad_coef),
//                 color: egui::Color32::from(egui::Rgba::from_rgba_premultiplied(
//                     v0_rgba[0] - (gradient_rgba[0] * grad_coef),
//                     v0_rgba[1] - (gradient_rgba[1] * grad_coef),
//                     v0_rgba[2] - (gradient_rgba[2] * grad_coef),
//                     v0_rgba[3] - (gradient_rgba[3] * grad_coef),
//                 )),
//             };
//         }
//         fn calc_intersection_y(
//             v0: egui::epaint::Vertex,
//             v1: egui::epaint::Vertex,
//             edge: f32,
//         ) -> egui::epaint::Vertex {
//             let diff = v1.pos.y - v0.pos.y;
//             let gradient_x = (v1.pos.x - v0.pos.x) / diff;
//             let gradient_uvs = (v1.uv - v0.uv) / diff;
//             let v0_rgba = egui::Rgba::from(v0.color).to_array();
//             let v1_rgba = egui::Rgba::from(v1.color).to_array();
//             let gradient_rgba = [
//                 (v1_rgba[0] - v0_rgba[0]) / diff,
//                 (v1_rgba[1] - v0_rgba[1]) / diff,
//                 (v1_rgba[2] - v0_rgba[2]) / diff,
//                 (v1_rgba[3] - v0_rgba[3]) / diff,
//             ];
//             let grad_coef = v0.pos.y - edge;
//             return egui::epaint::Vertex {
//                 pos: egui::Pos2 {
//                     x: v0.pos.x - (gradient_x * grad_coef),
//                     y: edge,
//                 },
//                 uv: v0.uv - (gradient_uvs * grad_coef),
//                 color: egui::Color32::from(egui::Rgba::from_rgba_premultiplied(
//                     v0_rgba[0] - (gradient_rgba[0] * grad_coef),
//                     v0_rgba[1] - (gradient_rgba[1] * grad_coef),
//                     v0_rgba[2] - (gradient_rgba[2] * grad_coef),
//                     v0_rgba[3] - (gradient_rgba[3] * grad_coef),
//                 )),
//             };
//         }
//         // Left edge clip
//         for tri in working_tris.drain(..) {
//             clip_egui_tri(
//                 &mut clipped_tris,
//                 tri,
//                 |v: egui::epaint::Vertex, edge: f32| v.pos.x >= edge,
//                 calc_intersection_x,
//                 clip_rect.min.x,
//             );
//         }
//         std::mem::swap(&mut working_tris, &mut clipped_tris);
//         // Right edge clip
//         for tri in working_tris.drain(..) {
//             clip_egui_tri(
//                 &mut clipped_tris,
//                 tri,
//                 |v: egui::epaint::Vertex, edge: f32| v.pos.x <= edge,
//                 calc_intersection_x,
//                 clip_rect.max.x,
//             );
//         }
//         std::mem::swap(&mut working_tris, &mut clipped_tris);
//         // Top edge clip
//         for tri in working_tris.drain(..) {
//             clip_egui_tri(
//                 &mut clipped_tris,
//                 tri,
//                 |v: egui::epaint::Vertex, edge: f32| v.pos.y >= edge,
//                 calc_intersection_y,
//                 clip_rect.min.y,
//             );
//         }
//         std::mem::swap(&mut working_tris, &mut clipped_tris);
//         // Bottom edge clip
//         for tri in working_tris.drain(..) {
//             clip_egui_tri(
//                 &mut clipped_tris,
//                 tri,
//                 |v: egui::epaint::Vertex, edge: f32| v.pos.y <= edge,
//                 calc_intersection_y,
//                 clip_rect.max.y,
//             );
//         }
//         // Draw triangles
//         for &tri in &clipped_tris {
//             #[derive(Clone, Copy, Debug)]
//             struct ScreenPoint {
//                 x: f32,
//                 y: u32,
//                 u: f32,
//                 v: f32,
//                 rgba: Rgba,
//             }
//             let mut screen_tri = tri.map(|v| ScreenPoint {
//                 x: v.pos.x,
//                 y: v.pos.y as u32,
//                 u: v.uv.x,
//                 v: v.uv.y,
//                 rgba: v.color.into(),
//             });
//             // Sort points in order of Y coordinate, first point should be at top.
//             // If Y coords are equal, sort by X.
//             screen_tri.sort_unstable_by(|p1, p2| p1.y.cmp(&p2.y).then(p1.x.total_cmp(&p2.x)));
//             let [p0, p1, p2] = screen_tri;
//             // Skip triangles without area
//             if p0.y == p2.y {
//                 continue;
//             }
//             #[derive(Clone, Copy, Debug, Default)]
//             struct TexSlopeInfo {
//                 x_current: f32,
//                 x_step: f32,
//                 u_current: f32,
//                 u_step: f32,
//                 v_current: f32,
//                 v_step: f32,
//                 rgba_current: Rgba,
//                 rgba_step: Rgba,
//             }
//             impl TexSlopeInfo {
//                 pub fn new(from: ScreenPoint, to: ScreenPoint) -> Self {
//                     let y_diff = (to.y - from.y) as f32;
//                     Self {
//                         x_current: from.x,
//                         x_step: (to.x - from.x) / y_diff,
//                         u_current: from.u,
//                         u_step: (to.u - from.u) / y_diff,
//                         v_current: from.v,
//                         v_step: (to.v - from.v) / y_diff,
//                         rgba_current: from.rgba,
//                         rgba_step: (to.rgba - from.rgba) / y_diff,
//                     }
//                 }
//
//                 pub fn step(&mut self) {
//                     self.x_current += self.x_step;
//                     self.u_current += self.u_step;
//                     self.v_current += self.v_step;
//                     self.rgba_current += self.rgba_step;
//                 }
//             }
//             let is_short_left =
//                 (p1.y - p0.y) as f32 * (p2.x - p0.x) < (p2.y - p0.y) as f32 * (p1.x - p0.x);
//             let mut sides = [TexSlopeInfo::default(); 2];
//             sides[!is_short_left as usize] = TexSlopeInfo::new(p0, p2);
//             let mut current_y = p0.y;
//             let mut end_y = p0.y;
//             loop {
//                 if current_y >= end_y {
//                     if current_y >= p2.y {
//                         // Finish once we've reached p2
//                         break;
//                     } else {
//                         // Recalculate slope for short side
//                         sides[is_short_left as usize] = if current_y < p1.y {
//                             end_y = p1.y;
//                             TexSlopeInfo::new(p0, p1)
//                         } else {
//                             end_y = p2.y;
//                             TexSlopeInfo::new(p1, p2)
//                         };
//                     }
//                 }
//                 // Draw scanline
//                 // `y` contains the Y coordinate of the scanline
//                 // `sides[0]` contains the left side scanline information
//                 // `sides[1]` contains the right side scanline information
//                 let mut current_x = sides[0].x_current as u32;
//                 let end_x = sides[1].x_current as u32;
//                 let x_diff_f32 = sides[1].x_current - sides[0].x_current;
//                 let mut current_u = sides[0].u_current;
//                 let u_step = (sides[1].u_current - current_u) / x_diff_f32;
//                 let mut current_v = sides[0].v_current;
//                 let v_step = (sides[1].v_current - current_v) / x_diff_f32;
//                 let mut current_rgba = sides[0].rgba_current;
//                 let rgba_step = (sides[1].rgba_current - current_rgba) / x_diff_f32;
//                 while current_x < end_x {
//                     // Sample texture, combine with vertex colour
//                     let tex_sample_raw = texture.sample(current_u, current_v);
//                     let tex_sample = Rgba::from_srgba8(tex_sample_raw);
//                     let colour = tex_sample * current_rgba;
//                     // Blend final colour with output buffer colour
//                     let fb_colour = Rgba::from_srgba8(out_render.get_pixel(current_x, current_y));
//                     out_render.put_pixel(
//                         current_x,
//                         current_y,
//                         (colour.half_blend(colour.a, 1.0 - colour.a)
//                             + fb_colour.half_blend(1.0 - colour.a, colour.a))
//                         .to_srgba8(),
//                     );
//                     // Step to next scanline pixel
//                     current_x += 1;
//                     current_u += u_step;
//                     current_v += v_step;
//                     current_rgba += rgba_step;
//                 }
//                 // Step to next scanline
//                 sides[0].step();
//                 sides[1].step();
//                 current_y += 1;
//             }
//         }
//     }
// }
//
// #[inline]
// fn clip_egui_tri(
//     out_clipped_tris: &mut Vec<[egui::epaint::Vertex; 3]>,
//     tri: [egui::epaint::Vertex; 3],
//     test_point_bounds: impl Fn(egui::epaint::Vertex, f32) -> bool,
//     calc_intersection: impl Fn(egui::epaint::Vertex, egui::epaint::Vertex, f32) -> egui::epaint::Vertex,
//     edge: f32,
// ) {
//     let points_in_bounds = tri.map(|v| test_point_bounds(v, edge));
//     // Seems to produce slightly better code
//     let num_points_in_bounds = match points_in_bounds {
//         [false, false, false] => 0,
//         [false, false, true] | [false, true, false] | [true, false, false] => 1,
//         [false, true, true] | [true, false, true] | [true, true, false] => 2,
//         [true, true, true] => 3,
//     };
//     // let num_points_in_bounds = points_in_bounds.map(|b| b as u8).iter().sum();
//     match num_points_in_bounds {
//         // Don't generate any triangles if all points are out of bounds
//         0 => return,
//         // Generate single clipped triangle if two points are out of bounds
//         1 => {
//             if points_in_bounds[0] {
//                 let new_v1 = calc_intersection(tri[0], tri[1], edge);
//                 let new_v2 = calc_intersection(tri[0], tri[2], edge);
//                 out_clipped_tris.push([tri[0], new_v1, new_v2]);
//             } else if points_in_bounds[1] {
//                 let new_v0 = calc_intersection(tri[1], tri[1], edge);
//                 let new_v2 = calc_intersection(tri[1], tri[2], edge);
//                 out_clipped_tris.push([new_v0, tri[1], new_v2]);
//             } else {
//                 let new_v0 = calc_intersection(tri[2], tri[0], edge);
//                 let new_v1 = calc_intersection(tri[2], tri[1], edge);
//                 out_clipped_tris.push([new_v0, new_v1, tri[2]]);
//             }
//         }
//         // Generate two clipped triangles if one point is out of bounds
//         2 => {
//             if !points_in_bounds[0] {
//                 let new_0 = calc_intersection(tri[1], tri[0], edge);
//                 let new_1 = calc_intersection(tri[2], tri[0], edge);
//                 out_clipped_tris.push([new_0, tri[1], tri[2]]);
//                 out_clipped_tris.push([new_0, tri[2], new_1]);
//             } else if !points_in_bounds[1] {
//                 let new_0 = calc_intersection(tri[2], tri[1], edge);
//                 let new_1 = calc_intersection(tri[0], tri[1], edge);
//                 out_clipped_tris.push([tri[0], new_0, tri[2]]);
//                 out_clipped_tris.push([new_0, tri[0], new_1]);
//             } else {
//                 let new_0 = calc_intersection(tri[0], tri[2], edge);
//                 let new_1 = calc_intersection(tri[1], tri[2], edge);
//                 out_clipped_tris.push([tri[0], tri[1], new_0]);
//                 out_clipped_tris.push([new_0, tri[1], new_1]);
//             }
//         }
//         // No clipping required if no points are out of bounds
//         3 => out_clipped_tris.push(tri),
//         _ => unreachable!(),
//     }
// }
