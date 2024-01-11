use super::super::super::prelude::*;
use super::crafting::Slot;
use nom::branch::alt;
use nom::multi::many_till;
use nom::Parser;
use nom_supreme::tag::complete::tag;
use quartz_nbt::NbtCompound;
use uuid::Uuid;

#[derive(Clone, Debug)]
#[repr(transparent)]
pub struct EntryList(pub Vec<Entry>);

impl Deserialize for EntryList {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        // We can't just implement Deserialize for Entry due to blanket Vec<D: Deserialize> impl
        many_till(
            <(u8, EntryValue)>::deserialize.map(|(index, value)| Entry { index, value }),
            tag(&[0xFF][..]),
        )
        .map(|(entries, _)| Self(entries))
        .parse(input)
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub index: u8,
    pub value: EntryValue,
}

#[derive(Clone, Debug)]
pub enum EntryValue {
    Byte(u8),
    VarInt(VarInt),
    VarLong(VarLong),
    Float(f32),
    String(String),
    Chat(Chat),
    OptChat(Option<Chat>),
    Slot(Slot),
    Bool(bool),
    // TODO Convert this to maths type in game maths library
    Rotation {
        x: f32,
        y: f32,
        z: f32,
    },
    Position(Position),
    OptPosition(Option<Position>),
    Direction(AxisDirection),
    OptUuid(Option<Uuid>),
    BlockId(VarInt),
    OptBlockId(VarInt),
    Nbt(NbtCompound),
    Particle(Particle),
    VillagerData {
        ty: VarInt,
        profession: VarInt,
        level: i32,
    },
    // TODO Figure out what this is, consider converting to OptEntityId
    OptVarInt(VarInt),
    // TODO Convert to enum
    Pose(VarInt),
    // TODO Consider making some RegistryId type for these or something
    CatVariant(VarInt),
    FrogVariant(VarInt),
    OptGlobalPos(Option<GlobalPosition>),
    PaintingVariant(VarInt),
    // TODO Convert to enum
    SnifferState(VarInt),
    // TODO Convert these to maths types in game maths library
    Vector3 {
        x: f32,
        y: f32,
        z: f32,
    },
    Quaternion {
        x: f32,
        y: f32,
        z: f32,
        w: f32,
    },
}

impl Deserialize for EntryValue {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        alt((
            var_int_tagged_parser!(
                0 => u8::deserialize.map(Self::Byte),
                1 => VarInt::deserialize.map(Self::VarInt),
                2 => VarLong::deserialize.map(Self::VarLong),
                3 => f32::deserialize.map(Self::Float),
                4 => String::deserialize.map(Self::String),
                5 => Chat::deserialize.map(Self::Chat),
                6 => <Option<Chat>>::deserialize.map(Self::OptChat),
                7 => Slot::deserialize.map(Self::Slot),
                8 => bool::deserialize.map(Self::Bool),
                9 => <(f32, f32, f32)>::deserialize.map(|(x, y, z)| Self::Rotation {x, y, z}),
                10 => Position::deserialize.map(Self::Position),
                11 => <Option<Position>>::deserialize.map(Self::OptPosition),
                12 => AxisDirection::deserialize.map(Self::Direction),
                13 => <Option<Uuid>>::deserialize.map(Self::OptUuid),
                14 => VarInt::deserialize.map(Self::BlockId),
                15 => VarInt::deserialize.map(Self::OptBlockId),
                16 => NbtCompound::deserialize.map(Self::Nbt),
                17 => Particle::deserialize.map(Self::Particle),
                18 => <(VarInt, VarInt, VarInt)>::deserialize.map(|(ty, profession, level)| {
                    Self::VillagerData {
                        ty,
                        profession,
                        level: level.0,
                    }
                }),
                20 => VarInt::deserialize.map(Self::OptVarInt),
            ),
            var_int_tagged_parser!(
                21 => VarInt::deserialize.map(Self::Pose),
                22 => VarInt::deserialize.map(Self::CatVariant),
                23 => VarInt::deserialize.map(Self::FrogVariant),
                24 => <Option<GlobalPosition>>::deserialize.map(Self::OptGlobalPos),
                25 => VarInt::deserialize.map(Self::PaintingVariant),
                26 => VarInt::deserialize.map(Self::SnifferState),
                27 => <(f32, f32, f32)>::deserialize.map(|(x, y, z)| Self::Vector3 {x, y, z}),
                28 => <(f32, f32, f32, f32)>::deserialize.map(|(x, y, z, w)| {
                    Self::Quaternion {x, y, z, w}
                }),
            ),
        ))(input)
    }
}
