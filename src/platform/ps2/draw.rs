use super::gs::DepthTestMethod;
use super::display::{vram, PixelStorageMethod};

// TODO: Merge this into `display`

#[derive(Debug)]
pub struct Framebuffer {
    texture: vram::Texture,
    pub mask: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FramebufferInitArgs {
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelStorageMethod,
    pub mask: u32,
    pub placement_preference: vram::Placement,
}

impl Framebuffer {
    pub fn new(args: FramebufferInitArgs) -> Result<Self, vram::OutOfMemoryError> {
        let texture = vram::Texture::new(
            args.width,
            args.height,
            args.pixel_format,
            args.placement_preference,
        )?;
        Ok(Self {
            texture,
            mask: args.mask,
        })
    }

    pub fn get_texture<'a>(&'a self) -> &'a vram::Texture {
        &self.texture
    }

    pub fn width(&self) -> u32 {
        self.texture.width()
    }

    pub fn height(&self) -> u32 {
        self.texture.height()
    }

    pub fn pixel_format(&self) -> PixelStorageMethod {
        self.texture.pixel_format()
    }

    pub fn start_address(&self) -> u32 {
        self.texture.start_address()
    }

    pub fn buffer_size(&self) -> u32 {
        self.texture.buffer_size()
    }
}

#[derive(Debug)]
pub struct ZBuffer {
    texture: vram::Texture,
    pub depth_test_method: DepthTestMethod,
    pub mask: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZBufferInitArgs {
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelStorageMethod,
    pub depth_test_method: DepthTestMethod,
    pub mask: bool,
    pub placement_preference: vram::Placement,
}

impl ZBuffer {
    pub fn new(args: ZBufferInitArgs) -> Result<Self, vram::OutOfMemoryError> {
        let texture = vram::Texture::new(
            args.width,
            args.height,
            args.pixel_format,
            args.placement_preference,
        )?;
        Ok(Self {
            texture,
            depth_test_method: args.depth_test_method,
            mask: args.mask,
        })
    }

    pub fn get_texture<'a>(&'a self) -> &'a vram::Texture {
        &self.texture
    }

    pub fn width(&self) -> u32 {
        self.texture.width()
    }

    pub fn height(&self) -> u32 {
        self.texture.height()
    }

    pub fn pixel_format(&self) -> PixelStorageMethod {
        self.texture.pixel_format()
    }

    pub fn start_address(&self) -> u32 {
        self.texture.start_address()
    }

    pub fn buffer_size(&self) -> u32 {
        self.texture.buffer_size()
    }
}
