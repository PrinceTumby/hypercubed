pub mod boat;

use resources::Identifier;
use crate::protocol::play::SpawnEntityInfo;
use nalgebra::Point3;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EntityRenderModelMesh {
    pub texture: Identifier,
    pub light_levels: [u8; 2],
    pub start_quad: u32,
    pub num_quads: u32,
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub struct EntityRenderModelQuad(pub [EntityRenderModelVertex; 4]);

#[derive(Clone, Copy, Debug)]
pub struct EntityRenderModelVertex {
    pub pos: [f32; 3],
    pub uvs: [f32; 2],
}

pub trait RenderEntity {}

pub type RenderEntitySpawnFunc = fn(info: SpawnEntityInfo) -> Box<dyn RenderEntity>;

static SPAWN_FUNC_LISTS: &[&[(&str, RenderEntitySpawnFunc)]] = &[boat::ENTITY_SPAWN_FUNCS];
