pub mod boat;
pub mod pig;

use anyhow::{Context, ensure};
use hypercubed_core::types::UnitAngleU8;
use hypercubed_entity_models::EntityRenderQuad;
use nalgebra::{Point3, Vector3};
use portable_std::FastHashMap;
use resources::{RegistryData, RegistryIndex, identifier};
use slab::Slab;

use crate::protocol::basic_types::EntityId;
use crate::protocol::play::{
    SpawnEntityInfo, TeleportEntity, UpdateEntityPosition, UpdateEntityPositionAndRotation,
    UpdateEntityRotation,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityHandle(pub usize);

#[derive(Debug)]
pub struct ActiveEntity {
    pub manager: RegistryIndex,
    pub handle: EntityHandle,
}

pub trait EntityTypeManager {
    fn spawn_entity(&mut self, entity_info: &SpawnEntityInfo) -> anyhow::Result<EntityHandle>;

    /// May panic if the provided handle is invalid.
    fn remove_entity(&mut self, handle: EntityHandle);

    /// May panic if the provided handle is invalid.
    fn update_entity_pos(&mut self, handle: &EntityHandle, pos_diff: Vector3<f64>);

    /// May panic if the provided handle is invalid.
    fn update_entity_rot(
        &mut self,
        handle: &EntityHandle,
        new_yaw: UnitAngleU8,
        new_pitch: UnitAngleU8,
    );

    /// May panic if the provided handle is invalid.
    fn update_entity_pos_and_rot(
        &mut self,
        handle: &EntityHandle,
        pos_diff: Vector3<f64>,
        new_yaw: UnitAngleU8,
        new_pitch: UnitAngleU8,
    );

    /// May panic if the provided handle is invalid.
    fn teleport_entity(
        &mut self,
        handle: &EntityHandle,
        new_pos: Point3<f64>,
        new_yaw: UnitAngleU8,
        new_pitch: UnitAngleU8,
    );

    /// Allows the manager to comapact its internal storage, calling the provided function to remap
    /// old to new entity handles.
    ///
    /// Panics if the remapping function is called with an invalid old entity handle.
    fn compact_if_needed(&mut self, remap: &mut (dyn FnMut(EntityHandle, EntityHandle) + '_));

    // TODO: Visibility information.
    fn render_visible(&self, out_quads: &mut Vec<EntityRenderQuad>);
}

pub struct EntityState {
    pub manager_registry: RegistryData<Box<dyn EntityTypeManager>>,
    pub entities: FastHashMap<EntityId, ActiveEntity>,
}

impl EntityState {
    pub fn new_vanilla(entity_texture_atlas: &resources::texture::Atlas) -> anyhow::Result<Self> {
        let mut manager_registry = RegistryData::new();
        register_vanilla_managers(&mut manager_registry, entity_texture_atlas)
            .context("Error while registering vanilla entity managers")?;
        Ok(Self {
            manager_registry,
            entities: FastHashMap::new(),
        })
    }

    pub fn spawn_entity(&mut self, entity_info: &SpawnEntityInfo) -> anyhow::Result<()> {
        let manager = entity_info.entity_type;
        let handle = self
            .manager_registry
            .get_mut(manager)
            .context("Unknown entity manager")?
            .spawn_entity(entity_info)?;
        ensure!(
            !self.entities.contains_key(&entity_info.id),
            "Entity with ID {} already exists",
            entity_info.id.0,
        );
        self.entities
            .insert(entity_info.id, ActiveEntity { manager, handle });
        Ok(())
    }

    pub fn remove_entity(&mut self, entity_id: EntityId) -> anyhow::Result<()> {
        let active_entity = self.entities.remove(&entity_id).context("Unknown entity")?;
        self.manager_registry[active_entity.manager].remove_entity(active_entity.handle);
        Ok(())
    }

    pub fn update_entity_pos(&mut self, new_pos_info: &UpdateEntityPosition) -> anyhow::Result<()> {
        let active_entity = self
            .entities
            .get(&new_pos_info.entity_id)
            .context("Unknown entity")?;
        self.manager_registry[active_entity.manager]
            .update_entity_pos(&active_entity.handle, new_pos_info.get_delta_vec());
        Ok(())
    }

    pub fn update_entity_rot(&mut self, new_rot_info: &UpdateEntityRotation) -> anyhow::Result<()> {
        let active_entity = self
            .entities
            .get(&new_rot_info.entity_id)
            .context("Unknown entity")?;
        self.manager_registry[active_entity.manager].update_entity_rot(
            &active_entity.handle,
            new_rot_info.new_yaw,
            new_rot_info.new_pitch,
        );
        Ok(())
    }

    pub fn update_entity_pos_and_rot(
        &mut self,
        new_pos_and_rot_info: &UpdateEntityPositionAndRotation,
    ) -> anyhow::Result<()> {
        let active_entity = self
            .entities
            .get(&new_pos_and_rot_info.entity_id)
            .context("Unknown entity")?;
        self.manager_registry[active_entity.manager].update_entity_pos_and_rot(
            &active_entity.handle,
            new_pos_and_rot_info.get_delta_vec(),
            new_pos_and_rot_info.new_yaw,
            new_pos_and_rot_info.new_pitch,
        );
        Ok(())
    }

    pub fn teleport_entity(&mut self, teleport_info: &TeleportEntity) -> anyhow::Result<()> {
        let active_entity = self
            .entities
            .get(&teleport_info.entity_id)
            .context("Unknown entity")?;
        self.manager_registry[active_entity.manager].teleport_entity(
            &active_entity.handle,
            teleport_info.get_new_pos(),
            teleport_info.new_yaw,
            teleport_info.new_pitch,
        );
        Ok(())
    }

    pub fn compact_if_needed(&mut self) {
        for manager in &mut self.manager_registry.entries {
            manager.compact_if_needed(&mut |old_handle, new_handle| {
                // FIXME: Switch to using a bidirectional map.
                let active_entity = self
                    .entities
                    .values_mut()
                    .find(|entity| entity.handle == old_handle)
                    .unwrap();
                active_entity.handle = new_handle;
            });
        }
    }

    // TODO: Visibility information.
    #[tracing::instrument(skip_all)]
    pub fn render(&self) -> Vec<EntityRenderQuad> {
        let mut out_quads: Vec<EntityRenderQuad> = Vec::new();
        for manager in &self.manager_registry.entries {
            manager.render_visible(&mut out_quads);
        }
        out_quads
    }
}

fn register_vanilla_managers(
    registry: &mut RegistryData<Box<dyn EntityTypeManager>>,
    entity_texture_atlas: &resources::texture::Atlas,
) -> anyhow::Result<()> {
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
    registry.register(
        identifier!("boat"),
        boat::BoatManager::new(entity_texture_atlas)
            .context("Error while registering boat manager")?,
    );
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
    registry.register(
        identifier!("pig"),
        pig::Manager::new(entity_texture_atlas).context("Error while registering pig manager")?,
    );
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
    Ok(())
}

trait SimpleEntity {
    fn new(pos: Point3<f64>, yaw: UnitAngleU8, head_yaw: UnitAngleU8, pitch: UnitAngleU8) -> Self;

    fn add_pos(&mut self, pos_diff: Vector3<f64>);

    fn set_pos(&mut self, new_pos: Point3<f64>);

    fn set_rot(&mut self, new_yaw: UnitAngleU8, new_pitch: UnitAngleU8);
}

trait SimpleEntityTypeManager {
    type Entity: SimpleEntity;

    fn get_entity_storage_mut(&mut self) -> &mut Slab<Self::Entity>;

    fn render_visible(&self, out_quads: &mut Vec<EntityRenderQuad>);
}

impl<T: SimpleEntityTypeManager> EntityTypeManager for T {
    fn spawn_entity(&mut self, entity_info: &SpawnEntityInfo) -> anyhow::Result<EntityHandle> {
        let entities = self.get_entity_storage_mut();
        let key = entities.insert(<Self as SimpleEntityTypeManager>::Entity::new(
            Point3::from(entity_info.coords),
            entity_info.yaw,
            entity_info.head_yaw,
            entity_info.pitch,
        ));
        Ok(EntityHandle(key))
    }

    fn remove_entity(&mut self, handle: EntityHandle) {
        self.get_entity_storage_mut().remove(handle.0);
    }

    fn update_entity_pos(&mut self, handle: &EntityHandle, pos_diff: Vector3<f64>) {
        self.get_entity_storage_mut()[handle.0].add_pos(pos_diff);
    }

    fn update_entity_rot(
        &mut self,
        handle: &EntityHandle,
        new_yaw: UnitAngleU8,
        new_pitch: UnitAngleU8,
    ) {
        self.get_entity_storage_mut()[handle.0].set_rot(new_yaw, new_pitch);
    }

    fn update_entity_pos_and_rot(
        &mut self,
        handle: &EntityHandle,
        pos_diff: Vector3<f64>,
        new_yaw: UnitAngleU8,
        new_pitch: UnitAngleU8,
    ) {
        let entities = self.get_entity_storage_mut();
        let entity = &mut entities[handle.0];
        entity.add_pos(pos_diff);
        entity.set_rot(new_yaw, new_pitch);
    }

    fn teleport_entity(
        &mut self,
        handle: &EntityHandle,
        new_pos: Point3<f64>,
        new_yaw: UnitAngleU8,
        new_pitch: UnitAngleU8,
    ) {
        let entities = self.get_entity_storage_mut();
        let entity = &mut entities[handle.0];
        entity.set_pos(new_pos);
        entity.set_rot(new_yaw, new_pitch);
    }

    fn compact_if_needed(&mut self, remap: &mut (dyn FnMut(EntityHandle, EntityHandle) + '_)) {
        let entities = self.get_entity_storage_mut();
        let compaction_needed = entities.len() as f32 * 1.25 <= entities.capacity() as f32;
        if !compaction_needed {
            return;
        }
        entities.compact(|_value, old_key, new_key| {
            remap(EntityHandle(old_key), EntityHandle(new_key));
            true
        });
    }

    fn render_visible(&self, out_quads: &mut Vec<EntityRenderQuad>) {
        SimpleEntityTypeManager::render_visible(self, out_quads)
    }
}

#[derive(Clone, Copy, Debug)]
struct DummyManager;

impl EntityTypeManager for DummyManager {
    fn spawn_entity(&mut self, _entity_info: &SpawnEntityInfo) -> anyhow::Result<EntityHandle> {
        Ok(EntityHandle(0))
    }

    fn remove_entity(&mut self, _handle: EntityHandle) {}

    fn update_entity_pos(&mut self, _handle: &EntityHandle, _pos_diff: Vector3<f64>) {}

    fn update_entity_rot(
        &mut self,
        _handle: &EntityHandle,
        _new_yaw_deg: UnitAngleU8,
        _new_pitch_deg: UnitAngleU8,
    ) {
    }

    fn update_entity_pos_and_rot(
        &mut self,
        _handle: &EntityHandle,
        _pos_diff: Vector3<f64>,
        _new_yaw_deg: UnitAngleU8,
        _new_pitch_deg: UnitAngleU8,
    ) {
    }

    fn teleport_entity(
        &mut self,
        _handle: &EntityHandle,
        _new_pos: Point3<f64>,
        _new_yaw_deg: UnitAngleU8,
        _new_pitch_deg: UnitAngleU8,
    ) {
    }

    fn compact_if_needed(&mut self, _remap: &mut (dyn FnMut(EntityHandle, EntityHandle) + '_)) {}

    fn render_visible(&self, _out_quads: &mut Vec<EntityRenderQuad>) {}
}
