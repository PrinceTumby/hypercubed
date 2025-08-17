use super::{display, gs};

#[derive(Debug)]
pub struct Framebuffer {
    address: u32,
    width: u32,
    height: u32,
    psm: display::PixelStorageMethod,
    mask: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FramebufferInitArgs {
    pub width: u32,
    pub height: u32,
    pub psm: display::PixelStorageMethod,
    pub mask: u32,
}

impl Framebuffer {
    #[expect(clippy::missing_safety_doc)]
    pub unsafe fn new(args: FramebufferInitArgs) -> Result<Self, display::vram::OutOfMemoryError> {
        Ok(Self {
            address: unsafe {
                display::vram::allocate(
                    args.width,
                    args.height,
                    args.psm,
                    display::vram::PAGE_ALIGNMENT,
                )?
            },
            width: args.width,
            height: args.height,
            psm: args.psm,
            mask: args.mask,
        })
    }

    pub fn get_address(&self) -> u32 {
        self.address
    }

    pub fn get_width(&self) -> u32 {
        self.width
    }

    pub fn get_height(&self) -> u32 {
        self.height
    }

    pub fn get_psm(&self) -> display::PixelStorageMethod {
        self.psm
    }

    pub fn get_mask(&self) -> u32 {
        self.mask
    }
}

#[derive(Debug)]
pub struct ZBuffer {
    address: u32,
    width: u32,
    height: u32,
    zsm: display::PixelStorageMethod,
    pub depth_test_method: gs::DepthTestMethod,
    mask: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZBufferInitArgs {
    pub width: u32,
    pub height: u32,
    pub zsm: display::PixelStorageMethod,
    pub depth_test_method: gs::DepthTestMethod,
    pub mask: bool,
}

impl ZBuffer {
    #[expect(clippy::missing_safety_doc)]
    pub unsafe fn new(args: ZBufferInitArgs) -> Result<Self, display::vram::OutOfMemoryError> {
        Ok(Self {
            address: unsafe {
                display::vram::allocate(
                    args.width,
                    args.height,
                    args.zsm,
                    display::vram::PAGE_ALIGNMENT,
                )?
            },
            width: args.width,
            height: args.height,
            zsm: args.zsm,
            depth_test_method: args.depth_test_method,
            mask: args.mask,
        })
    }

    pub fn get_address(&self) -> u32 {
        self.address
    }

    pub fn get_width(&self) -> u32 {
        self.width
    }

    pub fn get_height(&self) -> u32 {
        self.height
    }

    pub fn get_zsm(&self) -> display::PixelStorageMethod {
        self.zsm
    }

    pub fn get_masked(&self) -> bool {
        self.mask
    }
}
