use super::{asm_helpers, draw};
use core::arch::asm;
use core::task::Poll;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelStorageMethod {
    Psm32 = 0x00,
    Psm24 = 0x01,
    Psm16 = 0x02,
    Psm16S = 0x0A,
    PsmPs24 = 0x12,
    Psm8 = 0x13,
    Psm4 = 0x14,
    Psm8H = 0x1B,
    Psm4HL = 0x24,
    Psm4HH = 0x2C,
    PsmZ32 = 0x30,
    PsmZ24 = 0x31,
    PsmZ16 = 0x32,
    PsmZ16S = 0x3A,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Region {
    Ntsc = 0,
    Pal = 1,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeId {
    Auto = 0x00,
    /// NTSC-NI
    Ntsc = 0x02,
    /// PAL-NI
    Pal = 0x03,
    /// 480p
    Hd480p = 0x50,
    /// 576p (only available in BIOS >= 220)
    Hd576p = 0x53,
    /// 720p
    Hd720p = 0x52,
    /// 1080i
    Hd1080i = 0x51,
    /// VGA 640x480@60
    Vga640x60 = 0x1A,
    /// VGA 640x480@72
    Vga640x72 = 0x1B,
    /// VGA 640x480@75
    Vga640x75 = 0x1C,
    /// VGA 640x480@85
    Vga640x85 = 0x1D,
    /// VGA 800x600@56
    Vga800x56 = 0x2A,
    /// VGA 800x600@60
    Vga800x60 = 0x2B,
    /// VGA 800x600@72
    Vga800x72 = 0x2C,
    /// VGA 800x600@75
    Vga800x75 = 0x2D,
    /// VGA 800x600@85
    Vga800x85 = 0x2E,
    /// VGA 1024x768@60
    Vga1024x60 = 0x3B,
    /// VGA 1024x768@70
    Vga1024x70 = 0x3C,
    /// VGA 1024x768@75
    Vga1024x75 = 0x3D,
    /// VGA 1024x768@85
    Vga1024x85 = 0x3E,
    /// VGA 1280x1024@60
    Vga1280x60 = 0x4A,
    /// VGA 1280x1024@75
    Vga1280x75 = 0x4B,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeInfo {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    /// Non-interlaced height.
    pub height: u32,
}

impl ModeId {
    pub fn get_info(&self) -> ModeInfo {
        match *self {
            Self::Auto => ModeInfo {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            Self::Ntsc => ModeInfo {
                x: 652,
                y: 26,
                width: 2560,
                height: 224,
            },
            Self::Pal => ModeInfo {
                x: 680,
                y: 37,
                width: 2560,
                height: 256,
            },
            Self::Vga640x60 => ModeInfo {
                x: 280,
                y: 18,
                width: 1280,
                height: 480,
            },
            _ => todo!(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameMode {
    Field = 0,
    Frame = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlphaBlend {
    ReadCircuit1 = 0,
    Alpha = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlphaOutput {
    ReadCircuit1 = 0,
    ReadCircuit2 = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendMethod {
    ReadCircuit2 = 0,
    BackgroundColour = 1,
}

static mut CURRENT_WIDTH: u32 = 0;
static mut CURRENT_HEIGHT: u32 = 0;
// static mut CURRENT_ASPECT_RATIO: f32 = 0.0;
static mut CURRENT_FLICKER_FILTER: bool = false;
static mut CURRENT_MODE_ID: ModeId = ModeId::Auto;
static mut CURRENT_INTERLACED: bool = false;
static mut CURRENT_FRAME_MODE: FrameMode = FrameMode::Field;
static mut CURRENT_X: u32 = 0;
static mut CURRENT_Y: u32 = 0;
static mut CURRENT_MAGH: u32 = 0;
static mut CURRENT_MAGV: u32 = 0;

#[expect(clippy::missing_safety_doc)]
pub unsafe fn initialise(framebuffer: &draw::Framebuffer, x: u32, y: u32, flicker_filter: bool) {
    unsafe {
        // Set a default interlaced video mode with optional flicker filter.
        set_mode(SetModeArgs {
            interlaced: true,
            mode_id: match get_region() {
                Region::Ntsc => ModeId::Ntsc,
                Region::Pal => ModeId::Pal,
            },
            frame_mode: FrameMode::Field,
            flicker_filter,
        });
        // Screen setup
        set_screen(0, 0, framebuffer.get_width(), framebuffer.get_height());
        // Set black background
        gs::background_colour::set(0, 0, 0);
        // Final setup
        set_framebuffer_filtered(SetFramebufferFilteredArgs {
            vram_framebuffer_address: framebuffer.get_address(),
            width: framebuffer.get_width(),
            psm: framebuffer.get_psm(),
            x,
            y,
        });
        enable_output();
    }
}

#[expect(clippy::missing_safety_doc)]
pub unsafe fn get_region() -> Region {
    // FIXME: Implement region detection
    Region::Ntsc
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SetModeArgs {
    pub interlaced: bool,
    pub mode_id: ModeId,
    pub frame_mode: FrameMode,
    pub flicker_filter: bool,
}

#[expect(clippy::missing_safety_doc)]
pub unsafe fn set_mode(args: SetModeArgs) {
    unsafe {
        // Reset GS
        gs::csr::set_reset();
        gs::csr::clear();
        // Unmask GS VSYNC interrupt
        syscalls::gs_set_imr(0x00007700);
        // Ensure registers are written prior to setting another mode
        asm_helpers::sync_p();
        // Fixup values to ensure correctness
        let interlaced = args.interlaced || args.mode_id == ModeId::Hd1080i;
        let flicker_filter = args.flicker_filter && interlaced;
        // Save mode, interlacing, and frame mode information
        CURRENT_MODE_ID = args.mode_id;
        CURRENT_INTERLACED = interlaced;
        CURRENT_FRAME_MODE = args.frame_mode;
        CURRENT_FLICKER_FILTER = flicker_filter;
        // Set requested mode
        syscalls::gs_set_crt(interlaced, args.mode_id, args.frame_mode);
    }
}

unsafe fn set_screen(x: u32, y: u32, width: u32, height: u32) {
    unsafe {
        assert!(CURRENT_MODE_ID != ModeId::Auto);
        CURRENT_X = x;
        CURRENT_Y = y;
        CURRENT_WIDTH = width;
        CURRENT_HEIGHT = height;
        let mode_id = CURRENT_MODE_ID;
        let mode_info = mode_id.get_info();
        // Add X adjustment to default X offset
        let dx = mode_info.x + CURRENT_X;
        // Get default Y offset
        let mut dy = mode_info.y + CURRENT_Y;
        // Get screen's width and height
        let dw = mode_info.width;
        let mut dh = mode_info.height;
        // Double Y offset for interlacing in FIELD mode
        if CURRENT_INTERLACED && CURRENT_FRAME_MODE == FrameMode::Field {
            dy = (dy - 1) * 2;
            dh = mode_info.height * 2;
        }
        // Now add Y adjustment
        dy += CURRENT_Y;
        // Determine magnification
        CURRENT_MAGH = dw / width;
        // CURRENT_MAGH = 1;
        CURRENT_MAGV = dh / height;
        // Ensure magnification values aren't negative
        if CURRENT_MAGH < 1 {
            CURRENT_MAGH = 1;
        }
        if CURRENT_MAGV < 1 {
            CURRENT_MAGV = 1;
        }
        // Set display attributes but use user defined height
        gs::display::set(
            gs::display::DisplayIndex::Display1,
            gs::display::SetArgs {
                dx: dx as u16,
                dy: dy as u16,
                magh: (CURRENT_MAGH - 1) as u8,
                magv: (CURRENT_MAGV - 1) as u8,
                dw: (dw - 1) as u16,
                dh: (height - 1) as u16,
            },
        );
        if CURRENT_FLICKER_FILTER {
            // For flicker filter, we need to add an extra line
            gs::display::set(
                gs::display::DisplayIndex::Display2,
                gs::display::SetArgs {
                    dx: dx as u16,
                    dy: dy as u16,
                    magh: (CURRENT_MAGH - 1) as u8,
                    magv: (CURRENT_MAGV - 1) as u8,
                    dw: (dw - 1) as u16,
                    dh: (height - 2) as u16,
                },
            );
        } else {
            gs::display::set(
                gs::display::DisplayIndex::Display2,
                gs::display::SetArgs {
                    dx: dx as u16,
                    dy: dy as u16,
                    magh: (CURRENT_MAGH - 1) as u8,
                    magv: (CURRENT_MAGV - 1) as u8,
                    dw: (dw - 1) as u16,
                    dh: (height - 1) as u16,
                },
            );
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SetFramebufferFilteredArgs {
    pub vram_framebuffer_address: u32,
    pub width: u32,
    pub psm: PixelStorageMethod,
    pub x: u32,
    pub y: u32,
}

unsafe fn set_framebuffer_filtered(args: SetFramebufferFilteredArgs) {
    unsafe {
        let shifted_address: u16 = (args.vram_framebuffer_address >> 11).try_into().unwrap();
        let shifted_width: u8 = (args.width >> 6).try_into().unwrap();
        gs::display_fb::set(
            gs::display_fb::DisplayIndex::Display1,
            gs::display_fb::SetArgs {
                shifted_address,
                shifted_width,
                psm: args.psm,
                dbx: args.x.try_into().unwrap(),
                dby: args.y.try_into().unwrap(),
            },
        );
        // For flicker filter, we need to offset the lines by 1 for second read circuit
        gs::display_fb::set(
            gs::display_fb::DisplayIndex::Display2,
            gs::display_fb::SetArgs {
                shifted_address,
                shifted_width,
                psm: args.psm,
                dbx: args.x.try_into().unwrap(),
                dby: (args.y + 1).try_into().unwrap(),
            },
        );
    }
}

unsafe fn enable_output() {
    unsafe {
        if CURRENT_FLICKER_FILTER {
            gs::pmode::set(
                true,
                true,
                AlphaBlend::Alpha,
                AlphaOutput::ReadCircuit1,
                BlendMethod::ReadCircuit2,
                0x70,
            );
        } else {
            gs::pmode::set(
                false,
                true,
                AlphaBlend::ReadCircuit1,
                AlphaOutput::ReadCircuit2,
                BlendMethod::ReadCircuit2,
                0x80,
            );
        }
    }
}

pub mod gs {
    use super::*;

    /// System status register
    pub mod csr {
        use super::*;

        const PTR: *mut u64 = 0x12001000 as *mut _;

        #[expect(clippy::missing_safety_doc)]
        pub unsafe fn set_reset() {
            unsafe {
                asm_helpers::write_u64(PTR, 1 << 9, 0);
            }
        }

        #[expect(clippy::missing_safety_doc)]
        pub unsafe fn clear() {
            unsafe {
                asm_helpers::write_u64(PTR, 0, 0);
            }
        }

        #[expect(clippy::missing_safety_doc)]
        pub async unsafe fn wait_for_drawing_finished_async() {
            unsafe {
                let ptr_u32 = PTR as *mut u32;
                futures::future::poll_fn(|_cx| match ptr_u32.read_volatile() & 2 == 0 {
                    false => Poll::Ready(()),
                    true => Poll::Pending,
                })
                .await;
                ptr_u32.write_volatile(ptr_u32.read_volatile() | 2);
            }
        }

        #[expect(clippy::missing_safety_doc)]
        pub async unsafe fn wait_for_vsync_async() {
            unsafe {
                // Start VSYNC interrupt.
                let ptr_u32 = PTR as *mut u32;
                ptr_u32.write_volatile(ptr_u32.read_volatile() | (ptr_u32.read_volatile() & 8));
                // Wait for interrupt.
                futures::future::poll_fn(|_cx| match ptr_u32.read_volatile() & 8 == 0 {
                    false => Poll::Ready(()),
                    true => Poll::Pending,
                })
                .await;
            }
        }

        #[expect(clippy::missing_safety_doc)]
        pub unsafe fn wait_until_drawing_finished() {
            let ptr_u32 = PTR as *mut u32;
            unsafe {
                while ptr_u32.read_volatile() & 2 == 0 {
                    asm!("nop", "nop", "nop", "nop");
                }
                ptr_u32.write_volatile(ptr_u32.read_volatile() | 2);
            }
        }

        #[expect(clippy::missing_safety_doc)]
        pub unsafe fn wait_for_vsync() {
            let ptr_u32 = PTR as *mut u32;
            unsafe {
                // Start VSYNC interrupt
                ptr_u32.write_volatile(ptr_u32.read_volatile() | (ptr_u32.read_volatile() & 8));
                // Wait for interrupt
                while ptr_u32.read_volatile() & 8 == 0 {
                    asm!("nop", "nop", "nop", "nop");
                }
            }
        }
    }

    /// Settings for Rectangular Area Read Output Circuit 1 and 2
    pub mod display_fb {
        use super::*;

        pub const PTRS: [*mut u64; 2] = [0x12000070 as *mut _, 0x12000090 as *mut _];

        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum DisplayIndex {
            Display1 = 0,
            Display2 = 1,
        }

        #[derive(Clone, Copy, Debug)]
        pub struct SetArgs {
            pub shifted_address: u16,
            pub shifted_width: u8,
            pub psm: PixelStorageMethod,
            pub dbx: u16,
            pub dby: u16,
        }

        #[expect(clippy::missing_safety_doc)]
        pub unsafe fn set(display: DisplayIndex, args: SetArgs) {
            unsafe {
                let low: u32 = (args.shifted_address as u32 & 0x1FF)
                    | ((args.shifted_width as u32 & 0x3F) << 9)
                    | ((args.psm as u32) << 15);
                let high: u32 = (args.dbx as u32 & 0x7FF) | ((args.dby as u32 & 0x7FF) << 11);
                let ptr = PTRS[display as usize];
                asm_helpers::write_u64(ptr, low, high);
            }
        }
    }

    /// Settings for Rectangular Area Read Output Circuit 1 and 2
    pub mod display {
        use super::*;

        pub const PTRS: [*mut u64; 2] = [0x12000080 as *mut _, 0x120000A0 as *mut _];

        pub use super::display_fb::DisplayIndex;

        #[derive(Clone, Copy, Debug)]
        pub struct SetArgs {
            pub dx: u16,
            pub dy: u16,
            pub magh: u8,
            pub magv: u8,
            pub dw: u16,
            pub dh: u16,
        }

        #[expect(clippy::missing_safety_doc)]
        pub unsafe fn set(display: DisplayIndex, args: SetArgs) {
            unsafe {
                let low: u32 = (args.dx as u32 & 0xFFF)
                    | ((args.dy as u32 & 0x7FF) << 12)
                    | ((args.magh as u32 & 0xF) << 23)
                    | ((args.magv as u32 & 0x3) << 27);
                let high: u32 = (args.dw as u32 & 0xFFF) | ((args.dh as u32 & 0x7FF) << 12);
                let ptr = PTRS[display as usize];
                asm_helpers::write_u64(ptr, low, high);
            }
        }
    }

    pub mod background_colour {
        use super::*;

        const PTR: *mut u64 = 0x120000E0 as *mut _;

        #[expect(clippy::missing_safety_doc)]
        pub unsafe fn set(r: u8, g: u8, b: u8) {
            unsafe {
                asm_helpers::write_u64(PTR, (r as u32) | ((g as u32) << 8) | ((b as u32) << 16), 0);
            }
        }
    }

    /// PCRTC mode setting
    pub mod pmode {
        use super::*;

        pub const PTR: *mut u64 = 0x12000000 as *mut _;

        #[expect(clippy::missing_safety_doc)]
        pub unsafe fn set(
            read_circuit_1: bool,
            read_circuit_2: bool,
            alpha_select: AlphaBlend,
            alpha_output: AlphaOutput,
            blend_method: BlendMethod,
            alpha: u8,
        ) {
            unsafe {
                asm_helpers::write_u64(
                    PTR,
                    (read_circuit_1 as u32)
                        | ((read_circuit_2 as u32) << 1)
                        | (1 << 2)
                        | ((alpha_select as u32) << 5)
                        | ((alpha_output as u32) << 6)
                        | ((blend_method as u32) << 7)
                        | ((alpha as u32) << 8),
                    0,
                );
            }
        }
    }
}

pub mod vram {
    use super::*;

    // Each word is 1 32-bit pixel
    pub const MAX_WORDS: u32 = 1048576;
    pub const PAGE_ALIGNMENT: u32 = 2048;

    static mut CURRENT_WORD_ADDRESS: u32 = 0;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct OutOfMemoryError;

    #[expect(clippy::missing_safety_doc)]
    pub unsafe fn allocate(
        width: u32,
        height: u32,
        psm: PixelStorageMethod,
        alignment: u32,
    ) -> Result<u32, OutOfMemoryError> {
        unsafe {
            // Calculate size and increment pointer
            let size = get_current_size(width, height, psm, alignment);
            // Check if we've overflowed VRAM
            if CURRENT_WORD_ADDRESS + size > MAX_WORDS {
                return Err(OutOfMemoryError);
            }
            let address = CURRENT_WORD_ADDRESS;
            CURRENT_WORD_ADDRESS += size;
            Ok(address)
        }
    }

    #[expect(clippy::missing_safety_doc)]
    pub unsafe fn get_current_size(
        width: u32,
        height: u32,
        psm: PixelStorageMethod,
        alignment: u32,
    ) -> u32 {
        use PixelStorageMethod::*;
        // First correct the buffer width to be a multiple of 64 or 128.
        // If width <= 16, then it's a palette.
        let width = if width > 16 {
            match psm {
                Psm8 | Psm4 | Psm8H | Psm4HL | Psm4HH => 0xFFFF_FF80 & (width + 127),
                _ => 0xFFFF_FFC0 & (width + 63),
            }
        } else {
            width
        };
        // Texture storage size is in pixels/word
        let size = match psm {
            Psm4 => width * (height >> 3),
            Psm8 => width * (height >> 2),
            Psm24 | Psm32 | Psm8H | Psm4HL | Psm4HH | PsmZ24 | PsmZ32 => width * height,
            Psm16 | Psm16S | PsmZ16 | PsmZ16S => width * (height >> 1),
            _ => return 0,
        };
        // Buffer size is dependent on alignment
        0u32.wrapping_sub(alignment) & (size + (alignment - 1))
    }
}

#[expect(unused)]
mod syscalls {
    use super::*;

    unsafe extern "C" {
        #[link_name = "syscall_gs_get_imr"]
        pub unsafe fn gs_get_imr() -> usize;

        #[link_name = "syscall_gs_set_imr"]
        pub unsafe fn gs_set_imr(new_imr: usize);

        #[link_name = "syscall_gs_set_crt"]
        unsafe fn raw_gs_set_crt(interlaced: i16, mode_id: i16, field: i16);
    }

    pub unsafe fn gs_set_crt(interlaced: bool, mode_id: ModeId, frame_mode: FrameMode) {
        unsafe { raw_gs_set_crt(interlaced as i16, mode_id as i16, frame_mode as i16) }
    }
}
