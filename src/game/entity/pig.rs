use anyhow::Context;
use hypercubed_core::types::UnitAngleF32;
use hypercubed_entity_models::{self as models, EntityRenderQuad};
use nalgebra::{Point3, Vector3};
use slab::Slab;

#[derive(Debug)]
pub struct PigEntity {
    pub pos: Point3<f64>,
    // TODO: Make a `DegreesF32` type.
    pub yaw: f32,
    pub head_yaw: f32,
    pub pitch: f32,
}

impl super::SimpleEntity for PigEntity {
    #[inline]
    fn new(pos: Point3<f64>, yaw: f32, head_yaw: f32, pitch: f32) -> Self {
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
    fn set_rot(&mut self, new_yaw: f32, new_pitch: f32) {
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
                entity.yaw,
                entity.head_yaw,
                entity.pitch,
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
