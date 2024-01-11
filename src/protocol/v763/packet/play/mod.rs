pub mod crafting;
pub mod entity_metadata;

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
                0x1A => String::deserialize.map(|reason| Self::ErrorDisconnect { reason }).context("ErrorDisconnect"),
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

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct SpawnNonLivingEntity {
    pub id: VarInt,
    pub uuid: Uuid,
    pub ty: VarInt,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub pitch: Angle,
    pub yaw: Angle,
    pub head_yaw: Angle,
    pub data: VarInt,
    pub velocity_x: i16,
    pub velocity_y: i16,
    pub velocity_z: i16,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SetBlockEntityData {
    pub position: Position,
    pub ty: VarInt,
    pub nbt: OptionalNbt,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct BlockUpdate {
    pub position: Position,
    pub block_id: VarInt,
}

// #[derive(Clone, Debug, Deserialize)]
// pub struct CommandInfo {
//     pub nodes: Vec<CommandNode>,
//     pub root_index: VarInt,
// }

// // TODO Convert to enum for type
// #[derive(Clone, Debug)]
// pub struct CommandNode {
//     pub flags: u8,
//     pub children: Vec<VarInt>,
//     pub redirect_node: Option<VarInt>,
//     pub name: Option<String>,
//     pub parser_id: Option<VarInt>,
//     pub properties: Option<VarInt>,
// }

// TODO Move to play/game_event.rs, give better types
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct GameEvent {
    pub event: GameEventType,
    pub value: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum GameEventType {
    NoRespawnBlockAvailable = 0,
    BeginRaining = 1,
    EndRaining = 2,
    ChangeGameMode = 3,
    WinGame = 4,
    DemoEvent = 5,
    ArrowHitPlayer = 6,
    RainLevelChange = 7,
    ThunderLevelChange = 8,
    PlayPufferfishStingSound = 9,
    PlayElderGuardianMobAppearance = 10,
    SetRespawnScreenEnabled = 11,
}

impl Deserialize for GameEventType {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        verify(take(1usize), |variant: &[u8]| variant[0] <= 11)
            .map(|variant: &[u8]| match variant[0] {
                0 => Self::NoRespawnBlockAvailable,
                1 => Self::BeginRaining,
                2 => Self::EndRaining,
                3 => Self::ChangeGameMode,
                4 => Self::WinGame,
                5 => Self::DemoEvent,
                6 => Self::ArrowHitPlayer,
                7 => Self::RainLevelChange,
                8 => Self::ThunderLevelChange,
                9 => Self::PlayPufferfishStingSound,
                10 => Self::PlayElderGuardianMobAppearance,
                11 => Self::SetRespawnScreenEnabled,
                _ => unreachable!(),
            })
            .parse(input)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct InitializeWorldBorder {
    pub x: f64,
    pub z: f64,
    pub old_diameter: f64,
    pub new_diameter: f64,
    pub speed: VarLong,
    pub portal_teleport_boundary: VarInt,
    pub warning_blocks: VarInt,
    pub warning_time: VarInt,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ChunkDataAndUpdateLight {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub heightmaps: NbtCompound,
    pub chunk_data: Vec<u8>,
    pub block_entities: Vec<BlockEntity>,
    pub sky_light_mask: BitVec,
    pub block_light_mask: BitVec,
    pub empty_sky_light_mask: BitVec,
    pub empty_block_light_mask: BitVec,
    pub sky_light_arrays: Vec<Vec<u8>>,
    pub block_light_arrays: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BlockEntity {
    pub packed_xz: u8,
    pub y: i16,
    pub ty: VarInt,
    pub data: OptionalNbt,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LoginPlay {
    pub entity_id: i32,
    pub is_hardcore: bool,
    pub game_mode: GameMode,
    pub previous_game_mode: i8,
    pub dimension_names: Vec<String>,
    pub registry_codec: NbtCompound,
    pub spawn_dimension_type: String,
    pub spawn_dimension_name: String,
    pub hashed_seed: i64,
    pub max_players: VarInt,
    pub view_distance: VarInt,
    pub simulation_distance: VarInt,
    pub reduced_debug_info: bool,
    pub enable_respawn_screen: bool,
    pub is_world_debug: bool,
    pub is_world_flat: bool,
    pub death_position: Option<GlobalPosition>,
    pub portal_cooldown: VarInt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum GameMode {
    Survival = 0,
    Creative = 1,
    Adventure = 2,
    Spectator = 3,
}

impl Deserialize for GameMode {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        byte_enum_parser!(
            0 => Self::Survival,
            1 => Self::Creative,
            2 => Self::Adventure,
            3 => Self::Spectator,
        )
        .context("GameMode")
        .parse(input)
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct UpdateEntityPosition {
    pub entity_id: VarInt,
    pub delta_x: i16,
    pub delta_y: i16,
    pub delta_z: i16,
    pub on_ground: bool,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct UpdateEntityPositionAndRotation {
    pub entity_id: VarInt,
    pub delta_x: i16,
    pub delta_y: i16,
    pub delta_z: i16,
    pub yaw: Angle,
    pub pitch: Angle,
    pub on_ground: bool,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct UpdateEntityRotation {
    pub entity_id: VarInt,
    pub yaw: Angle,
    pub pitch: Angle,
    pub on_ground: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SetContainerContent {
    pub window_id: u8,
    pub state_id: VarInt,
    pub slot_data: Vec<crafting::Slot>,
    pub carried_item: crafting::Slot,
}

#[derive(Clone, Debug)]
pub struct PluginMessage {
    // TODO Make identifier
    pub channel: String,
    pub data: Vec<u8>,
}

impl Deserialize for PluginMessage {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        let (rest, channel) = String::deserialize(input)?;
        Ok((
            &[],
            Self {
                channel,
                data: rest.to_vec(),
            },
        ))
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct DamageEvent {
    pub entity_id: EntityId,
    pub source_type_id: EntityId,
    pub source_cause_id: OptionalEntityId,
    pub source_direct_id: OptionalEntityId,
    pub source_position: Option<(f64, f64, f64)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Difficulty {
    Peaceful = 0,
    Easy = 1,
    Normal = 2,
    Hard = 3,
}

impl Deserialize for Difficulty {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        byte_enum_parser!(
            0 => Self::Peaceful,
            1 => Self::Easy,
            2 => Self::Normal,
            3 => Self::Hard,
        )
        .context("Difficulty")
        .parse(input)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct TagGroup {
    pub tag_type: Identifier,
    pub tags: Vec<Tag>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Tag {
    pub name: Identifier,
    pub ids: Vec<VarInt>,
}

#[derive(Clone, Debug)]
pub struct UpdatePlayerInfo(pub Vec<(Uuid, PlayerInfoUpdateAction)>);

impl Deserialize for UpdatePlayerInfo {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        fn action_parser(flags: u8) -> impl Fn(&[u8]) -> IResult<&[u8], PlayerInfoUpdateAction> {
            move |input: &[u8]| {
                tuple((
                    cond(flags & 0b000001 != 0, PlayerInfoAddPlayer::deserialize),
                    cond(flags & 0b000010 != 0, PlayerInfoInitializeChat::deserialize),
                    cond(flags & 0b000100 != 0, PlayerInfoUpdateGameMode::deserialize),
                    cond(flags & 0b001000 != 0, PlayerInfoUpdateListed::deserialize),
                    cond(flags & 0b010000 != 0, PlayerInfoUpdateLatency::deserialize),
                    cond(
                        flags & 0b100000 != 0,
                        PlayerInfoUpdateDisplayName::deserialize,
                    ),
                ))
                .map(
                    |(
                        add_player,
                        initialize_chat,
                        update_game_mode,
                        update_listed,
                        update_latency,
                        update_display_name,
                    )| PlayerInfoUpdateAction {
                        add_player,
                        initialize_chat,
                        update_game_mode,
                        update_listed,
                        update_latency,
                        update_display_name,
                    },
                )
                .parse(input)
            }
        }
        let (rest, action_flags) = u8::deserialize(input)?;
        length_count(
            verify(VarInt::deserialize, |len| len.0 >= 0).map(|len| len.0 as usize),
            pair(Uuid::deserialize, action_parser(action_flags)),
        )
        .map(Self)
        .parse(rest)
    }
}

#[derive(Clone, Debug)]
pub struct PlayerInfoUpdateAction {
    pub add_player: Option<PlayerInfoAddPlayer>,
    pub initialize_chat: Option<PlayerInfoInitializeChat>,
    pub update_game_mode: Option<PlayerInfoUpdateGameMode>,
    pub update_listed: Option<PlayerInfoUpdateListed>,
    pub update_latency: Option<PlayerInfoUpdateLatency>,
    pub update_display_name: Option<PlayerInfoUpdateDisplayName>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PlayerInfoAddPlayer {
    pub name: String,
    pub properties: Vec<PlayerInfoProperty>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PlayerInfoProperty {
    pub name: String,
    pub value: String,
    pub signature: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[repr(transparent)]
pub struct PlayerInfoInitializeChat {
    pub signature_data: Option<PlayerInfoSignatureData>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PlayerInfoSignatureData {
    pub chat_session_id: Uuid,
    pub public_key_expiry_time: f64,
    pub encoded_public_key: Vec<u8>,
    pub public_key_signature: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize)]
#[repr(transparent)]
pub struct PlayerInfoUpdateGameMode(pub GameMode);

#[derive(Clone, Debug, Deserialize)]
#[repr(transparent)]
pub struct PlayerInfoUpdateListed(pub bool);

#[derive(Clone, Debug, Deserialize)]
#[repr(transparent)]
pub struct PlayerInfoUpdateLatency(pub VarInt);

#[derive(Clone, Debug, Deserialize)]
#[repr(transparent)]
pub struct PlayerInfoUpdateDisplayName(pub Option<Chat>);

#[derive(Clone, Copy, Debug)]
pub struct SynchronizePlayerPosition {
    pub x: PositionChange,
    pub y: PositionChange,
    pub z: PositionChange,
    pub yaw: RotationChange,
    pub pitch: RotationChange,
    pub teleport_id: TeleportId,
}

impl Deserialize for SynchronizePlayerPosition {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        let (rest, (x, y, z, yaw, pitch, flags, teleport_id)) = tuple((
            f64::deserialize,
            f64::deserialize,
            f64::deserialize,
            f32::deserialize,
            f32::deserialize,
            u8::deserialize,
            TeleportId::deserialize,
        ))(input)?;
        let x = match flags & 0b00001 != 0 {
            false => PositionChange::Absolute(x),
            true => PositionChange::Relative(x),
        };
        let y = match flags & 0b00010 != 0 {
            false => PositionChange::Absolute(y),
            true => PositionChange::Relative(y),
        };
        let z = match flags & 0b00100 != 0 {
            false => PositionChange::Absolute(z),
            true => PositionChange::Relative(z),
        };
        let yaw = match flags & 0b01000 != 0 {
            false => RotationChange::Absolute(yaw),
            true => RotationChange::Relative(yaw),
        };
        let pitch = match flags & 0b10000 != 0 {
            false => RotationChange::Absolute(pitch),
            true => RotationChange::Relative(pitch),
        };
        Ok((
            rest,
            Self {
                x,
                y,
                z,
                yaw,
                pitch,
                teleport_id,
            },
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PositionChange {
    Absolute(f64),
    Relative(f64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RotationChange {
    Absolute(f32),
    Relative(f32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub struct TeleportId(VarInt);

#[derive(Clone, Debug)]
pub struct UpdateRecipeBook {
    pub action: UpdateRecipeBookAction,
    pub crafting_recipe_book_open: bool,
    pub crafting_recipe_book_filter_active: bool,
    pub smelting_recipe_book_open: bool,
    pub smelting_recipe_book_filter_active: bool,
    pub blast_furnace_recipe_book_open: bool,
    pub blast_furnace_recipe_book_filter_active: bool,
    pub smoker_recipe_book_open: bool,
    pub smoker_recipe_book_filter_active: bool,
    pub recipe_ids_1: Vec<Identifier>,
    pub recipe_ids_2: Option<Vec<Identifier>>,
}

impl Deserialize for UpdateRecipeBook {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        let (
            rest,
            (
                action,
                crafting_recipe_book_open,
                crafting_recipe_book_filter_active,
                smelting_recipe_book_open,
                smelting_recipe_book_filter_active,
                blast_furnace_recipe_book_open,
                blast_furnace_recipe_book_filter_active,
                smoker_recipe_book_open,
                smoker_recipe_book_filter_active,
                recipe_ids_1,
            ),
        ) = tuple((
            UpdateRecipeBookAction::deserialize,
            bool::deserialize,
            bool::deserialize,
            bool::deserialize,
            bool::deserialize,
            bool::deserialize,
            bool::deserialize,
            bool::deserialize,
            bool::deserialize,
            <Vec<Identifier>>::deserialize,
        ))(input)?;
        let (rest, recipe_ids_2) = match action {
            UpdateRecipeBookAction::Init => {
                let (rest, ids) = <Vec<Identifier>>::deserialize(rest)?;
                (rest, Some(ids))
            }
            _ => (rest, None),
        };
        Ok((
            rest,
            UpdateRecipeBook {
                action,
                crafting_recipe_book_open,
                crafting_recipe_book_filter_active,
                smelting_recipe_book_open,
                smelting_recipe_book_filter_active,
                blast_furnace_recipe_book_open,
                blast_furnace_recipe_book_filter_active,
                smoker_recipe_book_open,
                smoker_recipe_book_filter_active,
                recipe_ids_1,
                recipe_ids_2,
            },
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateRecipeBookAction {
    Init = 0,
    Add = 1,
    Remove = 2,
}

impl Deserialize for UpdateRecipeBookAction {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        byte_enum_parser!(
            0 => Self::Init,
            1 => Self::Add,
            2 => Self::Remove,
        )
        .context("UpdateRecipeBookAction")
        .parse(input)
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct SetHeadRotation {
    pub entity_id: VarInt,
    pub head_yaw: Angle,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UpdateSectionBlocks {
    pub chunk_section_and_position: u64,
    pub blocks: Vec<VarLong>,
}

#[derive(Clone, Debug)]
pub struct ServerData {
    pub motd: Chat,
    pub icon: Option<Vec<u8>>,
    pub enforces_secure_chat: bool,
}

impl Deserialize for ServerData {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        let (rest, (motd, has_icon)) = tuple((Chat::deserialize, bool::deserialize))(input)?;
        let icon = match has_icon {
            false => None,
            true => Some(rest[0..rest.len() - 1].to_vec()),
        };
        let (rest, enforces_secure_chat) = bool::deserialize(&rest[rest.len() - 1..])?;
        Ok((
            rest,
            Self {
                motd,
                icon,
                enforces_secure_chat,
            },
        ))
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct SetCenterChunk {
    pub x: VarInt,
    pub z: VarInt,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct SetDefaultSpawnPosition {
    pub position: Position,
    pub angle: f32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SetEntityMetadata {
    pub entity_id: VarInt,
    pub metadata: entity_metadata::EntryList,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct SetEntityVelocity {
    pub entity_id: VarInt,
    pub velocity_x: i16,
    pub velocity_y: i16,
    pub velocity_z: i16,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EquipmentSlot {
    MainHand,
    OffHand,
    Boots,
    Leggings,
    Chestplate,
    Helmet,
}

impl Deserialize for EquipmentSlot {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        byte_enum_parser!(
            0x00 => Self::MainHand,
            0x01 => Self::OffHand,
            0x02 => Self::Boots,
            0x03 => Self::Leggings,
            0x04 => Self::Chestplate,
            0x05 => Self::Helmet,
            0x80 => Self::MainHand,
            0x81 => Self::OffHand,
            0x82 => Self::Boots,
            0x83 => Self::Leggings,
            0x84 => Self::Chestplate,
            0x85 => Self::Helmet,
        )
        .context("EquipmentSlot")
        .parse(input)
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct SetExperience {
    pub experience_bar: f32,
    pub total_experience: VarInt,
    pub level: VarInt,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct SetHealth {
    pub health: f32,
    pub food: VarInt,
    pub food_saturation: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct UpdateTime {
    pub world_age: u64,
    pub time_of_day: u64,
    pub sun_frozen: bool,
}

impl Deserialize for UpdateTime {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        pair(u64::deserialize, i64::deserialize)
            .map(|(world_age, signed_time_of_day)| Self {
                world_age,
                time_of_day: signed_time_of_day.abs() as u64,
                sun_frozen: signed_time_of_day < 0,
            })
            .parse(input)
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct TeleportEntity {
    pub entity_id: VarInt,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: Angle,
    pub pitch: Angle,
    pub on_ground: bool,
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
    pub criteria: Vec<Identifier>,
    pub requirements_list: Vec<Vec<String>>,
    pub include_in_telemetry_when_complete: bool,
}

#[derive(Clone, Debug)]
pub struct AdvancementDisplay {
    pub title: Chat,
    pub description: Chat,
    pub icon: crafting::Slot,
    pub frame_type: FrameType,
    pub flags: u8,
    pub background_texture: Option<Identifier>,
    pub x_pos: f32,
    pub y_pos: f32,
}

impl Deserialize for AdvancementDisplay {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        let (rest, (title, description, icon, frame_type, flags)) =
            <(Chat, Chat, crafting::Slot, FrameType, u8)>::deserialize
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
    Task,
    Challenge,
    Goal,
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

#[derive(Clone, Debug, Deserialize)]
pub struct UpdateEntityAttributes {
    pub entity_id: VarInt,
    pub properties: Vec<EntityAttribute>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EntityAttribute {
    pub key: Identifier,
    pub value: f64,
    pub modifiers: Vec<EntityAttributeModifier>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EntityAttributeModifier {
    pub uuid: Uuid,
    pub amount: f64,
    pub operation: EntityAttributeModifierOperation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityAttributeModifierOperation {
    Add,
    AddPercentage,
    MultiplyPercentage,
}

impl Deserialize for EntityAttributeModifierOperation {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        byte_enum_parser!(
            0 => Self::Add,
            1 => Self::AddPercentage,
            2 => Self::MultiplyPercentage,
        )
        .context("EntityAttributeModifierOperation")
        .parse(input)
    }
}
