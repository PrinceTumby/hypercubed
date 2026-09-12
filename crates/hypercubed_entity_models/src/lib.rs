#[rustfmt::skip]
pub mod oak_boat;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct EntityRenderQuad(pub [EntityRenderVertex; 4]);

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct EntityRenderVertex {
    pub pos: [f32; 3],
    pub uv: [u16; 2],
}
