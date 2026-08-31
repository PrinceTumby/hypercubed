pub mod boat;

use anyhow::{Context, ensure};
use portable_std::FastHashMap;
use resources::{RegistryData, RegistryIndex, identifier};

use crate::protocol::basic_types::EntityId;
use crate::protocol::play::SpawnEntityInfo;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityHandle(usize);

#[derive(Debug)]
pub struct ActiveEntity {
    pub manager: RegistryIndex,
    pub handle: EntityHandle,
}

pub trait EntityTypeManager {
    fn spawn_entity(&mut self, entity_info: &SpawnEntityInfo) -> anyhow::Result<EntityHandle>;
}

pub struct EntityState {
    pub manager_registry: RegistryData<Box<dyn EntityTypeManager>>,
    pub entities: FastHashMap<EntityId, ActiveEntity>,
}

impl EntityState {
    pub fn new_vanilla() -> Self {
        let mut manager_registry = RegistryData::new();
        register_vanilla_managers(&mut manager_registry);
        Self {
            manager_registry,
            entities: FastHashMap::new(),
        }
    }

    pub fn spawn_entity(&mut self, entity_info: &SpawnEntityInfo) -> anyhow::Result<()> {
        let manager = entity_info.entity_type;
        let handle = self
            .manager_registry
            .get_mut(manager)
            .context("Unknown entity manager")?
            .spawn_entity(entity_info)?;
        ensure!(!self.entities.contains_key(&entity_info.id));
        self.entities
            .insert(entity_info.id, ActiveEntity { manager, handle });
        Ok(())
    }
}

fn register_vanilla_managers(registry: &mut RegistryData<Box<dyn EntityTypeManager>>) {
    registry.register(identifier!("allay"), Box::new(DummyManager));
    registry.register(identifier!("area_effect_cloud"), Box::new(DummyManager));
    registry.register(identifier!("armadillo"), Box::new(DummyManager));
    registry.register(identifier!("armor_stand"), Box::new(DummyManager));
    registry.register(identifier!("arrow"), Box::new(DummyManager));
    registry.register(identifier!("axolotl"), Box::new(DummyManager));
    registry.register(identifier!("bat"), Box::new(DummyManager));
    registry.register(identifier!("bee"), Box::new(DummyManager));
    registry.register(identifier!("blaze"), Box::new(DummyManager));
    registry.register(identifier!("block_display"), Box::new(DummyManager));
    registry.register(identifier!("boat"), Box::new(boat::BoatManager::new()));
    registry.register(identifier!("bogged"), Box::new(DummyManager));
    registry.register(identifier!("breeze"), Box::new(DummyManager));
    registry.register(identifier!("breeze_wind_charge"), Box::new(DummyManager));
    registry.register(identifier!("camel"), Box::new(DummyManager));
    registry.register(identifier!("cat"), Box::new(DummyManager));
    registry.register(identifier!("cave_spider"), Box::new(DummyManager));
    registry.register(identifier!("chest_boat"), Box::new(DummyManager));
    registry.register(identifier!("chest_minecart"), Box::new(DummyManager));
    registry.register(identifier!("chicken"), Box::new(DummyManager));
    registry.register(identifier!("cod"), Box::new(DummyManager));
    registry.register(
        identifier!("command_block_minecart"),
        Box::new(DummyManager),
    );
    registry.register(identifier!("cow"), Box::new(DummyManager));
    registry.register(identifier!("creeper"), Box::new(DummyManager));
    registry.register(identifier!("dolphin"), Box::new(DummyManager));
    registry.register(identifier!("donkey"), Box::new(DummyManager));
    registry.register(identifier!("dragon_fireball"), Box::new(DummyManager));
    registry.register(identifier!("drowned"), Box::new(DummyManager));
    registry.register(identifier!("egg"), Box::new(DummyManager));
    registry.register(identifier!("elder_guardian"), Box::new(DummyManager));
    registry.register(identifier!("end_crystal"), Box::new(DummyManager));
    registry.register(identifier!("ender_dragon"), Box::new(DummyManager));
    registry.register(identifier!("ender_pearl"), Box::new(DummyManager));
    registry.register(identifier!("enderman"), Box::new(DummyManager));
    registry.register(identifier!("endermite"), Box::new(DummyManager));
    registry.register(identifier!("evoker"), Box::new(DummyManager));
    registry.register(identifier!("evoker_fangs"), Box::new(DummyManager));
    registry.register(identifier!("experience_bottle"), Box::new(DummyManager));
    registry.register(identifier!("experience_orb"), Box::new(DummyManager));
    registry.register(identifier!("eye_of_ender"), Box::new(DummyManager));
    registry.register(identifier!("falling_block"), Box::new(DummyManager));
    registry.register(identifier!("fireball"), Box::new(DummyManager));
    registry.register(identifier!("firework_rocket"), Box::new(DummyManager));
    registry.register(identifier!("fox"), Box::new(DummyManager));
    registry.register(identifier!("frog"), Box::new(DummyManager));
    registry.register(identifier!("furnace_minecart"), Box::new(DummyManager));
    registry.register(identifier!("ghast"), Box::new(DummyManager));
    registry.register(identifier!("giant"), Box::new(DummyManager));
    registry.register(identifier!("glow_item_frame"), Box::new(DummyManager));
    registry.register(identifier!("glow_squid"), Box::new(DummyManager));
    registry.register(identifier!("goat"), Box::new(DummyManager));
    registry.register(identifier!("guardian"), Box::new(DummyManager));
    registry.register(identifier!("hoglin"), Box::new(DummyManager));
    registry.register(identifier!("hopper_minecart"), Box::new(DummyManager));
    registry.register(identifier!("horse"), Box::new(DummyManager));
    registry.register(identifier!("husk"), Box::new(DummyManager));
    registry.register(identifier!("illusioner"), Box::new(DummyManager));
    registry.register(identifier!("interaction"), Box::new(DummyManager));
    registry.register(identifier!("iron_golem"), Box::new(DummyManager));
    registry.register(identifier!("item"), Box::new(DummyManager));
    registry.register(identifier!("item_display"), Box::new(DummyManager));
    registry.register(identifier!("item_frame"), Box::new(DummyManager));
    registry.register(identifier!("leash_knot"), Box::new(DummyManager));
    registry.register(identifier!("lightning_bolt"), Box::new(DummyManager));
    registry.register(identifier!("llama"), Box::new(DummyManager));
    registry.register(identifier!("llama_spit"), Box::new(DummyManager));
    registry.register(identifier!("magma_cube"), Box::new(DummyManager));
    registry.register(identifier!("marker"), Box::new(DummyManager));
    registry.register(identifier!("minecart"), Box::new(DummyManager));
    registry.register(identifier!("mooshroom"), Box::new(DummyManager));
    registry.register(identifier!("mule"), Box::new(DummyManager));
    registry.register(identifier!("ocelot"), Box::new(DummyManager));
    registry.register(identifier!("ominous_item_spawner"), Box::new(DummyManager));
    registry.register(identifier!("painting"), Box::new(DummyManager));
    registry.register(identifier!("panda"), Box::new(DummyManager));
    registry.register(identifier!("parrot"), Box::new(DummyManager));
    registry.register(identifier!("phantom"), Box::new(DummyManager));
    registry.register(identifier!("pig"), Box::new(DummyManager));
    registry.register(identifier!("piglin"), Box::new(DummyManager));
    registry.register(identifier!("piglin_brute"), Box::new(DummyManager));
    registry.register(identifier!("pillager"), Box::new(DummyManager));
    registry.register(identifier!("polar_bear"), Box::new(DummyManager));
    registry.register(identifier!("potion"), Box::new(DummyManager));
    registry.register(identifier!("pufferfish"), Box::new(DummyManager));
    registry.register(identifier!("rabbit"), Box::new(DummyManager));
    registry.register(identifier!("ravager"), Box::new(DummyManager));
    registry.register(identifier!("salmon"), Box::new(DummyManager));
    registry.register(identifier!("sheep"), Box::new(DummyManager));
    registry.register(identifier!("shulker"), Box::new(DummyManager));
    registry.register(identifier!("shulker_bullet"), Box::new(DummyManager));
    registry.register(identifier!("silverfish"), Box::new(DummyManager));
    registry.register(identifier!("skeleton"), Box::new(DummyManager));
    registry.register(identifier!("skeleton_horse"), Box::new(DummyManager));
    registry.register(identifier!("slime"), Box::new(DummyManager));
    registry.register(identifier!("small_fireball"), Box::new(DummyManager));
    registry.register(identifier!("sniffer"), Box::new(DummyManager));
    registry.register(identifier!("snow_golem"), Box::new(DummyManager));
    registry.register(identifier!("snowball"), Box::new(DummyManager));
    registry.register(identifier!("spawner_minecart"), Box::new(DummyManager));
    registry.register(identifier!("spectral_arrow"), Box::new(DummyManager));
    registry.register(identifier!("spider"), Box::new(DummyManager));
    registry.register(identifier!("squid"), Box::new(DummyManager));
    registry.register(identifier!("stray"), Box::new(DummyManager));
    registry.register(identifier!("strider"), Box::new(DummyManager));
    registry.register(identifier!("tadpole"), Box::new(DummyManager));
    registry.register(identifier!("text_display"), Box::new(DummyManager));
    registry.register(identifier!("tnt"), Box::new(DummyManager));
    registry.register(identifier!("tnt_minecart"), Box::new(DummyManager));
    registry.register(identifier!("trader_llama"), Box::new(DummyManager));
    registry.register(identifier!("trident"), Box::new(DummyManager));
    registry.register(identifier!("tropical_fish"), Box::new(DummyManager));
    registry.register(identifier!("turtle"), Box::new(DummyManager));
    registry.register(identifier!("vex"), Box::new(DummyManager));
    registry.register(identifier!("villager"), Box::new(DummyManager));
    registry.register(identifier!("vindicator"), Box::new(DummyManager));
    registry.register(identifier!("wandering_trader"), Box::new(DummyManager));
    registry.register(identifier!("warden"), Box::new(DummyManager));
    registry.register(identifier!("wind_charge"), Box::new(DummyManager));
    registry.register(identifier!("witch"), Box::new(DummyManager));
    registry.register(identifier!("wither"), Box::new(DummyManager));
    registry.register(identifier!("wither_skeleton"), Box::new(DummyManager));
    registry.register(identifier!("wither_skull"), Box::new(DummyManager));
    registry.register(identifier!("wolf"), Box::new(DummyManager));
    registry.register(identifier!("zoglin"), Box::new(DummyManager));
    registry.register(identifier!("zombie"), Box::new(DummyManager));
    registry.register(identifier!("zombie_horse"), Box::new(DummyManager));
    registry.register(identifier!("zombie_villager"), Box::new(DummyManager));
    registry.register(identifier!("zombified_piglin"), Box::new(DummyManager));
    registry.register(identifier!("player"), Box::new(DummyManager));
    registry.register(identifier!("fishing_bobber"), Box::new(DummyManager));
}

#[derive(Clone, Copy, Debug)]
struct DummyManager;

impl EntityTypeManager for DummyManager {
    fn spawn_entity(&mut self, _entity_info: &SpawnEntityInfo) -> anyhow::Result<EntityHandle> {
        Ok(EntityHandle(0))
    }
}
