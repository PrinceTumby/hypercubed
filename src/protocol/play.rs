pub mod crafting;
pub mod entity_metadata;
pub mod particle;

use super::PluginMessage;
use super::chunk::RawChunkLightInfo;
pub use super::configuration::{ChatMode, MainHand};
use super::prelude::*;
use crate::portable_prelude::*;
use nom::Parser;
use nom::bytes::complete::take;
use nom::combinator::{cond, verify};
use nom::multi::length_count;
use nom::sequence::pair;
use protocol_derive::{Deserialize, PacketRead, PacketWrite, Serialize};
use resources::block::GlobalPaletteIndex;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, PacketRead)]
#[repr(i32)]
pub enum Clientbound {
    ErrorDisconnect {
        reason: TextComponent,
    } = 0x1D,
    BundleDelimiter = 0x00,
    SpawnEntity(SpawnEntityInfo) = 0x01,
    SetEntityAnimation(SetEntityAnimation) = 0x03,
    SetBlockEntityData(SetBlockEntityData) = 0x07,
    BlockAction {
        pos: Position,
        action_id: u8,
        action_data: u8,
        block_registry_id: VarInt,
    } = 0x08,
    BlockUpdate(BlockUpdate) = 0x09,
    ChangeDifficulty {
        difficulty: Difficulty,
        is_locked: bool,
    } = 0x0B,
    ChunkBatchEnd {
        num_chunks: VarInt,
    } = 0x0C,
    ChunkBatchStart = 0x0D,
    DeclareCommands {
        nodes: Vec<CommandNode>,
        root_index: VarInt,
    } = 0x11,
    SetContainerContent(SetContainerContent) = 0x13,
    PluginMessage(PluginMessage) = 0x19,
    DamageEvent(DamageEvent) = 0x1A,
    EntityEvent {
        id: i32,
        status: u8,
    } = 0x1F,
    Explosion {
        base_coords: [f64; 3],
        strength: f32,
        affected_block_offsets: Vec<[i8; 3]>,
        player_push_velocity: [f32; 3],
        block_interaction: ExplosionBlockInteraction,
        small_explosion_particle: particle::Particle,
        large_explosion_particle: particle::Particle,
        // FIXME: Haven't figured out format of this yet, wiki.vg seems to be wrong.
        sound_data: ProtocolRawBytes,
        // sound_name: Identifier,
        // fixed_range: Option<f32>,
    } = 0x20,
    UnloadChunk {
        chunk_z: i32,
        chunk_x: i32,
    } = 0x21,
    GameEvent {
        event: GameEventType,
        value: f32,
    } = 0x22,
    InitializeWorldBorder(InitializeWorldBorder) = 0x25,
    KeepAlive {
        id: u64,
    } = 0x26,
    ChunkDataAndUpdateLight(ChunkDataAndUpdateLight) = 0x27,
    WorldEvent(WorldEvent) = 0x28,
    DisplayParticle {
        is_long_distance: bool,
        pos: [f64; 3],
        random_offset_magnitude: [f32; 3],
        max_speed: f32,
        num_particles: u32,
        particle: particle::Particle,
    } = 0x29,
    UpdateLight {
        chunk_xz: [VarInt; 2],
        light_info: RawChunkLightInfo,
    } = 0x2A,
    LoginPlay {
        raw_entity_id: u32,
        is_hardcore: bool,
        dimension_names: Vec<String>,
        max_players: VarInt,
        view_distance: VarInt,
        simulation_distance: VarInt,
        reduced_debug_info: bool,
        enable_respawn_screen: bool,
        is_crafting_limited: bool,
        spawn_dimension_type: String,
        spawn_dimension_name: String,
        hashed_seed: i64,
        game_mode: GameMode,
        previous_game_mode: i8,
        is_world_debug: bool,
        is_world_flat: bool,
        death_position: Option<GlobalPosition>,
        portal_cooldown: VarInt,
        is_secure_chat_enforced: bool,
    } = 0x2B,
    UpdateEntityPosition(UpdateEntityPosition) = 0x2E,
    UpdateEntityPositionAndRotation(UpdateEntityPositionAndRotation) = 0x2F,
    UpdateEntityRotation(UpdateEntityRotation) = 0x30,
    PlayerAbilities {
        flags: u8,
        fly_speed: f32,
        fov_modifier: f32,
    } = 0x38,
    RemovePlayersInfo(Vec<Uuid>) = 0x3D,
    UpdatePlayerInfo(UpdatePlayerInfo) = 0x3E,
    SynchronizePlayerPosition(SynchronizePlayerPosition) = 0x40,
    UpdateRecipeBook(UpdateRecipeBook) = 0x41,
    RemoveEntities(Vec<VarInt>) = 0x42,
    SetHeadRotation(SetHeadRotation) = 0x48,
    UpdateSectionBlocks(UpdateSectionBlocks) = 0x49,
    ServerData(ServerData) = 0x4B,
    SetHeldItem {
        slot: u8,
    } = 0x53,
    SetCenterChunk(SetCenterChunk) = 0x54,
    SetDefaultSpawnPosition(SetDefaultSpawnPosition) = 0x56,
    SetEntityMetadata {
        entity_id: VarInt,
        metadata: entity_metadata::EntryList,
    } = 0x58,
    SetEntityVelocity {
        entity_id: EntityId,
        velocity: [i16; 3],
    } = 0x5A,
    SetEquipment(SetEquipment) = 0x5B,
    SetExperience(SetExperience) = 0x5C,
    SetHealth {
        health: f32,
        food: VarInt,
        food_saturation: f32,
    } = 0x5D,
    UpdateTime(WorldTime) = 0x64,
    PlaySoundEffect(PlaySoundEffect) = 0x68,
    SystemChatMessage {
        content: TextComponent,
        at_action_bar: bool,
    } = 0x6C,
    SetTabListHeaderAndFooter {
        header: TextComponent,
        footer: TextComponent,
    } = 0x6D,
    AnimatePickupEntity {
        collected_entity_id: EntityId,
        collector_entity_id: EntityId,
        count: VarInt,
    } = 0x6F,
    TeleportEntity {
        id: EntityId,
        coords: [f64; 3],
        yaw: Angle,
        pitch: Angle,
        on_ground: bool,
    } = 0x70,
    SetTickingState(TickingState) = 0x71,
    StepTicks(VarInt) = 0x72,
    UpdateAdvancements(UpdateAdvancements) = 0x74,
    UpdateEntityAttributes(UpdateEntityAttributes) = 0x75,
    UpdateRecipes(Vec<crafting::Recipe>) = 0x77,
    UpdateTags(Vec<TagGroup>) = 0x78,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct SpawnEntityInfo {
    pub id: EntityId,
    pub uuid: Uuid,
    pub entity_type: VarInt,
    pub coords: [f64; 3],
    pub pitch: Angle,
    pub yaw: Angle,
    pub head_yaw: Angle,
    pub data: VarInt,
    pub velocity: [i16; 3],
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct SetEntityAnimation {
    pub entity_id: EntityId,
    pub animation: SetEntityAnimationType,
}

#[derive(Clone, Copy, Debug)]
pub enum SetEntityAnimationType {
    SwingPrimaryArm = 0,
    SwingSecondaryArm = 2,
    LeaveBed = 3,
    CriticalEffect = 4,
    MagicalCriticalEffect = 5,
}

impl Deserialize for SetEntityAnimationType {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        verify(take(1usize), |variant: &InputSpan| {
            variant[0] <= 5 && variant[0] != 1
        })
        .map(|variant: InputSpan| match variant[0] {
            0 => Self::SwingPrimaryArm,
            2 => Self::LeaveBed,
            3 => Self::SwingSecondaryArm,
            4 => Self::CriticalEffect,
            5 => Self::MagicalCriticalEffect,
            _ => unreachable!(),
        })
        .parse(input)
    }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    fn deserialize(input: InputSpan) -> IResult<Self> {
        verify(take(1usize), |variant: &InputSpan| variant[0] <= 13)
            .map(|variant: InputSpan| match variant[0] {
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
    pub chunk_xz: [i32; 2],
    pub heightmaps: NetworkNbt,
    pub chunk_data: Vec<u8>,
    pub block_entities: Vec<BlockEntity>,
    pub light_info: RawChunkLightInfo,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BlockEntity {
    pub packed_xz: u8,
    pub y: i16,
    pub ty: VarInt,
    pub data: OptionalNbt,
}

#[derive(Clone, Debug, Deserialize)]
pub struct WorldEvent {
    pub event: u32,
    pub location: Position,
    pub extra_data: u32,
    pub disable_volume_distance: bool,
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
    fn deserialize(input: InputSpan) -> IResult<Self> {
        nom_context(
            "GameMode",
            byte_enum_parser!(
                0 => Self::Survival,
                1 => Self::Creative,
                2 => Self::Adventure,
                3 => Self::Spectator,
            ),
        )
        .parse(input)
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct UpdateEntityPosition {
    pub entity_id: EntityId,
    pub delta_x: i16,
    pub delta_y: i16,
    pub delta_z: i16,
    pub on_ground: bool,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct UpdateEntityPositionAndRotation {
    pub entity_id: EntityId,
    pub delta_x: i16,
    pub delta_y: i16,
    pub delta_z: i16,
    pub yaw: Angle,
    pub pitch: Angle,
    pub on_ground: bool,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct UpdateEntityRotation {
    pub entity_id: EntityId,
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

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct DamageEvent {
    pub entity_id: EntityId,
    pub source_type_id: EntityId,
    pub source_cause_id: OptionalEntityId,
    pub source_direct_id: OptionalEntityId,
    pub source_position: Option<(f64, f64, f64)>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum ExplosionBlockInteraction {
    Keep = 0,
    Destroy = 1,
    DestroyWithDecay = 2,
    TriggerBlock = 3,
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
    fn deserialize(input: InputSpan) -> IResult<Self> {
        nom_context(
            "Difficulty",
            byte_enum_parser!(
                0 => Self::Peaceful,
                1 => Self::Easy,
                2 => Self::Normal,
                3 => Self::Hard,
            ),
        )
        .parse(input)
    }
}

#[derive(Clone, Debug)]
pub struct CommandNode {
    pub child_indices: Vec<VarInt>,
    pub redirect_node: Option<VarInt>,
    pub info: CommandNodeInfo,
    pub is_executable: bool,
}

impl Deserialize for CommandNode {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        let (rest, flags) = nom_context("CommandNode.flags", u8::deserialize).parse(input)?;
        match flags & 0x03 {
            // Root
            0 => {
                assert!(flags & 0x10 == 0);
                nom_context(
                    "CommandNode::Root",
                    pair(
                        <Vec<VarInt>>::deserialize,
                        cond(flags & 0x08 != 0, VarInt::deserialize),
                    ),
                )
                .map(move |(child_indices, redirect_node)| CommandNode {
                    child_indices,
                    redirect_node,
                    info: CommandNodeInfo::Root,
                    is_executable: flags & 0x04 != 0,
                })
                .parse(rest)
            }
            // Literal
            1 => {
                assert!(flags & 0x10 == 0);
                nom_context(
                    "CommandNode::Literal",
                    (
                        <Vec<VarInt>>::deserialize,
                        cond(flags & 0x08 != 0, VarInt::deserialize),
                        String::deserialize,
                    ),
                )
                .map(move |(child_indices, redirect_node, name)| CommandNode {
                    child_indices,
                    redirect_node,
                    info: CommandNodeInfo::Literal { name },
                    is_executable: flags & 0x04 != 0,
                })
                .parse(rest)
            }
            // Argument
            2 => nom_context(
                "CommandNode::Argument",
                (
                    <Vec<VarInt>>::deserialize,
                    cond(flags & 0x08 != 0, VarInt::deserialize),
                    String::deserialize,
                    CommandNodeParserInfo::deserialize,
                    cond(flags & 0x10 != 0, Identifier::deserialize),
                ),
            )
            .map(
                move |(child_indices, redirect_node, name, parser_info, suggestions_type)| {
                    CommandNode {
                        child_indices,
                        redirect_node,
                        info: CommandNodeInfo::Argument {
                            name,
                            parser_info,
                            suggestions_type,
                        },
                        is_executable: flags & 0x04 != 0,
                    }
                },
            )
            .parse(rest),
            _ => unimplemented!(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum CommandNodeInfo {
    Root,
    Literal {
        name: String,
    },
    Argument {
        name: String,
        parser_info: CommandNodeParserInfo,
        suggestions_type: Option<Identifier>,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[repr(i32)]
pub enum CommandNodeParserInfo {
    Bool = 0,
    Float(CommandNodeFloatParserInfo) = 1,
    Double(CommandNodeDoubleParserInfo) = 2,
    Int(CommandNodeIntParserInfo) = 3,
    Long(CommandNodeLongParserInfo) = 4,
    String(CommandNodeStringParserInfo) = 5,
    Entity { flags: u8 } = 6,
    GameProfile = 7,
    BlockPos = 8,
    ColumnPos = 9,
    Vec3 = 10,
    Vec2 = 11,
    BlockState = 12,
    BlockPredicate = 13,
    ItemStack = 14,
    ItemPredicate = 15,
    Color = 16,
    JSONTextComponent = 17,
    Style = 18,
    Message = 19,
    Nbt = 20,
    NbtTag = 21,
    NbtPath = 22,
    Objective = 23,
    ObjectiveCriteria = 24,
    Operation = 25,
    Particle = 26,
    Angle = 27,
    Rotation = 28,
    ScoreboardSlot = 29,
    ScoreHolder { flags: u8 } = 30,
    Swizzle = 31,
    Team = 32,
    ItemSlot = 33,
    ResourceLocation = 34,
    Function = 35,
    EntityAnchor = 36,
    IntRange = 37,
    FloatRange = 38,
    Dimension = 39,
    Gamemode = 40,
    // FIXME: ImHex testing seems to indicate that there's an unknown parser somewhere before 42,
    // as on wiki.vg `Time` is at 41. The packet doesn't parse without having this here. Needs more
    // digging.
    Unknown = 41,
    Time { min: i32 } = 42,
    ResourceOrTag { registry: Identifier } = 43,
    ResourceOrTagKey { registry: Identifier } = 44,
    Resource { registry: Identifier } = 45,
    ResourceKey { registry: Identifier } = 46,
    TemplateMirror = 47,
    TemplateRotation = 48,
    Heightmap = 49,
    Uuid = 50,
    // FIXME: See above, more ImHex digging found even more parser IDs.
    Unknown2 = 51,
    Unknown3 = 52,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CommandNodeFloatParserInfo {
    min: f32,
    max: f32,
}

impl Deserialize for CommandNodeFloatParserInfo {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        let (rest, flags) = u8::deserialize(input)?;
        nom_context(
            "CommandNodeFloatParserInfo",
            pair(
                cond(flags & 0b01 != 0, f32::deserialize).map(|v| v.unwrap_or(f32::MIN)),
                cond(flags & 0b10 != 0, f32::deserialize).map(|v| v.unwrap_or(f32::MAX)),
            ),
        )
        .map(|(min, max)| Self { min, max })
        .parse(rest)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CommandNodeDoubleParserInfo {
    min: f64,
    max: f64,
}

impl Deserialize for CommandNodeDoubleParserInfo {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        let (rest, flags) = u8::deserialize(input)?;
        nom_context(
            "CommandNodeDoubleParserInfo",
            pair(
                cond(flags & 0b01 != 0, f64::deserialize).map(|v| v.unwrap_or(f64::MIN)),
                cond(flags & 0b10 != 0, f64::deserialize).map(|v| v.unwrap_or(f64::MAX)),
            ),
        )
        .map(|(min, max)| Self { min, max })
        .parse(rest)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CommandNodeIntParserInfo {
    min: i32,
    max: i32,
}

impl Deserialize for CommandNodeIntParserInfo {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        let (rest, flags) = u8::deserialize(input)?;
        nom_context(
            "CommandNodeIntParserInfo",
            pair(
                cond(flags & 0b01 != 0, i32::deserialize).map(|v| v.unwrap_or(i32::MIN)),
                cond(flags & 0b10 != 0, i32::deserialize).map(|v| v.unwrap_or(i32::MAX)),
            ),
        )
        .map(|(min, max)| Self { min, max })
        .parse(rest)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CommandNodeLongParserInfo {
    min: i64,
    max: i64,
}

impl Deserialize for CommandNodeLongParserInfo {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        let (rest, flags) = u8::deserialize(input)?;
        nom_context(
            "CommandNodeLongParserInfo",
            pair(
                cond(flags & 0b01 != 0, i64::deserialize).map(|v| v.unwrap_or(i64::MIN)),
                cond(flags & 0b10 != 0, i64::deserialize).map(|v| v.unwrap_or(i64::MAX)),
            ),
        )
        .map(|(min, max)| Self { min, max })
        .parse(rest)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum CommandNodeStringParserInfo {
    SingleWord = 0,
    QuotablePhrase = 1,
    GreedyPhrase = 2,
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
    fn deserialize(input: InputSpan) -> IResult<Self> {
        fn action_parser<'a>(
            flags: u8,
        ) -> impl Parser<InputSpan<'a>, Output = PlayerInfoUpdateAction, Error = IErr<'a>> {
            struct ActionParser {
                flags: u8,
            }

            impl<'a> Parser<InputSpan<'a>> for ActionParser {
                type Output = PlayerInfoUpdateAction;
                type Error = IErr<'a>;

                fn process<OM: nom::OutputMode>(
                    &mut self,
                    input: InputSpan<'a>,
                ) -> nom::PResult<OM, InputSpan<'a>, Self::Output, Self::Error> {
                    let flags = self.flags;
                    (
                        cond(flags & 0b000001 != 0, PlayerInfoAddPlayer::deserialize),
                        cond(flags & 0b000010 != 0, PlayerInfoInitializeChat::deserialize),
                        cond(flags & 0b000100 != 0, GameMode::deserialize),
                        cond(flags & 0b001000 != 0, bool::deserialize),
                        cond(flags & 0b010000 != 0, VarInt::deserialize),
                        cond(flags & 0b100000 != 0, <Option<TextComponent>>::deserialize),
                    )
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
                        .process::<OM>(input)
                }
            }

            ActionParser { flags }
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
    pub update_game_mode: Option<GameMode>,
    pub update_listed: Option<bool>,
    pub update_latency: Option<VarInt>,
    pub update_display_name: Option<Option<TextComponent>>,
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

#[derive(Clone, Copy, Debug)]
pub struct SynchronizePlayerPosition {
    pub x: PositionChange,
    pub y: PositionChange,
    pub z: PositionChange,
    pub yaw: RotationChange,
    pub pitch: RotationChange,
    pub teleport_id: VarInt,
}

impl Deserialize for SynchronizePlayerPosition {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        let (rest, (x, y, z, yaw, pitch, flags, teleport_id)) = (
            f64::deserialize,
            f64::deserialize,
            f64::deserialize,
            f32::deserialize,
            f32::deserialize,
            u8::deserialize,
            VarInt::deserialize,
        )
            .parse(input)?;
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
    fn deserialize(input: InputSpan) -> IResult<Self> {
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
        ) = (
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
        )
            .parse(input)?;
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
    fn deserialize(input: InputSpan) -> IResult<Self> {
        nom_context(
            "UpdateRecipeBookAction",
            byte_enum_parser!(
                0 => Self::Init,
                1 => Self::Add,
                2 => Self::Remove,
            ),
        )
        .parse(input)
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct SetHeadRotation {
    pub entity_id: VarInt,
    pub head_yaw: Angle,
}

#[derive(Clone, Debug)]
pub struct UpdateSectionBlocks {
    pub subchunk_coords: [i32; 3],
    pub blocks: Vec<([u8; 3], GlobalPaletteIndex)>,
}

impl Deserialize for UpdateSectionBlocks {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        nom_context("UpdateSedctionBlocks", <(i64, Vec<VarLong>)>::deserialize)
            .map(|(packed_coords, packed_blocks)| UpdateSectionBlocks {
                subchunk_coords: [
                    (packed_coords >> 42) as i32,
                    (packed_coords << 44 >> 44) as i32,
                    (packed_coords << 22 >> 42) as i32,
                ],
                blocks: packed_blocks
                    .into_iter()
                    .map(|packed_block| {
                        (
                            [
                                ((packed_block.0 >> 8) & 0xF) as u8,
                                (packed_block.0 & 0xF) as u8,
                                ((packed_block.0 >> 4) & 0xF) as u8,
                            ],
                            (packed_block.0 >> 12).try_into().unwrap(),
                        )
                    })
                    .collect(),
            })
            .parse(input)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct ServerData {
    pub motd: TextComponent,
    pub icon: Option<Vec<u8>>,
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

#[derive(Clone, Debug)]
pub struct SetEquipment {
    pub entity_id: EntityId,
    pub equipment: Vec<EquipmentPiece>,
}

impl Deserialize for SetEquipment {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        let (mut rest, entity_id) =
            nom_context("SetEquipment::entity_id", EntityId::deserialize).parse(input)?;
        let mut more_pieces = true;
        let mut equipment = Vec::new();
        while more_pieces {
            more_pieces = rest[0] & 0x80 != 0;
            let (new_rest, equipment_piece) =
                nom_context("SetEquipment::equipment", EquipmentPiece::deserialize).parse(rest)?;
            equipment.push(equipment_piece);
            rest = new_rest;
        }
        Ok((
            rest,
            Self {
                entity_id,
                equipment,
            },
        ))
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct EquipmentPiece {
    pub slot: EquipmentSlot,
    pub item: crafting::Slot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EquipmentSlot {
    MainHand = 0,
    OffHand = 1,
    Boots = 2,
    Leggings = 3,
    Chestplate = 4,
    Helmet = 5,
    Body = 6,
}

impl Deserialize for EquipmentSlot {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        nom_context(
            "EquipmentSlot",
            byte_enum_parser!(
                0x00 => Self::MainHand,
                0x01 => Self::OffHand,
                0x02 => Self::Boots,
                0x03 => Self::Leggings,
                0x04 => Self::Chestplate,
                0x05 => Self::Helmet,
                0x06 => Self::Body,
                0x80 => Self::MainHand,
                0x81 => Self::OffHand,
                0x82 => Self::Boots,
                0x83 => Self::Leggings,
                0x84 => Self::Chestplate,
                0x85 => Self::Helmet,
                0x86 => Self::Body,
            ),
        )
        .parse(input)
    }
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
    fn deserialize(input: InputSpan) -> IResult<Self> {
        let (rest, VarInt(sound_id)) = VarInt::deserialize(input)?;
        (
            cond(sound_id == 0, <(Identifier, Option<f32>)>::deserialize),
            verify(VarInt::deserialize, |value| value.0 > 0).map(|value| value.0 as u32),
            (i32::deserialize, i32::deserialize, i32::deserialize),
            f32::deserialize,
            f32::deserialize,
            u64::deserialize,
        )
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

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct SetExperience {
    pub experience_bar: f32,
    pub total_experience: VarInt,
    pub level: VarInt,
}

#[derive(Clone, Copy, Debug)]
pub struct WorldTime {
    pub world_age: u64,
    pub time_of_day: u64,
    pub sun_frozen: bool,
}

impl Deserialize for WorldTime {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        pair(u64::deserialize, i64::deserialize)
            .map(|(world_age, signed_time_of_day)| Self {
                world_age,
                time_of_day: signed_time_of_day.unsigned_abs(),
                sun_frozen: signed_time_of_day < 0,
            })
            .parse(input)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct UpdateEntityAttributes {
    pub entity_id: VarInt,
    pub properties: Vec<EntityAttribute>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EntityAttribute {
    pub id: VarInt,
    pub value: f64,
    pub modifiers: Vec<EntityAttributeModifier>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EntityAttributeModifier {
    pub id: Identifier,
    pub amount: f64,
    pub operation: EntityAttributeModifierOperation,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum EntityAttributeModifierOperation {
    Add = 0,
    AddPercentage = 1,
    MultiplyPercentage = 2,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct TickingState {
    pub ticks_per_second: f32,
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
    pub title: TextComponent,
    pub description: TextComponent,
    pub icon: crafting::Slot,
    pub frame_type: FrameType,
    pub flags: u32,
    pub background_texture: Option<Identifier>,
    pub x_pos: f32,
    pub y_pos: f32,
}

impl Deserialize for AdvancementDisplay {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        let (rest, (title, description, icon, frame_type, flags)) = nom_context(
            "AdvancementDisplay",
            <(TextComponent, TextComponent, crafting::Slot, FrameType, u32)>::deserialize,
        )
        .parse(input)?;
        let (rest, (background_texture, x_pos, y_pos)) = nom_context(
            "AdvancementDisplay",
            (
                cond(flags & 0b001 != 0, Identifier::deserialize),
                f32::deserialize,
                f32::deserialize,
            ),
        )
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
    fn deserialize(input: InputSpan) -> IResult<Self> {
        nom_context(
            "FrameType",
            byte_enum_parser!(
                0 => Self::Task,
                1 => Self::Challenge,
                2 => Self::Goal,
            ),
        )
        .parse(input)
    }
}

pub type AdvancementProgress = Vec<(Identifier, CriterionProgress)>;

pub type CriterionProgress = Option<u64>;

pub mod serverbound {
    use super::*;

    #[derive(Clone, Copy, Debug, Serialize, PacketWrite)]
    #[packet_write(id = 0x00)]
    pub struct ConfirmTeleportation {
        pub id: VarInt,
    }

    #[derive(Clone, Debug, Serialize, PacketWrite)]
    #[packet_write(id = 0x04)]
    pub struct ChatCommand(pub String);

    #[derive(Clone, Copy, Debug, Serialize, PacketWrite)]
    #[packet_write(id = 0x08)]
    pub struct ChunkBatchReceived {
        pub desired_chunks_per_tick: f32,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, PacketWrite)]
    #[packet_write(id = 0x0A)]
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

    #[derive(Clone, Copy, Debug, Serialize, PacketWrite)]
    #[packet_write(id = 0x18)]
    pub struct KeepAliveResponse {
        pub id: u64,
    }

    #[derive(Clone, Copy, Debug, Serialize, PacketWrite)]
    #[packet_write(id = 0x1B)]
    pub struct SetPlayerPositionAndRotation {
        pub x: f64,
        pub feet_y: f64,
        pub z: f64,
        pub mc_yaw: f32,
        pub mc_pitch: f32,
        pub on_ground: bool,
    }
}
