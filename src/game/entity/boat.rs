use slab::Slab;
use nalgebra::Vector3;

use super::EntityTypeManager;

#[derive(Debug)]
pub struct BoatEntity {
    pub pos: Vector3<f32>,
    /// Yaw direction of the boat, in degrees.
    // TODO: Make a `DegreesF32` type.
    pub yaw: f32,
}

pub struct BoatManager {
    pub boats: Slab<BoatEntity>,
}

impl Default for BoatManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BoatManager {
    pub const fn new() -> Self {
        Self {
            boats: Slab::new(),
        }
    }
}

impl EntityTypeManager for BoatManager {
    fn spawn(&mut self, entity_info: crate::protocol::play::SpawnEntityInfo) -> anyhow::Result<()> {
        // TODO:
        Ok(())
    }
}
