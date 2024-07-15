pub mod crafting;

pub use super::configuration::{ChatMode, MainHand};
pub use crate::protocol::v763::packet::play::{
    entity_metadata, BlockUpdate, DamageEvent, Difficulty, EquipmentSlot, GameMode,
    InitializeWorldBorder, SetBlockEntityData, SetCenterChunk, SetContainerContent,
    SetDefaultSpawnPosition, SetEntityMetadata, SetEntityVelocity, SetExperience, SetHeadRotation,
    SetHealth, SpawnNonLivingEntity, SynchronizePlayerPosition, TagGroup, TeleportEntity,
    UpdateEntityAttributes, UpdateEntityPosition, UpdateEntityPositionAndRotation,
    UpdateEntityRotation, UpdatePlayerInfo, UpdateRecipeBook, UpdateSectionBlocks, UpdateTime,
};

use super::prelude::*;
use nom::branch::alt;
use nom::bytes::complete::take;
use nom::combinator::{cond, value, verify};
use nom::multi::many_till;
use nom::sequence::{pair, tuple};
use nom::Parser;
use nom_supreme::tag::complete::tag;
use nom_supreme::ParserExt;

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
    ChunkBatchEnd {
        num_chunks: usize,
    },
    ChunkBatchStart,
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
    UpdateLight(UpdateLight),
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
    PlaySoundEffect(PlaySoundEffect),
    TeleportEntity(TeleportEntity),
    SetTickingState(SetTickingState),
    StepTicks(i32),
    UpdateAdvancements(UpdateAdvancements),
    UpdateEntityAttributes(UpdateEntityAttributes),
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
                0x07 => SetBlockEntityData::deserialize.map(Self::SetBlockEntityData),
                0x09 => BlockUpdate::deserialize.map(Self::BlockUpdate),
                0x0B => <(Difficulty, bool)>::deserialize.map(
                    |(difficulty, is_locked)| Self::ChangeDifficulty { difficulty, is_locked }
                ),
                0x0C => verify(VarInt::deserialize, |chunks| chunks.0 > 0).map(|chunks| Self::ChunkBatchEnd{ num_chunks: chunks.0 as usize }),
                0x0D => <()>::deserialize.map(|()| Self::ChunkBatchStart),
                0x11 => |input: &[u8]| Ok(([].as_slice(), Self::DeclareCommands(input.to_vec()))),
                0x13 => SetContainerContent::deserialize.map(Self::SetContainerContent),
                0x18 => PluginMessage::deserialize.map(Self::PluginMessage),
                0x19 => DamageEvent::deserialize.map(Self::DamageEvent),
                0x1D =>
                    <(i32, u8)>::deserialize.map(|(id, status)| Self::EntityEvent { id, status }),
                0x20 => GameEvent::deserialize.map(Self::GameEvent),
                0x23 => InitializeWorldBorder::deserialize.map(Self::InitializeWorldBorder),
                0x25 => ChunkDataAndUpdateLight::deserialize.map(Self::ChunkDataAndUpdateLight),
                0x28 => UpdateLight::deserialize.map(Self::UpdateLight),
                0x29 => LoginPlay::deserialize.map(Self::LoginPlay),
                0x2C => UpdateEntityPosition::deserialize.map(Self::UpdateEntityPosition),
                0x2D => UpdateEntityPositionAndRotation::deserialize.map(Self::UpdateEntityPositionAndRotation),
                0x2E => UpdateEntityRotation::deserialize.map(Self::UpdateEntityRotation),
            ),
            var_int_tagged_parser!(
                0x36 => <(u8, f32, f32)>::deserialize.map(
                    |(flags, fly_speed, fov_modifier)| Self::PlayerAbilities {
                        flags,
                        fly_speed,
                        fov_modifier,
                    }
                ),
                0x3C => UpdatePlayerInfo::deserialize.map(Self::UpdatePlayerInfo),
                0x3E =>
                    SynchronizePlayerPosition::deserialize.map(Self::SynchronizePlayerPosition),
                0x3F => UpdateRecipeBook::deserialize.map(Self::UpdateRecipeBook),
                0x40 => <Vec<VarInt>>::deserialize.map(Self::RemoveEntities),
                0x46 => SetHeadRotation::deserialize.map(Self::SetHeadRotation),
                0x47 => UpdateSectionBlocks::deserialize.map(Self::UpdateSectionBlocks),
                0x49 => ServerData::deserialize.map(Self::ServerData),
                0x51 => verify(u8::deserialize, |&slot| slot <= 8).map(|slot| Self::SetHeldItem {
                    slot
                }),
                0x52 => SetCenterChunk::deserialize.map(Self::SetCenterChunk),
                0x54 => SetDefaultSpawnPosition::deserialize.map(Self::SetDefaultSpawnPosition),
                0x56 => SetEntityMetadata::deserialize.map(Self::SetEntityMetadata),
                0x58 => SetEntityVelocity::deserialize.map(Self::SetEntityVelocity),
                0x59 => SetEquipment::deserialize.map(Self::SetEquipment),
                0x5A => SetExperience::deserialize.map(Self::SetExperience),
                0x5B => SetHealth::deserialize.map(Self::SetHealth),
                0x62 => UpdateTime::deserialize.map(Self::UpdateTime),
                0x66 => PlaySoundEffect::deserialize.map(Self::PlaySoundEffect),
                0x6D => TeleportEntity::deserialize.map(Self::TeleportEntity),
                0x6E => SetTickingState::deserialize.map(Self::SetTickingState),
                0x6F => VarInt::deserialize.map(|ticks| Self::StepTicks(ticks.0)),
            ),
            var_int_tagged_parser!(
                0x70 => UpdateAdvancements::deserialize.map(Self::UpdateAdvancements),
                0x71 => UpdateEntityAttributes::deserialize.map(Self::UpdateEntityAttributes),
                0x73 => <Vec<crafting::Recipe>>::deserialize.map(Self::UpdateRecipes),
                0x74 => <Vec<TagGroup>>::deserialize.map(Self::UpdateTags),
            ),
        )).context("Clientbound").parse(input)
    }
}

// TODO Move to play/game_event.rs, give better types
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct GameEvent {
    pub event: GameEventType,
    pub value: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum GameEventType {
    NoRespawnBlockAvailable = 0,
    EndRaining = 1,
    BeginRaining = 2,
    ChangeGameMode = 3,
    WinGame = 4,
    DemoEvent = 5,
    ArrowHitPlayer = 6,
    RainLevelChange = 7,
    ThunderLevelChange = 8,
    PlayPufferfishStingSound = 9,
    PlayElderGuardianMobAppearance = 10,
    SetRespawnScreenEnabled = 11,
    LimitedCrafting = 12,
    StartWaitingForLevelChunks = 13,
}

impl Deserialize for GameEventType {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        verify(take(1usize), |variant: &[u8]| variant[0] <= 13)
            .map(|variant: &[u8]| match variant[0] {
                0 => Self::NoRespawnBlockAvailable,
                1 => Self::EndRaining,
                2 => Self::BeginRaining,
                3 => Self::ChangeGameMode,
                4 => Self::WinGame,
                5 => Self::DemoEvent,
                6 => Self::ArrowHitPlayer,
                7 => Self::RainLevelChange,
                8 => Self::ThunderLevelChange,
                9 => Self::PlayPufferfishStingSound,
                10 => Self::PlayElderGuardianMobAppearance,
                11 => Self::SetRespawnScreenEnabled,
                12 => Self::LimitedCrafting,
                13 => Self::StartWaitingForLevelChunks,
                _ => unreachable!(),
            })
            .parse(input)
    }
}

#[derive(Clone, Debug, Deserialize, serde::Serialize, serde::Deserialize)]
pub struct ChunkDataAndUpdateLight {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub heightmaps: NetworkNbtCompound,
    pub chunk_data: Vec<u8>,
    pub block_entities: Vec<BlockEntity>,
    pub sky_light_mask: BitVec,
    pub block_light_mask: BitVec,
    pub empty_sky_light_mask: BitVec,
    pub empty_block_light_mask: BitVec,
    pub sky_light_arrays: Vec<Vec<u8>>,
    pub block_light_arrays: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, Deserialize, serde::Serialize, serde::Deserialize)]
pub struct BlockEntity {
    pub packed_xz: u8,
    pub y: i16,
    pub ty: VarInt,
    pub data: OptionalNbt,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UpdateLight {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub sky_light_mask: BitVec,
    pub block_light_mask: BitVec,
    pub empty_sky_light_mask: BitVec,
    pub empty_block_light_mask: BitVec,
    pub sky_light_arrays: Vec<Vec<u8>>,
    pub block_light_arrays: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LoginPlay {
    pub entity_id: i32,
    pub is_hardcore: bool,
    pub dimension_names: Vec<String>,
    pub max_players: VarInt,
    pub view_distance: VarInt,
    pub simulation_distance: VarInt,
    pub reduced_debug_info: bool,
    pub enable_respawn_screen: bool,
    pub is_crafting_limited: bool,
    pub spawn_dimension_type: String,
    pub spawn_dimension_name: String,
    pub hashed_seed: i64,
    pub game_mode: GameMode,
    pub previous_game_mode: i8,
    pub is_world_debug: bool,
    pub is_world_flat: bool,
    pub death_position: Option<GlobalPosition>,
    pub portal_cooldown: VarInt,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ServerData {
    pub motd: Chat,
    pub icon: Option<Vec<u8>>,
    pub enforces_secure_chat: bool,
}

#[derive(Clone, Debug)]
pub struct SetEquipment {
    pub entity_id: VarInt,
    pub equipment: Vec<EquipmentPiece>,
}

impl Deserialize for SetEquipment {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        pair(
            VarInt::deserialize.context("SetEquipment.entity_id"),
            pair(
                many_till(
                    EquipmentPiece::deserialize,
                    verify(take(1usize), |input: &[u8]| input[0] & 0b1000000 == 0),
                ),
                EquipmentPiece::deserialize,
            )
            .map(|((mut vec, _), last_piece)| {
                vec.push(last_piece);
                vec
            })
            .context("SetEquipment.equipment"),
        )
        .map(|(entity_id, equipment)| Self {
            entity_id,
            equipment,
        })
        .context("SetEquipment")
        .parse(input)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct EquipmentPiece {
    pub slot: EquipmentSlot,
    pub item: crafting::PresentSlot,
}

#[derive(Clone, Debug)]
pub struct PlaySoundEffect {
    pub id: Option<i32>,
    pub name_and_fixed_range: Option<(Identifier, Option<f32>)>,
    pub category: u32,
    pub position: [i32; 3],
    pub volume: f32,
    pub pitch: f32,
    pub seed: u64,
}

impl Deserialize for PlaySoundEffect {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        let (rest, VarInt(sound_id)) = VarInt::deserialize(input)?;
        tuple((
            cond(sound_id == 0, <(Identifier, Option<f32>)>::deserialize),
            verify(VarInt::deserialize, |value| value.0 > 0).map(|value| value.0 as u32),
            tuple((i32::deserialize, i32::deserialize, i32::deserialize)),
            f32::deserialize,
            f32::deserialize,
            u64::deserialize,
        ))
        .map(
            |(name_and_fixed_range, category, (x, y, z), volume, pitch, seed)| Self {
                id: if sound_id == 0 { None } else { Some(sound_id) },
                name_and_fixed_range,
                category,
                position: [x, y, z],
                volume,
                pitch,
                seed,
            },
        )
        .parse(rest)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct SetTickingState {
    pub tick_rate: f32,
    pub is_frozen: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UpdateAdvancements {
    pub clear: bool,
    pub advancement_map: Vec<(Identifier, Advancement)>,
    pub remove_identifiers: Vec<Identifier>,
    pub progress_map: Vec<(Identifier, AdvancementProgress)>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Advancement {
    pub parent_id: Option<Identifier>,
    pub display_data: Option<AdvancementDisplay>,
    pub requirements_list: Vec<Vec<String>>,
    pub include_in_telemetry_when_complete: bool,
}

#[derive(Clone, Debug)]
pub struct AdvancementDisplay {
    pub title: Chat,
    pub description: Chat,
    pub icon: crafting::Slot,
    pub frame_type: FrameType,
    pub flags: u32,
    pub background_texture: Option<Identifier>,
    pub x_pos: f32,
    pub y_pos: f32,
}

impl Deserialize for AdvancementDisplay {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        let (rest, (title, description, icon, frame_type, flags)) =
            <(Chat, Chat, crafting::Slot, FrameType, u32)>::deserialize
                .context("AdvancementDisplay")
                .parse(input)?;
        let (rest, (background_texture, x_pos, y_pos)) = tuple((
            cond(flags & 0b001 != 0, Identifier::deserialize),
            f32::deserialize,
            f32::deserialize,
        ))
        .context("AdvancementDisplay")
        .parse(rest)?;
        Ok((
            rest,
            Self {
                title,
                description,
                icon,
                frame_type,
                flags,
                background_texture,
                x_pos,
                y_pos,
            },
        ))
    }
}

#[derive(Clone, Copy, Debug)]
pub enum FrameType {
    Task = 0,
    Challenge = 1,
    Goal = 2,
}

impl Deserialize for FrameType {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        byte_enum_parser!(
            0 => Self::Task,
            1 => Self::Challenge,
            2 => Self::Goal,
        )
        .context("FrameType")
        .parse(input)
    }
}

pub type AdvancementProgress = Vec<(Identifier, CriterionProgress)>;

pub type CriterionProgress = Option<u64>;

pub mod serverbound {
    use super::*;

    #[derive(Clone, Copy, Debug, Serialize, PacketWrite)]
    #[packet_write(id = 0x07)]
    pub struct ChunkBatchReceived {
        pub desired_chunks_per_tick: f32,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, PacketWrite)]
    #[packet_write(id = 0x09)]
    pub struct ClientInformation {
        pub locale: [u8; 16],
        pub view_distance: u8,
        pub chat_mode: ChatMode,
        pub chat_colors_enabled: bool,
        pub displayed_skin_parts: u8,
        pub main_hand: MainHand,
        pub text_filtering_enabled: bool,
        pub server_listings_allowed: bool,
    }
}
