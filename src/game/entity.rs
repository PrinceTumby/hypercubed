pub mod boat;

use resources::{identifier, Identifier, RegistryData};
use crate::protocol::play::SpawnEntityInfo;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityHandle(usize);

pub trait EntityTypeManager {
    fn spawn(&mut self, entity_info: SpawnEntityInfo) -> anyhow::Result<()>;
}

pub struct EntityState {
    pub manager_registry: RegistryData<Box<dyn EntityTypeManager>>,
}

impl EntityState {
    pub fn new_vanilla() -> Self {
        let mut manager_registry = RegistryData::new();
        register_vanilla_managers(&mut manager_registry);
        Self {
            manager_registry
        }
    }
}

fn register_vanilla_managers(registry: &mut RegistryData<Box<dyn EntityTypeManager>>) {
    registry.register(identifier!("allay"), Box::new(DummyManager));
    registry.register(identifier!("boat"), Box::new(boat::BoatManager::new()));
}

#[derive(Clone, Copy, Debug)]
struct DummyManager;

impl EntityTypeManager for DummyManager {
    fn spawn(&mut self, _entity_info: SpawnEntityInfo) -> anyhow::Result<()> {
        Ok(())
    }
}
