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
static mut CURRENT_MODE_ID: Option<ModeId> = None;
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
        set_screen(0, 0, framebuffer.width(), framebuffer.height());
        // Set black background
        gs::background_colour::set(0, 0, 0);
        // Final setup
        set_framebuffer_filtered(SetFramebufferFilteredArgs {
            vram_framebuffer_address: framebuffer.start_address(),
            width: framebuffer.width(),
            psm: framebuffer.pixel_format(),
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
        CURRENT_MODE_ID = Some(args.mode_id);
        CURRENT_INTERLACED = interlaced;
        CURRENT_FRAME_MODE = args.frame_mode;
        CURRENT_FLICKER_FILTER = flicker_filter;
        // Set requested mode
        syscalls::gs_set_crt(interlaced, args.mode_id, args.frame_mode);
    }
}

unsafe fn set_screen(x: u32, y: u32, width: u32, height: u32) {
    unsafe {
        CURRENT_X = x;
        CURRENT_Y = y;
        CURRENT_WIDTH = width;
        CURRENT_HEIGHT = height;
        let mode_id = (*&raw const CURRENT_MODE_ID).unwrap();
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

#[allow(static_mut_refs)]
pub mod vram {
    use super::*;

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum Placement {
        /// Place the allocation at the lowest VRAM address that will fit the buffer.
        #[default]
        PreferLowest,
        /// Place the allocation at the highest VRAM address that will fit the buffer.
        PreferHighest,
    }

    impl Placement {
        pub const ANY: Self = Self::PreferLowest;
    }

    #[derive(Debug)]
    pub struct Buffer {
        start_page: u16,
        num_pages: u16,
    }

    impl Drop for Buffer {
        fn drop(&mut self) {
            unsafe {
                remove_allocation(self.as_allocation_info());
            }
        }
    }

    impl Buffer {
        pub fn new(
            num_pages: u16,
            placement_preference: Placement,
        ) -> Result<Self, OutOfMemoryError> {
            unsafe {
                match placement_preference {
                    Placement::PreferLowest => {
                        // Check gap before first allocation (may be zero sized).
                        let beginning_gap_range = if NUM_ALLOCATIONS == 0 {
                            0..VRAM_NUM_PAGES
                        } else {
                            0..ALLOCATIONS[0].start_page
                        };
                        if beginning_gap_range.len() as u16 >= num_pages {
                            let allocation = Allocation {
                                start_page: beginning_gap_range.start,
                                num_pages,
                            };
                            insert_allocation(allocation, 0);
                            return Ok(Self::from_allocation_info(&allocation));
                        }
                        // Check gaps between allocations (may be zero sized).
                        for w in ALLOCATIONS[0..NUM_ALLOCATIONS].windows(2).enumerate() {
                            let (a_i, &[a, b]) = w else { unreachable!() };
                            let b_i = a_i + 1;
                            let a_end_page = a.start_page + a.num_pages;
                            let gap_range = a_end_page..b.start_page;
                            if gap_range.len() as u16 >= num_pages {
                                let allocation = Allocation {
                                    start_page: gap_range.start,
                                    num_pages,
                                };
                                insert_allocation(allocation, b_i);
                                return Ok(Self::from_allocation_info(&allocation));
                            }
                        }
                        // Check gap after last allocation (may be zero sized).
                        let ending_gap_range = if NUM_ALLOCATIONS == 0 {
                            0..VRAM_NUM_PAGES
                        } else {
                            let last_allocation = ALLOCATIONS[NUM_ALLOCATIONS - 1];
                            (last_allocation.start_page + last_allocation.num_pages)..VRAM_NUM_PAGES
                        };
                        if ending_gap_range.len() as u16 >= num_pages {
                            let allocation = Allocation {
                                start_page: ending_gap_range.start,
                                num_pages,
                            };
                            insert_allocation(allocation, NUM_ALLOCATIONS);
                            return Ok(Self::from_allocation_info(&allocation));
                        }
                        // Return an error if we couldn't find a gap big enough.
                        return Err(OutOfMemoryError);
                    }
                    Placement::PreferHighest => {
                        // Check gap after last allocation (may be zero sized).
                        let ending_gap_range = if NUM_ALLOCATIONS == 0 {
                            0..VRAM_NUM_PAGES
                        } else {
                            let last_allocation = ALLOCATIONS[NUM_ALLOCATIONS - 1];
                            (last_allocation.start_page + last_allocation.num_pages)..VRAM_NUM_PAGES
                        };
                        if ending_gap_range.len() as u16 >= num_pages {
                            let allocation = Allocation {
                                start_page: ending_gap_range.end - num_pages,
                                num_pages,
                            };
                            insert_allocation(allocation, NUM_ALLOCATIONS);
                            return Ok(Self::from_allocation_info(&allocation));
                        }
                        // Check gaps between allocations (may be zero sized).
                        for w in ALLOCATIONS[0..NUM_ALLOCATIONS].windows(2).enumerate().rev() {
                            let (a_i, &[a, b]) = w else { unreachable!() };
                            let b_i = a_i + 1;
                            let a_end_page = a.start_page + a.num_pages;
                            let gap_range = a_end_page..b.start_page;
                            if gap_range.len() as u16 >= num_pages {
                                let allocation = Allocation {
                                    start_page: gap_range.end - num_pages,
                                    num_pages,
                                };
                                insert_allocation(allocation, b_i);
                                return Ok(Self::from_allocation_info(&allocation));
                            }
                        }
                        // Check gap before first allocation (may be zero sized).
                        let beginning_gap_range = if NUM_ALLOCATIONS == 0 {
                            0..VRAM_NUM_PAGES
                        } else {
                            0..ALLOCATIONS[0].start_page
                        };
                        if beginning_gap_range.len() as u16 >= num_pages {
                            let allocation = Allocation {
                                start_page: beginning_gap_range.end - num_pages,
                                num_pages,
                            };
                            insert_allocation(allocation, 0);
                            return Ok(Self::from_allocation_info(&allocation));
                        }
                        // Return an error if we couldn't find a gap big enough.
                        return Err(OutOfMemoryError);
                    }
                }
            }
        }

        pub fn start_address(&self) -> u32 {
            self.start_page as u32 * PAGE_SIZE
        }

        pub fn size(&self) -> u32 {
            self.num_pages as u32 * PAGE_SIZE
        }
    }

    impl Buffer {
        unsafe fn from_allocation_info(allocation: &Allocation) -> Self {
            Self {
                start_page: allocation.start_page,
                num_pages: allocation.num_pages,
            }
        }

        fn as_allocation_info(&self) -> Allocation {
            Allocation {
                start_page: self.start_page,
                num_pages: self.num_pages,
            }
        }
    }

    #[derive(Debug)]
    pub struct Texture {
        buffer: Buffer,
        width: u32,
        height: u32,
        pixel_format: PixelStorageMethod,
    }

    impl Texture {
        pub fn new(
            width: u32,
            height: u32,
            pixel_format: PixelStorageMethod,
            placement_preference: Placement,
        ) -> Result<Texture, OutOfMemoryError> {
            // Calculate size and increment pointer
            let size = Texture::calculate_size(width, height, pixel_format, PAGE_SIZE);
            let num_pages: u16 = size.div_ceil(PAGE_SIZE).try_into().unwrap();
            let buffer = Buffer::new(num_pages, placement_preference)?;
            Ok(Texture {
                buffer,
                width,
                height,
                pixel_format,
            })
        }

        pub fn calculate_size(
            width: u32,
            height: u32,
            pixel_format: PixelStorageMethod,
            alignment: u32,
        ) -> u32 {
            use PixelStorageMethod::*;
            // First correct the buffer width to be a multiple of 64 or 128.
            // If width <= 16, then it's a palette.
            let width = if width > 16 {
                match pixel_format {
                    Psm8 | Psm4 | Psm8H | Psm4HL | Psm4HH => 0xFFFF_FF80 & (width + 127),
                    _ => 0xFFFF_FFC0 & (width + 63),
                }
            } else {
                width
            };
            // Texture storage size is in pixels/word
            let num_words = match pixel_format {
                Psm4 => (width * height) / 2,
                Psm8 => width * height,
                Psm24 | Psm32 | Psm8H | Psm4HL | Psm4HH | PsmZ24 | PsmZ32 => width * height * 4,
                Psm16 | Psm16S | PsmZ16 | PsmZ16S => width * height * 2,
            };
            // Buffer size is dependent on alignment
            0u32.wrapping_sub(alignment) & (num_words + (alignment - 1))
        }

        pub fn get_buffer<'a>(&'a self) -> &'a Buffer {
            &self.buffer
        }

        pub fn start_address(&self) -> u32 {
            self.buffer.start_address()
        }

        pub fn buffer_size(&self) -> u32 {
            self.buffer.size()
        }

        pub fn width(&self) -> u32 {
            self.width
        }

        pub fn height(&self) -> u32 {
            self.height
        }

        pub fn pixel_format(&self) -> PixelStorageMethod {
            self.pixel_format
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct OutOfMemoryError;

    impl core::fmt::Display for OutOfMemoryError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            "out of video memory".fmt(f)
        }
    }

    impl core::error::Error for OutOfMemoryError {}

    pub const VRAM_NUM_PAGES: u16 = 512;
    pub const PAGE_SIZE: u32 = 8192;

    #[derive(Clone, Copy, Debug)]
    struct Allocation {
        pub start_page: u16,
        pub num_pages: u16,
    }

    const DUMMY_ALLOCATION: Allocation = Allocation {
        start_page: 0,
        num_pages: 0,
    };

    static mut ALLOCATIONS: [Allocation; 8] = [DUMMY_ALLOCATION; 8];
    static mut NUM_ALLOCATIONS: usize = 0;

    unsafe fn insert_allocation(allocation: Allocation, new_index: usize) {
        unsafe {
            assert!(NUM_ALLOCATIONS < ALLOCATIONS.len());
            assert!(new_index <= NUM_ALLOCATIONS);
            // Shift up and expand valid allocations array.
            if new_index < NUM_ALLOCATIONS {
                for i in (new_index..NUM_ALLOCATIONS).rev() {
                    ALLOCATIONS[i + 1] = ALLOCATIONS[i];
                }
            }
            ALLOCATIONS[new_index] = allocation;
            NUM_ALLOCATIONS += 1;
        }
    }

    unsafe fn remove_allocation(allocation: Allocation) {
        unsafe {
            // Find allocation.
            let allocation_i = ALLOCATIONS[0..NUM_ALLOCATIONS]
                .iter()
                .position(|a| a.start_page == allocation.start_page)
                .unwrap();
            // Shrink and shift down valid allocations array.
            NUM_ALLOCATIONS -= 1;
            if allocation_i < NUM_ALLOCATIONS {
                for i in allocation_i..NUM_ALLOCATIONS {
                    ALLOCATIONS[i] = ALLOCATIONS[i + 1];
                }
            }
        }
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
