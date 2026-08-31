use nalgebra::Point3;
use slab::Slab;

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
}

impl BoatManager {
    pub fn new(/* entity_texture_atlas: &resources::texture::Atlas */) -> Self {
        Self { boats: Slab::new() }
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
}
