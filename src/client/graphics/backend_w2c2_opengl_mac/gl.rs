// Import OpenGL W2C2 bindings, generated from `gl.kdl`.
// See `build.rs` and the `minecraft_client_w2c2_binding_generator` crate for more information.
include!(concat!(env!("OUT_DIR"), "/w2c2_generated_bindings/gl.rs"));

pub use super::to_host_f32_2d_array;
pub use crate::platform::w2c2_opengl_mac::{HostF32, HostU32};

pub type RawBufferHandle = HostU32;

#[repr(transparent)]
#[derive(Debug)]
pub struct BufferHandle(pub u32);

impl BufferHandle {
    pub const fn from_raw(raw_handle: RawBufferHandle) -> Self {
        Self(raw_handle.to_num())
    }
}
