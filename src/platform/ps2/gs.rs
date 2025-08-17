use super::draw;
use super::gif::GsRegisterPacketData;
use super::display::PixelStorageMethod;
use bitfield::bitfield;

/// Fixed point 12.4 number.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fixed12P4(pub u16);

impl Fixed12P4 {
    pub const ZERO: Self = Self(0);

    pub fn from_u16(x: u16) -> Self {
        Self(x << 4)
    }
}

/// Fixed point 28.4 number.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fixed28P4(pub u32);

impl Fixed28P4 {
    pub const ZERO: Self = Self(0);

    pub fn from_u32(x: u32) -> Self {
        Self(x << 4)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DepthTestMethod {
    AlwaysFail = 0,
    AlwaysPass = 1,
    GreaterThanOrEqual = 2,
    GreaterThan = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimitiveType {
    Point = 0,
    Line = 1,
    LineStrip = 2,
    Triangle = 3,
    TriangleStrip = 4,
    TriangleFan = 5,
    Sprite = 6,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlphaTestMethod {
    AlwaysFail = 0,
    AlwaysPass = 1,
    LessThanAlphaRef = 2,
    LessThanOrEqualAlphaRef = 3,
    EqualAlphaRef = 4,
    GreaterThanOrEqualAlphaRef = 5,
    GreaterThanAlphaRef = 6,
    NotEqualAlphaRef = 7,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlphaFailMode {
    UpdateNeither = 0,
    UpdateFramebufferRgba = 1,
    UpdateZBuffer = 2,
    UpdateFramebufferRgb = 3,
}

pub mod tag {
    use super::*;

    macro_rules! impl_gs_reg_packet_data {
        ($name:ident, $reg_num:expr) => {
            unsafe impl GsRegisterPacketData for $name {
                fn register_num(&self) -> u64 {
                    $reg_num
                }

                fn to_dword(&self) -> u64 {
                    self.0.into()
                }
            }
        };
        ($name:ident, $reg_num:expr, $to_dword_fn:item) => {
            unsafe impl GsRegisterPacketData for $name {
                fn register_num(&self) -> u64 {
                    $reg_num
                }

                $to_dword_fn
            }
        };
    }

    // Primitives

    bitfield! {
        #[derive(Clone, Copy)]
        pub struct SetPrimitive(u64);
        impl Debug;
        u8;
        pub get_primitive_type_raw, set_primitive_type_raw: 2, 0;
        pub get_gouraud_shading_enabled, set_gouraud_shading_enabled: 3;
        pub get_texture_mapping_enabled, set_texture_mapping_enabled: 4;
        pub get_fog_enabled, set_fog_enabled: 5;
        pub get_alpha_blending_enabled, set_alpha_blending_enabled: 6;
        pub get_antialiasing_enabled, set_antialiasing_enabled: 7;
        pub get_use_uv_coords, set_use_uv_coords: 8;
        pub get_use_context_2_regs, set_use_context_2_regs: 9;
        pub get_fix_fragment_value_enabled, set_fix_fragment_value_enabled: 10;
    }

    impl_gs_reg_packet_data!(SetPrimitive, 0x00);

    impl SetPrimitive {
        pub fn get_primitive_type(&self) -> PrimitiveType {
            match self.get_primitive_type_raw() {
                0 => PrimitiveType::Point,
                1 => PrimitiveType::Line,
                2 => PrimitiveType::LineStrip,
                3 => PrimitiveType::Triangle,
                4 => PrimitiveType::TriangleStrip,
                5 => PrimitiveType::TriangleFan,
                6 => PrimitiveType::Sprite,
                _ => unreachable!(),
            }
        }

        pub fn set_primitive_type(&mut self, prim_type: PrimitiveType) {
            self.set_primitive_type_raw(prim_type as u8);
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct SetUseMainPrimitive(pub bool);

    impl_gs_reg_packet_data!(SetUseMainPrimitive, 0x1A);

    bitfield! {
        #[derive(Clone, Copy)]
        pub struct SetAlternativePrimitive(u64);
        impl Debug;
        u8;
        pub get_gouraud_shading_enabled, set_gouraud_shading_enabled: 3;
        pub get_texture_mapping_enabled, set_texture_mapping_enabled: 4;
        pub get_fog_enabled, set_fog_enabled: 5;
        pub get_alpha_blending_enabled, set_alpha_blending_enabled: 6;
        pub get_antialiasing_enabled, set_antialiasing_enabled: 7;
        pub get_use_uv_coords, set_use_uv_coords: 8;
        pub get_use_context_2_regs, set_use_context_2_regs: 9;
        pub get_fix_fragment_value_enabled, set_fix_fragment_value_enabled: 10;
    }

    impl_gs_reg_packet_data!(SetAlternativePrimitive, 0x1B);

    // Vertex Attributes

    #[derive(Clone, Copy)]
    pub struct SetRGBAQ {
        pub r: u8,
        pub g: u8,
        pub b: u8,
        pub a: u8,
        pub q: u32,
    }

    impl_gs_reg_packet_data!(
        SetRGBAQ,
        0x01,
        fn to_dword(&self) -> u64 {
            (self.r as u64)
                | ((self.g as u64) << 8)
                | ((self.b as u64) << 16)
                | ((self.a as u64) << 24)
                | ((self.q as u64) << 32)
        }
    );

    #[derive(Clone, Copy)]
    pub struct KickVertexXYZ2 {
        pub x: Fixed12P4,
        pub y: Fixed12P4,
        pub z: u32,
    }

    impl_gs_reg_packet_data!(
        KickVertexXYZ2,
        0x05,
        fn to_dword(&self) -> u64 {
            (self.x.0 as u64) | ((self.y.0 as u64) << 16) | ((self.z as u64) << 32)
        }
    );

    #[derive(Clone, Copy)]
    pub struct SetXYOffset1 {
        pub x: Fixed28P4,
        pub y: Fixed28P4,
    }

    impl_gs_reg_packet_data!(
        SetXYOffset1,
        0x18,
        fn to_dword(&self) -> u64 {
            (self.x.0 as u64) | ((self.y.0 as u64) << 32)
        }
    );

    impl SetXYOffset1 {
        pub fn to_xy_offset_2_packet_data(&self) -> [u64; 2] {
            [self.to_dword(), self.register_num() + 1]
        }
    }

    // Framebuffers and Z Buffers

    bitfield! {
        #[derive(Clone, Copy)]
        pub struct SetFramebuffer1(u64);
        impl Debug;
        u32;
        pub get_shifted_base_address, set_shifted_base_address: 8, 0;
        pub get_shifted_buffer_width, set_shifted_buffer_width: 21, 16;
        u8;
        pub get_format_raw, set_format_raw: 29, 24;
        u32;
        pub get_mask, set_mask: 63, 32;
    }

    impl_gs_reg_packet_data!(SetFramebuffer1, 0x4C);

    impl SetFramebuffer1 {
        pub fn new(framebuffer: &draw::Framebuffer) -> Self {
            let mut out = Self(0);
            out.set_shifted_base_address(framebuffer.get_address() >> 11);
            out.set_shifted_buffer_width(framebuffer.get_width() >> 6);
            out.set_format_raw(framebuffer.get_psm() as u8);
            out.set_mask(framebuffer.get_mask());
            out
        }

        pub fn to_framebuffer_2_packet_data(&self) -> [u64; 2] {
            [self.to_dword(), self.register_num() + 1]
        }
    }

    bitfield! {
        #[derive(Clone, Copy)]
        pub struct SetZBuffer1(u64);
        impl Debug;
        u32;
        pub get_shifted_base_address, set_shifted_base_address: 8, 0;
        u8;
        pub get_format_raw, set_format_raw: 27, 24;
        pub get_masked, set_masked: 32;
    }

    impl_gs_reg_packet_data!(SetZBuffer1, 0x4E);

    impl SetZBuffer1 {
        pub fn new(z_buffer: &draw::ZBuffer) -> Self {
            let mut out = Self(0);
            out.set_shifted_base_address(z_buffer.get_address() / 2048);
            out.set_format_raw(match z_buffer.get_zsm() {
                PixelStorageMethod::PsmZ32 => 0x00,
                PixelStorageMethod::PsmZ24 => 0x01,
                PixelStorageMethod::PsmZ16 => 0x02,
                PixelStorageMethod::PsmZ16S => 0x0A,
                format => panic!("Pixel format `{format:?}` invalid for Z Buffer"),
            });
            out.set_masked(z_buffer.get_masked());
            out
        }

        pub fn to_z_buffer_2_packet_data(&self) -> [u64; 2] {
            [self.to_dword(), self.register_num() + 1]
        }
    }

    // Dithering

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct SetDitheringEnabled(pub bool);

    impl_gs_reg_packet_data!(SetDitheringEnabled, 0x45);

    // Alpha Blending

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct SetColourClampingEnabled(pub bool);

    impl_gs_reg_packet_data!(SetColourClampingEnabled, 0x46);

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct SetAlphaCorrection1(pub AlphaCorrectionMode);

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum AlphaCorrectionMode {
        Rgba32 = 0,
        Rgba16 = 1,
    }

    impl_gs_reg_packet_data!(
        SetAlphaCorrection1,
        0x4A,
        fn to_dword(&self) -> u64 {
            self.0 as u64
        }
    );

    // Pixel Testing

    bitfield! {
        #[derive(Clone, Copy)]
        pub struct SetScissor1(u64);
        impl Debug;
        u16;
        pub get_x_min, set_x_min: 10, 0;
        pub get_x_max, set_x_max: 26, 16;
        pub get_y_min, set_y_min: 42, 32;
        pub get_y_max, set_y_max: 58, 48;
    }

    impl_gs_reg_packet_data!(SetScissor1, 0x40);

    impl SetScissor1 {
        pub fn new(x_min: u16, x_max: u16, y_min: u16, y_max: u16) -> Self {
            let mut out = Self(0);
            out.set_x_min(x_min);
            out.set_x_max(x_max);
            out.set_y_min(y_min);
            out.set_y_max(y_max);
            out
        }

        pub fn to_reg_2_packet_data(&self) -> [u64; 2] {
            [self.to_dword(), self.register_num() + 1]
        }
    }

    bitfield! {
        #[derive(Clone, Copy)]
        pub struct SetPixelTesting1(u64);
        impl Debug;
        u8;
        pub get_alpha_testing_enabled, set_alpha_testing_enabled: 0;
        pub get_alpha_test_method_raw, set_alpha_test_method_raw: 3, 1;
        pub get_alpha_ref, set_alpha_ref: 11, 4;
        pub get_alpha_fail_mode_raw, set_alpha_fail_mode_raw: 13, 12;
        pub get_dest_alpha_testing_enabled, set_dest_alpha_testing_enabled: 14;
        pub get_dest_alpha_pass_enabled, set_dest_alpha_pass_enabled: 15;
        pub get_depth_test_enabled, set_depth_test_enabled: 16;
        pub get_depth_test_method_raw, set_depth_test_method_raw: 18, 17;
    }

    impl_gs_reg_packet_data!(SetPixelTesting1, 0x47);

    impl SetPixelTesting1 {
        pub fn get_alpha_test_method(&self) -> AlphaTestMethod {
            match self.get_alpha_test_method_raw() {
                0 => AlphaTestMethod::AlwaysFail,
                1 => AlphaTestMethod::AlwaysPass,
                2 => AlphaTestMethod::LessThanAlphaRef,
                3 => AlphaTestMethod::LessThanOrEqualAlphaRef,
                4 => AlphaTestMethod::EqualAlphaRef,
                5 => AlphaTestMethod::GreaterThanOrEqualAlphaRef,
                6 => AlphaTestMethod::GreaterThanAlphaRef,
                7 => AlphaTestMethod::NotEqualAlphaRef,
                _ => unreachable!(),
            }
        }

        pub fn set_alpha_test_method(&mut self, method: AlphaTestMethod) {
            self.set_alpha_test_method_raw(method as u8);
        }

        pub fn get_alpha_fail_mode(&self) -> AlphaFailMode {
            match self.get_alpha_fail_mode_raw() {
                0 => AlphaFailMode::UpdateNeither,
                1 => AlphaFailMode::UpdateFramebufferRgba,
                2 => AlphaFailMode::UpdateZBuffer,
                3 => AlphaFailMode::UpdateFramebufferRgb,
                _ => unreachable!(),
            }
        }

        pub fn set_alpha_fail_mode(&mut self, method: AlphaFailMode) {
            self.set_alpha_fail_mode_raw(method as u8);
        }

        pub fn get_depth_test_method(&self) -> DepthTestMethod {
            match self.get_depth_test_method_raw() {
                0 => DepthTestMethod::AlwaysFail,
                1 => DepthTestMethod::AlwaysPass,
                2 => DepthTestMethod::GreaterThanOrEqual,
                3 => DepthTestMethod::GreaterThan,
                _ => unreachable!(),
            }
        }

        pub fn set_depth_test_method(&mut self, method: DepthTestMethod) {
            self.set_depth_test_method_raw(method as u8);
        }
    }

    // Configuration Finish

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Finished;

    impl_gs_reg_packet_data!(
        Finished,
        0x61,
        fn to_dword(&self) -> u64 {
            true as u64
        }
    );
}
