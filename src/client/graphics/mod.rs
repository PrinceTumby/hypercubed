cfg_if::cfg_if! {
    if #[cfg(feature = "graphics_backend_vulkan")] {
        mod backend_vulkan;
        pub use backend_vulkan::*;
    } else if #[cfg(feature = "graphics_backend_wgpu")] {
        mod backend_wgpu;
        pub use backend_wgpu::*;
    } else if #[cfg(feature = "graphics_backend_software")] {
        mod backend_software;
        pub use backend_software::*;
    } else if #[cfg(feature = "platform_ps2")] {
        mod backend_ps2;
        pub use backend_ps2::*;
    } else if #[cfg(feature = "platform_opengl_mac_tiger")] {
        mod backend_opengl_mac_tiger;
        pub use backend_opengl_mac_tiger::*;
    } else if #[cfg(feature = "platform_w2c2_opengl_mac")] {
        mod backend_w2c2_opengl_mac;
        pub use backend_w2c2_opengl_mac::*;
    } else {
        compile_error!("A graphics backend feature must be enabled.");
    }
}

use nalgebra::{Isometry3, Matrix4, Perspective3, Point3, UnitQuaternion, Vector3};

pub const DEFAULT_FOV: f32 = 80.0;
pub const DEFAULT_ZNEAR: f32 = 0.01;
pub const DEFAULT_ZFAR: f32 = 1024.0;

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub pos: Point3<f32>,
    pub proj_matrix: Perspective3<f32>,
    /// Represented in degrees.
    pub yaw: f32,
    /// Represented in degrees.
    pub pitch: f32,
    /// Represented in degrees.
    pub roll: f32,
}

impl Camera {
    pub fn get_rot(&self) -> UnitQuaternion<f32> {
        UnitQuaternion::from_euler_angles(
            self.pitch.to_radians(),
            -self.yaw.to_radians(),
            -self.roll.to_radians(),
        )
    }

    pub fn generate_view_matrix(&self) -> Matrix4<f32> {
        let translate = Isometry3::new(self.pos.coords, nalgebra::zero())
            .inverse()
            .to_matrix();
        let rotate = self.get_rot().inverse().to_homogeneous();
        self.proj_matrix.as_matrix() * rotate * translate
    }

    pub fn generate_view_matrix_slice(&self) -> [[f32; 4]; 4] {
        *self.generate_view_matrix().as_ref()
    }

    pub fn generate_reversed_depth_view_matrix(&self) -> Matrix4<f32> {
        // Using a standard depth buffer had issues with Z-fighting on faraway objects (snow
        // clipping through spruce leaves from high enough up was a particularly bad case).
        Matrix4::new_nonuniform_scaling(&Vector3::new(1.0, 1.0, -0.5))
            .append_translation(&Vector3::new(0.0, 0.0, 0.5))
            * self.generate_view_matrix()
    }

    pub fn generate_reversed_depth_view_matrix_slice(&self) -> [[f32; 4]; 4] {
        *self.generate_reversed_depth_view_matrix().as_ref()
    }

    pub fn generate_debug_crosshair_view_matrix_slice(&self) -> [[f32; 4]; 4] {
        let fake_pos = Point3::from(self.get_rot() * Vector3::z().scale(30.0));
        let up = self.get_rot() * Vector3::y();
        let look_at = Matrix4::look_at_rh(&fake_pos, &Point3::origin(), &up);
        let view_matrix = self.proj_matrix.as_matrix() * look_at;
        *view_matrix.as_ref()
    }

    /// Generates a normal and offset for each view clipping plane.
    /// Planes are in order of left, right, bottom, top, near, far.
    pub fn generate_clipping_planes(&self) -> [(Vector3<f32>, f32); 6] {
        /// Converts constants from a plane equation to a normal vector and offset
        fn convert_abcd(a: f32, b: f32, c: f32, d: f32) -> (Vector3<f32>, f32) {
            let normal = Vector3::new(a, b, c);
            let normal_len = normal.magnitude();
            (normal / normal_len, d / normal_len)
        }
        let m = self.generate_view_matrix();
        [
            // Left
            convert_abcd(m.m41 + m.m11, m.m42 + m.m12, m.m43 + m.m13, m.m44 + m.m14),
            // Right
            convert_abcd(m.m41 - m.m11, m.m42 - m.m12, m.m43 - m.m13, m.m44 - m.m14),
            // Bottom
            convert_abcd(m.m41 + m.m21, m.m42 + m.m22, m.m43 + m.m23, m.m44 + m.m24),
            // Top
            convert_abcd(m.m41 - m.m21, m.m42 - m.m22, m.m43 - m.m23, m.m44 - m.m24),
            // Near
            convert_abcd(m.m41 + m.m31, m.m42 + m.m32, m.m43 + m.m33, m.m44 + m.m34),
            // Far
            convert_abcd(m.m41 - m.m31, m.m42 - m.m32, m.m43 - m.m33, m.m44 - m.m34),
        ]
    }
}
