use anyhow::Context;
use hypercubed_core::types::UnitAngleU8;
use hypercubed_entity_models::{self as models, EntityRenderQuad};
use nalgebra::{Point3, Vector3};
use slab::Slab;

#[derive(Debug)]
pub struct PigEntity {
    pub pos: Point3<f64>,
    pub yaw: UnitAngleU8,
    pub head_yaw: UnitAngleU8,
    pub pitch: UnitAngleU8,
}

impl super::SimpleEntity for PigEntity {
    #[inline]
    fn new(pos: Point3<f64>, yaw: UnitAngleU8, head_yaw: UnitAngleU8, pitch: UnitAngleU8) -> Self {
        Self {
            pos,
            yaw,
            head_yaw,
            pitch,
        }
    }

    #[inline]
    fn add_pos(&mut self, pos_diff: Vector3<f64>) {
        self.pos += pos_diff;
    }

    #[inline]
    fn set_pos(&mut self, new_pos: Point3<f64>) {
        self.pos = new_pos;
    }

    #[inline]
    fn set_rot(&mut self, new_yaw: UnitAngleU8, new_pitch: UnitAngleU8) {
        self.yaw = new_yaw;
        self.head_yaw = new_yaw;
        self.pitch = new_pitch;
    }
}

pub struct Manager {
    pub entities: Slab<PigEntity>,
    pub uv_storage: models::pig::UvStorage,
}

impl super::SimpleEntityTypeManager for Manager {
    type Entity = PigEntity;

    #[inline]
    fn get_entity_storage_mut(&mut self) -> &mut Slab<Self::Entity> {
        &mut self.entities
    }

    #[inline]
    fn render_visible(&self, out_quads: &mut Vec<EntityRenderQuad>) {
        for (_key, entity) in &self.entities {
            models::pig::render(
                out_quads,
                &self.uv_storage,
                entity.pos.cast::<f32>(),
                entity.yaw.as_radians_f32(),
                entity.head_yaw.as_radians_f32(),
                entity.pitch.as_radians_f32(),
            );
        }
    }
}

impl Manager {
    pub fn new(entity_texture_atlas: &resources::texture::Atlas) -> anyhow::Result<Box<Self>> {
        Ok(Box::new(Self {
            entities: Slab::new(),
            uv_storage: models::pig::UvStorage::load_from(entity_texture_atlas)
                .context("Error while loading UVs")?,
        }))
    }
}
