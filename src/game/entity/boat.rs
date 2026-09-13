use anyhow::Context;
use hypercubed_core::types::UnitAngleU8;
use hypercubed_entity_models::{self as models, EntityRenderQuad};
use nalgebra::{Point3, Vector3};
use slab::Slab;

use super::{EntityHandle, EntityTypeManager};
use crate::protocol::play::SpawnEntityInfo;

#[derive(Debug)]
pub struct BoatEntity {
    pub pos: Point3<f64>,
    pub yaw: UnitAngleU8,
}

pub struct BoatManager {
    pub boats: Slab<BoatEntity>,
    pub oak_uv_storage: models::oak_boat::UvStorage,
}

impl BoatManager {
    pub fn new(entity_texture_atlas: &resources::texture::Atlas) -> anyhow::Result<Box<Self>> {
        Ok(Box::new(Self {
            boats: Slab::new(),
            oak_uv_storage: models::oak_boat::UvStorage::load_from(entity_texture_atlas)
                .context("Error while loading oak boat UVs")?,
        }))
    }
}

impl EntityTypeManager for BoatManager {
    fn spawn_entity(&mut self, entity_info: &SpawnEntityInfo) -> anyhow::Result<EntityHandle> {
        let key = self.boats.insert(BoatEntity {
            pos: Point3::from(entity_info.coords),
            yaw: entity_info.yaw,
        });
        Ok(EntityHandle(key))
    }

    fn remove_entity(&mut self, handle: EntityHandle) {
        self.boats.remove(handle.0);
    }

    fn update_entity_pos(&mut self, handle: &EntityHandle, pos_diff: Vector3<f64>) {
        self.boats[handle.0].pos += pos_diff;
    }

    fn update_entity_rot(
        &mut self,
        handle: &EntityHandle,
        new_yaw: UnitAngleU8,
        _new_pitch: UnitAngleU8,
    ) {
        self.boats[handle.0].yaw = new_yaw;
    }

    fn update_entity_pos_and_rot(
        &mut self,
        handle: &EntityHandle,
        pos_diff: Vector3<f64>,
        new_yaw: UnitAngleU8,
        _new_pitch: UnitAngleU8,
    ) {
        let boat = &mut self.boats[handle.0];
        boat.pos += pos_diff;
        boat.yaw = new_yaw;
    }

    fn teleport_entity(
        &mut self,
        handle: &EntityHandle,
        new_pos: Point3<f64>,
        new_yaw: UnitAngleU8,
        _new_pitch: UnitAngleU8,
    ) {
        let boat = &mut self.boats[handle.0];
        boat.pos = new_pos;
        boat.yaw = new_yaw;
    }

    fn compact_if_needed(&mut self, remap: &mut (dyn FnMut(EntityHandle, EntityHandle) + '_)) {
        let compaction_needed = self.boats.len() as f32 * 1.25 <= self.boats.capacity() as f32;
        if !compaction_needed {
            return;
        }
        self.boats.compact(|_value, old_key, new_key| {
            remap(EntityHandle(old_key), EntityHandle(new_key));
            true
        });
    }

    fn render_visible(&self, out_quads: &mut Vec<EntityRenderQuad>) {
        for (_boat_key, boat) in &self.boats {
            models::oak_boat::render(
                out_quads,
                &self.oak_uv_storage,
                boat.pos.cast::<f32>(),
                boat.yaw.as_radians_f32(),
                0.0,
                0.0,
            );
        }
    }
}
