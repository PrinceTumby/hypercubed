pub use crate::protocol::v763::{
    crafting, entity_metadata, BlockUpdate, ChunkDataAndUpdateLight, DamageEvent, Difficulty,
    GameEvent, InitializeWorldBorder, LoginPlay, PluginMessage, ServerData, SetBlockEntityData,
    SetCenterChunk, SetContainerContent, SetDefaultSpawnPosition, SetEntityMetadata,
    SetEntityVelocity, SetEquipment, SetExperience, SetHeadRotation, SetHealth,
    SpawnNonLivingEntity, SynchronizePlayerPosition, TagGroup, TeleportEntity, UpdateAdvancements,
    UpdateEntityAttributes, UpdateEntityPosition, UpdateEntityPositionAndRotation,
    UpdateEntityRotation, UpdatePlayerInfo, UpdateRecipeBook, UpdateSectionBlocks, UpdateTime,
};

use super::super::super::prelude::*;
use nom::branch::alt;
use nom::bytes::complete::take;
use nom::combinator::{cond, value, verify};
use nom::multi::{length_count, many_till};
use nom::sequence::{pair, tuple};
use nom::Parser;
use nom_supreme::tag::complete::tag;
use nom_supreme::ParserExt;
use quartz_nbt::NbtCompound;
use uuid::Uuid;

#[derive(Clone, Debug, PacketRead)]
pub enum Clientbound {
    ErrorDisconnect {
        reason: String,
    },
    BundleDelimiter,
    SpawnNonLivingEntity(SpawnNonLivingEntity),
    SetBlockEntityData(SetBlockEntityData),
    BlockUpdate(BlockUpdate),
    ChangeDifficulty {
        difficulty: Difficulty,
        is_locked: bool,
    },
    DeclareCommands(Vec<u8>),
    SetContainerContent(SetContainerContent),
    PluginMessage(PluginMessage),
    DamageEvent(DamageEvent),
    EntityEvent {
        id: i32,
        status: u8,
    },
    GameEvent(GameEvent),
    InitializeWorldBorder(InitializeWorldBorder),
    ChunkDataAndUpdateLight(ChunkDataAndUpdateLight),
    LoginPlay(LoginPlay),
    UpdateEntityPosition(UpdateEntityPosition),
    UpdateEntityPositionAndRotation(UpdateEntityPositionAndRotation),
    UpdateEntityRotation(UpdateEntityRotation),
    PlayerAbilities {
        flags: u8,
        fly_speed: f32,
        fov_modifier: f32,
    },
    UpdatePlayerInfo(UpdatePlayerInfo),
    SynchronizePlayerPosition(SynchronizePlayerPosition),
    UpdateRecipeBook(UpdateRecipeBook),
    RemoveEntities(Vec<VarInt>),
    SetHeadRotation(SetHeadRotation),
    UpdateSectionBlocks(UpdateSectionBlocks),
    ServerData(ServerData),
    SetHeldItem {
        slot: u8,
    },
    SetCenterChunk(SetCenterChunk),
    SetDefaultSpawnPosition(SetDefaultSpawnPosition),
    SetEntityMetadata(SetEntityMetadata),
    SetEntityVelocity(SetEntityVelocity),
    SetEquipment(SetEquipment),
    SetExperience(SetExperience),
    SetHealth(SetHealth),
    UpdateTime(UpdateTime),
    TeleportEntity(TeleportEntity),
    UpdateAdvancements(UpdateAdvancements),
    UpdateEntityAttributes(UpdateEntityAttributes),
    FeatureFlags(Vec<Identifier>),
    UpdateRecipes(Vec<crafting::Recipe>),
    UpdateTags(Vec<TagGroup>),
}

impl Deserialize for Clientbound {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        alt((
            var_int_tagged_parser!(
                0x1B => String::deserialize.map(|reason| Self::ErrorDisconnect { reason }).context("ErrorDisconnect"),
                0x00 => value(Self::BundleDelimiter, tag(&[][..])).context("Bundle Delimiter"),
                0x01 => SpawnNonLivingEntity::deserialize.map(Self::SpawnNonLivingEntity),
                0x08 => SetBlockEntityData::deserialize.map(Self::SetBlockEntityData),
                0x0A => BlockUpdate::deserialize.map(Self::BlockUpdate),
                0x0C => <(Difficulty, bool)>::deserialize.map(
                    |(difficulty, is_locked)| Self::ChangeDifficulty { difficulty, is_locked }
                ),
                0x10 => |input: &[u8]| Ok(([].as_slice(), Self::DeclareCommands(input.to_vec()))),
                0x12 => SetContainerContent::deserialize.map(Self::SetContainerContent),
                0x17 => PluginMessage::deserialize.map(Self::PluginMessage),
                0x18 => DamageEvent::deserialize.map(Self::DamageEvent),
                0x1C =>
                    <(i32, u8)>::deserialize.map(|(id, status)| Self::EntityEvent { id, status }),
                0x1F => GameEvent::deserialize.map(Self::GameEvent),
                0x22 => InitializeWorldBorder::deserialize.map(Self::InitializeWorldBorder),
                0x24 => ChunkDataAndUpdateLight::deserialize.map(Self::ChunkDataAndUpdateLight),
                0x28 => LoginPlay::deserialize.map(Self::LoginPlay),
                0x2B => UpdateEntityPosition::deserialize.map(Self::UpdateEntityPosition),
                0x2C => UpdateEntityPositionAndRotation::deserialize.map(Self::UpdateEntityPositionAndRotation),
                0x2D => UpdateEntityRotation::deserialize.map(Self::UpdateEntityRotation),
                0x34 => <(u8, f32, f32)>::deserialize.map(
                    |(flags, fly_speed, fov_modifier)| Self::PlayerAbilities {
                        flags,
                        fly_speed,
                        fov_modifier,
                    }
                ),
                0x3A => UpdatePlayerInfo::deserialize.map(Self::UpdatePlayerInfo),
                0x3C =>
                    SynchronizePlayerPosition::deserialize.map(Self::SynchronizePlayerPosition),
            ),
            var_int_tagged_parser!(
                0x3D => UpdateRecipeBook::deserialize.map(Self::UpdateRecipeBook),
                0x3E => <Vec<VarInt>>::deserialize.map(Self::RemoveEntities),
                0x42 => SetHeadRotation::deserialize.map(Self::SetHeadRotation),
                0x43 => UpdateSectionBlocks::deserialize.map(Self::UpdateSectionBlocks),
                0x45 => ServerData::deserialize.map(Self::ServerData),
                0x4D => verify(u8::deserialize, |&slot| slot <= 8).map(|slot| Self::SetHeldItem {
                    slot
                }),
                0x4E => SetCenterChunk::deserialize.map(Self::SetCenterChunk),
                0x50 => SetDefaultSpawnPosition::deserialize.map(Self::SetDefaultSpawnPosition),
                0x52 => SetEntityMetadata::deserialize.map(Self::SetEntityMetadata),
                0x54 => SetEntityVelocity::deserialize.map(Self::SetEntityVelocity),
                0x55 => SetEquipment::deserialize.map(Self::SetEquipment),
                0x56 => SetExperience::deserialize.map(Self::SetExperience),
                0x57 => SetHealth::deserialize.map(Self::SetHealth),
                0x5E => UpdateTime::deserialize.map(Self::UpdateTime),
                // 0x62 => PlaySoundEffect::deserialize.map(Self::PlaySoundEffect),
                0x68 => TeleportEntity::deserialize.map(Self::TeleportEntity),
                0x69 => UpdateAdvancements::deserialize.map(Self::UpdateAdvancements),
                0x6A => UpdateEntityAttributes::deserialize.map(Self::UpdateEntityAttributes),
                0x6B => <Vec<Identifier>>::deserialize.map(Self::FeatureFlags),
                0x6D => <Vec<crafting::Recipe>>::deserialize.map(Self::UpdateRecipes),
                0x6E => <Vec<TagGroup>>::deserialize.map(Self::UpdateTags),
            ),
        )).context("Clientbound").parse(input)
    }
}
