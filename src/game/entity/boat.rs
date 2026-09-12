use anyhow::Context;
use nalgebra::Point3;
use slab::Slab;
use hypercubed_entity_models::{self as models, EntityRenderQuad};

use super::{EntityHandle, EntityTypeManager};
use crate::protocol::play::SpawnEntityInfo;

#[derive(Debug)]
pub struct BoatEntity {
    pub pos: Point3<f32>,
    /// Yaw direction of the boat, in degrees.
    // TODO: Make a `DegreesF32` type.
    pub yaw: f32,
}

pub struct BoatManager {
    pub boats: Slab<BoatEntity>,
    pub oak_uv_storage: models::oak_boat::UvStorage,
}

impl BoatManager {
    pub fn new(entity_texture_atlas: &resources::texture::Atlas) -> anyhow::Result<Self> {
        Ok(Self {
            boats: Slab::new(),
            oak_uv_storage: models::oak_boat::UvStorage::load_from(entity_texture_atlas)
                .context("Error while loading oak boat UVs")?,
        })
    }
}

impl EntityTypeManager for BoatManager {
    fn spawn_entity(&mut self, entity_info: &SpawnEntityInfo) -> anyhow::Result<EntityHandle> {
        let key = self.boats.insert(BoatEntity {
            pos: Point3::from(entity_info.coords).cast::<f32>(),
            yaw: entity_info.yaw.degrees(),
        });
        Ok(EntityHandle(key))
    }

    fn render_visible(&self, out_quads: &mut Vec<EntityRenderQuad>) {
        models::oak_boat::render(out_quads);
    }
}
