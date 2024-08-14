use super::crafting::Slot;
use super::particle::Particle;
use crate::protocol::prelude::*;
use nom::multi::many_till;
use nom::Parser;
use nom_supreme::tag::complete::tag;
use protocol_derive::Deserialize;

// TODO: Couldn't figure out how to get this working quickly, so just stubbed it for now.
// Most of the code needed should be here, just needs a bit of debugging.
#[derive(Clone, Debug, Deserialize)]
#[repr(transparent)]
pub struct EntryList(ProtocolRawBytes);

// #[derive(Clone, Debug)]
// #[repr(transparent)]
// pub struct EntryList(pub Vec<Entry>);

// impl Deserialize for EntryList {
//     fn deserialize(input: InputSpan) -> IResult<Self> {
//         // We can't just implement Deserialize for Entry due to blanket Vec<D: Deserialize> impl
//         many_till(
//             <(u8, EntryValue)>::deserialize.map(|(index, value)| Entry { index, value }),
//             tag(&[0xFF][..]),
//         )
//         .map(|(entries, _)| Self(entries))
//         .parse(input)
//     }
// }

#[derive(Clone, Debug)]
pub struct Entry {
    pub index: u8,
    pub value: EntryValue,
}

#[derive(Clone, Debug, Deserialize)]
#[repr(i32)]
pub enum EntryValue {
    Byte(u8) = 0,
    VarInt(VarInt) = 1,
    VarLong(VarLong) = 2,
    Float(f32) = 3,
    String(String) = 4,
    TextComponent(TextComponent) = 5,
    OptTextComponent(Option<TextComponent>) = 6,
    Slot(Slot) = 7,
    Bool(bool) = 8,
    Rotation {
        x: f32,
        y: f32,
        z: f32,
    } = 9,
    Position(Position) = 10,
    OptPosition(Option<Position>) = 11,
    Direction(AxisDirection) = 12,
    OptUuid(Option<Uuid>) = 13,
    Blockstate(VarInt) = 14,
    OptBlockstate(VarInt) = 15,
    Nbt(NetworkNbtCompound) = 16,
    Particle(Particle) = 17,
    VillagerData {
        ty: VarInt,
        profession: VarInt,
        level: VarInt,
    } = 18,
    OptVarInt(VarInt) = 19,
    Pose(VarInt) = 20,
    CatVariant(VarInt) = 21,
    FrogVariant(VarInt) = 22,
    OptGlobalPos(Option<GlobalPosition>) = 23,
    PaintingVariant(VarInt) = 24,
    SnifferState(VarInt) = 25,
    Vector3 {
        x: f32,
        y: f32,
        z: f32,
    } = 26,
    Quaternion {
        x: f32,
        y: f32,
        z: f32,
        w: f32,
    } = 27,
}

// impl Deserialize for EntryValue {
//     fn deserialize(input: InputSpan) -> IResult<Self> {
//         alt((
//             var_int_tagged_parser!(
//                 0 => u8::deserialize.map(Self::Byte),
//                 1 => VarInt::deserialize.map(Self::VarInt),
//                 2 => VarLong::deserialize.map(Self::VarLong),
//                 3 => f32::deserialize.map(Self::Float),
//                 4 => String::deserialize.map(Self::String),
//                 5 => TextComponent::deserialize.map(Self::Chat),
//                 6 => <Option<TextComponent>>::deserialize.map(Self::OptChat),
//                 7 => Slot::deserialize.map(Self::Slot),
//                 8 => bool::deserialize.map(Self::Bool),
//                 9 => <(f32, f32, f32)>::deserialize.map(|(x, y, z)| Self::Rotation {x, y, z}),
//                 10 => Position::deserialize.map(Self::Position),
//                 11 => <Option<Position>>::deserialize.map(Self::OptPosition),
//                 12 => AxisDirection::deserialize.map(Self::Direction),
//                 13 => <Option<Uuid>>::deserialize.map(Self::OptUuid),
//                 14 => VarInt::deserialize.map(Self::BlockId),
//                 15 => VarInt::deserialize.map(Self::OptBlockId),
//                 16 => NbtCompound::deserialize.map(Self::Nbt),
//                 17 => Particle::deserialize.map(Self::Particle),
//                 18 => <(VarInt, VarInt, VarInt)>::deserialize.map(|(ty, profession, level)| {
//                     Self::VillagerData {
//                         ty,
//                         profession,
//                         level: level.0,
//                     }
//                 }),
//                 20 => VarInt::deserialize.map(Self::OptVarInt),
//             ),
//             var_int_tagged_parser!(
//                 21 => VarInt::deserialize.map(Self::Pose),
//                 22 => VarInt::deserialize.map(Self::CatVariant),
//                 23 => VarInt::deserialize.map(Self::FrogVariant),
//                 24 => <Option<GlobalPosition>>::deserialize.map(Self::OptGlobalPos),
//                 25 => VarInt::deserialize.map(Self::PaintingVariant),
//                 26 => VarInt::deserialize.map(Self::SnifferState),
//                 27 => <(f32, f32, f32)>::deserialize.map(|(x, y, z)| Self::Vector3 {x, y, z}),
//                 28 => <(f32, f32, f32, f32)>::deserialize.map(|(x, y, z, w)| {
//                     Self::Quaternion {x, y, z, w}
//                 }),
//             ),
//         ))(input)
//     }
// }
