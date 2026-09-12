use anyhow::Context;
use hypercubed_entity_models::{self as models, EntityRenderQuad};
use nalgebra::{Point3, Vector3};
use slab::Slab;

use super::{EntityHandle, EntityTypeManager};
use crate::protocol::play::SpawnEntityInfo;

#[derive(Debug)]
pub struct PigEntity {
    pub pos: Point3<f64>,
    /// Yaw direction of the pig, in degrees.
    // TODO: Make a `DegreesF32` type.
    pub yaw: f32,
    pub head_yaw: f32,
    pub pitch: f32,
}

pub struct Manager {
    pub entities: Slab<PigEntity>,
    pub uv_storage: models::pig::UvStorage,
}

impl Manager {
    pub fn new(entity_texture_atlas: &resources::texture::Atlas) -> anyhow::Result<Self> {
        Ok(Self {
            entities: Slab::new(),
            uv_storage: models::pig::UvStorage::load_from(entity_texture_atlas)
                .context("Error while loading UVs")?,
        })
    }
}

impl EntityTypeManager for Manager {
    fn spawn_entity(&mut self, entity_info: &SpawnEntityInfo) -> anyhow::Result<EntityHandle> {
        let key = self.entities.insert(PigEntity {
            pos: Point3::from(entity_info.coords),
            yaw: entity_info.yaw.degrees(),
            head_yaw: entity_info.head_yaw.degrees(),
            pitch: entity_info.pitch.degrees(),
        });
        Ok(EntityHandle(key))
    }

    fn remove_entity(&mut self, handle: EntityHandle) {
        self.entities.remove(handle.0);
    }

    fn update_entity_pos(&mut self, handle: &EntityHandle, pos_diff: Vector3<f64>) {
        self.entities[handle.0].pos += pos_diff;
    }

    fn update_entity_rot(&mut self, handle: &EntityHandle, new_yaw_deg: f32, new_pitch_deg: f32) {
        let boat = &mut self.entities[handle.0];
        boat.yaw = new_yaw_deg;
        boat.head_yaw = new_yaw_deg;
        boat.pitch = new_pitch_deg;
    }

    fn update_entity_pos_and_rot(
        &mut self,
        handle: &EntityHandle,
        pos_diff: Vector3<f64>,
        new_yaw_deg: f32,
        new_pitch_deg: f32,
    ) {
        let boat = &mut self.entities[handle.0];
        boat.pos += pos_diff;
        boat.yaw = new_yaw_deg;
        boat.head_yaw = new_yaw_deg;
        boat.pitch = new_pitch_deg;
    }

    fn teleport_entity(
        &mut self,
        handle: &EntityHandle,
        new_pos: Point3<f64>,
        new_yaw_deg: f32,
        new_pitch_deg: f32,
    ) {
        let boat = &mut self.entities[handle.0];
        boat.pos = new_pos;
        boat.yaw = new_yaw_deg;
        boat.head_yaw = new_yaw_deg;
        boat.pitch = new_pitch_deg;
    }

    fn compact_if_needed(&mut self, remap: &mut (dyn FnMut(EntityHandle, EntityHandle) + '_)) {
        let compaction_needed =
            self.entities.len() as f32 * 1.25 <= self.entities.capacity() as f32;
        if !compaction_needed {
            return;
        }
        self.entities.compact(|_value, old_key, new_key| {
            remap(EntityHandle(old_key), EntityHandle(new_key));
            true
        });
    }

    fn render_visible(&self, out_quads: &mut Vec<EntityRenderQuad>) {
        for (_key, entity) in &self.entities {
            models::pig::render(
                out_quads,
                &self.uv_storage,
                entity.pos.cast::<f32>(),
                entity.yaw,
                entity.head_yaw,
                entity.pitch,
            );
        }
    }
}
