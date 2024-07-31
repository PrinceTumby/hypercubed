use super::prelude::*;
use nom::bytes::complete::{take, take_while, take_while1};
use nom::combinator::{recognize, success, verify};
use nom::error::ParseError;
use nom::multi::{length_count, length_value};
use nom::sequence::{pair, tuple};
use nom::Parser;
use nom_supreme::parser_ext::ParserExt;
use quartz_nbt::NbtCompound;
use std::io::Cursor;
use uuid::Uuid;

// Helper macros

macro_rules! byte_enum_parser {
    ($( $tag_byte:expr => $value:expr $(,)? )+) => {{
        nom::branch::alt((
            $(
                nom::combinator::value($value, nom_supreme::tag::complete::tag(&[$tag_byte][..])),
            )+
        ))
    }}
}

// Unit

impl Deserialize for () {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        Ok((input, ()))
    }
}

// Boolean

impl Deserialize for bool {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        byte_enum_parser!(
            0 => false,
            1 => true,
        )
        .context("bool")
        .parse(input)
    }
}

impl Serialize for bool {
    fn serialize_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(&[*self as u8])
    }
}

// Integers

impl Deserialize for u8 {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        take(1usize)
            .map(|slice: &[u8]| slice[0])
            .context("u8")
            .parse(input)
    }
}

impl Deserialize for i8 {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        take(1usize)
            .map(|slice: &[u8]| slice[0] as i8)
            .context("i8")
            .parse(input)
    }
}

impl Serialize for u8 {
    fn serialize_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(&[*self])
    }
}

impl Serialize for i8 {
    fn serialize_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(&[*self as u8])
    }
}

macro_rules! impl_number_serialize {
    ($num_type:ty) => {
        impl Serialize for $num_type {
            fn serialize_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
                writer.write_all(&Self::to_be_bytes(*self))
            }
        }
    };
}

macro_rules! impl_number_deserialize {
    ($num_type:ty, $deserializer:expr) => {
        impl Deserialize for $num_type {
            fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
                $deserializer.context(stringify!($num_type)).parse(input)
            }
        }
    };
}

impl_number_serialize!(u16);

impl_number_serialize!(u32);

impl_number_serialize!(u64);

impl_number_serialize!(i16);

impl_number_serialize!(i32);

impl_number_serialize!(i64);

impl_number_deserialize!(u16, nom::number::complete::be_u16);

impl_number_deserialize!(u32, nom::number::complete::be_u32);

impl_number_deserialize!(u64, nom::number::complete::be_u64);

impl_number_deserialize!(i16, nom::number::complete::be_i16);

impl_number_deserialize!(i32, nom::number::complete::be_i32);

impl_number_deserialize!(i64, nom::number::complete::be_i64);

// Floats

impl_number_serialize!(f32);

impl_number_serialize!(f64);

impl_number_deserialize!(f32, nom::number::complete::be_f32);

impl_number_deserialize!(f64, nom::number::complete::be_f64);

// Variable sized i32s

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize, serde::Serialize)]
#[repr(transparent)]
pub struct VarInt(pub i32);

#[cfg(not(feature = "protocol_verbose"))]
impl VarInt {
    pub const fn tag(value: i32) -> impl Fn(&[u8]) -> IResult<&[u8], Self> {
        move |input: &[u8]| {
            verify(Self::deserialize, |parsed_value| parsed_value.0 == value)(input).map_err(|_| {
                nom::Err::Error(IErr::from_error_kind(input, nom::error::ErrorKind::Tag))
            })
        }
    }
}

#[cfg(feature = "protocol_verbose")]
impl VarInt {
    pub const fn tag(value: i32) -> impl Fn(&[u8]) -> IResult<&[u8], Self> {
        move |input: &[u8]| {
            verify(Self::deserialize, |parsed_value| parsed_value.0 == value)(input).map_err(|_| {
                nom::Err::Error(ErrorTree::Base {
                    location: input,
                    kind: nom_supreme::error::BaseErrorKind::External(
                        format!("expected {value:X}").into(),
                    ),
                })
            })
        }
    }
}

impl std::fmt::Debug for VarInt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, f)
    }
}

impl Deserialize for VarInt {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        let (rest, slice) = recognize(pair(
            take_while(|byte| byte & 0x80 != 0),
            take(1usize).and_then(take_while1(|byte| byte & 0x80 == 0)),
        ))
        .context("VarInt")
        .parse(input)?;
        assert!(slice.len() <= 5);
        let mut value: i32 = 0;
        for (i, &byte) in slice.iter().enumerate() {
            value |= (byte as i32 & 0x7F) << (i * 7);
        }
        Ok((rest, Self(value)))
    }
}

impl Serialize for VarInt {
    fn serialize_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let mut value = self.0;
        loop {
            if value & !0x7F == 0 {
                writer.write_all(&[value as u8])?;
                return Ok(());
            }
            writer.write_all(&[(value as u8 & 0x7F) | 0x80])?;
            value = (value as u32 >> 7) as i32;
        }
    }
}

impl TryFrom<VarInt> for usize {
    type Error = <usize as TryFrom<i32>>::Error;

    fn try_from(value: VarInt) -> Result<Self, Self::Error> {
        usize::try_from(value.0)
    }
}

#[cfg(test)]
mod var_int_tests {
    use super::{Deserialize, Serialize, VarInt};
    use std::io::Cursor;

    #[test]
    fn serialize_wiki_vg_samples() {
        macro_rules! test_case {
            ($value:expr, $expected_bytes:expr) => {{
                let mut buffer = Cursor::new(Vec::new());
                VarInt($value).serialize_into(&mut buffer).unwrap();
                assert_eq!(buffer.get_ref(), $expected_bytes);
            }};
        }
        test_case!(0, &[0x00]);
        test_case!(1, &[0x01]);
        test_case!(127, &[0x7F]);
        test_case!(128, &[0x80, 0x01]);
        test_case!(255, &[0xFF, 0x01]);
        test_case!(25565, &[0xDD, 0xC7, 0x01]);
        test_case!(2097151, &[0xFF, 0xFF, 0x7F]);
        test_case!(2147483647, &[0xFF, 0xFF, 0xFF, 0xFF, 0x07]);
        test_case!(-1, &[0xFF, 0xFF, 0xFF, 0xFF, 0x0F]);
        test_case!(-2147483648, &[0x80, 0x80, 0x80, 0x80, 0x08]);
    }

    #[test]
    fn deserialize_wiki_vg_samples() {
        macro_rules! test_case {
            ($bytes:expr, $expected_value:expr) => {
                assert_eq!(
                    VarInt::deserialize($bytes).unwrap(),
                    (&[] as &[u8], VarInt($expected_value))
                );
            };
        }
        test_case!(&[0x00], 0);
        test_case!(&[0x01], 1);
        test_case!(&[0x7F], 127);
        test_case!(&[0x80, 0x01], 128);
        test_case!(&[0xFF, 0x01], 255);
        test_case!(&[0xDD, 0xC7, 0x01], 25565);
        test_case!(&[0xFF, 0xFF, 0x7F], 2097151);
        test_case!(&[0xFF, 0xFF, 0xFF, 0xFF, 0x07], 2147483647);
        test_case!(&[0xFF, 0xFF, 0xFF, 0xFF, 0x0F], -1);
        test_case!(&[0x80, 0x80, 0x80, 0x80, 0x08], -2147483648);
    }
}

// Variable sized i64s

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct VarLong(pub i64);

#[cfg(not(feature = "protocol_verbose"))]
impl VarLong {
    pub const fn tag(value: i64) -> impl Fn(&[u8]) -> IResult<&[u8], Self> {
        move |input: &[u8]| {
            verify(Self::deserialize, |parsed_value| parsed_value.0 == value)(input).map_err(|_| {
                nom::Err::Error(IErr::from_error_kind(input, nom::error::ErrorKind::Tag))
            })
        }
    }
}

#[cfg(feature = "protocol_verbose")]
impl VarLong {
    pub const fn tag(value: i64) -> impl Fn(&[u8]) -> IResult<&[u8], Self> {
        move |input: &[u8]| {
            verify(Self::deserialize, |parsed_value| parsed_value.0 == value)(input).map_err(|_| {
                nom::Err::Error(ErrorTree::Base {
                    location: input,
                    kind: nom_supreme::error::BaseErrorKind::External(
                        format!("expected {value:X}").into(),
                    ),
                })
            })
        }
    }
}

impl std::fmt::Debug for VarLong {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, f)
    }
}

impl Deserialize for VarLong {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        let (rest, slice) = recognize(pair(
            take_while(|byte| byte & 0x80 != 0),
            take(1usize).and_then(take_while1(|byte| byte & 0x80 == 0)),
        ))
        .context("VarLong")
        .parse(input)?;
        assert!(slice.len() <= 10);
        let mut value: i64 = 0;
        for (i, &byte) in slice.iter().enumerate() {
            value |= (byte as i64 & 0x7F) << (i * 7);
        }
        Ok((rest, Self(value)))
    }
}

impl Serialize for VarLong {
    fn serialize_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let mut value = self.0;
        loop {
            if value & !0x7F == 0 {
                writer.write_all(&[value as u8])?;
                return Ok(());
            }
            writer.write_all(&[(value as u8 & 0x7F) | 0x80])?;
            value = (value as u64 >> 7) as i64;
        }
    }
}

impl TryFrom<VarLong> for usize {
    type Error = <usize as TryFrom<i64>>::Error;

    fn try_from(value: VarLong) -> Result<Self, Self::Error> {
        usize::try_from(value.0)
    }
}

#[cfg(test)]
mod var_long_tests {
    use super::{Deserialize, Serialize, VarLong};
    use std::io::Cursor;

    #[test]
    fn serialize_wiki_vg_samples() {
        macro_rules! test_case {
            ($value:expr, $expected_bytes:expr) => {{
                let mut buffer = Cursor::new(Vec::new());
                VarLong($value).serialize_into(&mut buffer).unwrap();
                assert_eq!(buffer.get_ref(), $expected_bytes);
            }};
        }
        test_case!(0, &[0x00]);
        test_case!(1, &[0x01]);
        test_case!(127, &[0x7F]);
        test_case!(128, &[0x80, 0x01]);
        test_case!(255, &[0xFF, 0x01]);
        test_case!(25565, &[0xDD, 0xC7, 0x01]);
        test_case!(2097151, &[0xFF, 0xFF, 0x7F]);
        test_case!(2147483647, &[0xFF, 0xFF, 0xFF, 0xFF, 0x07]);
        test_case!(
            9223372036854775807,
            &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F]
        );
        test_case!(
            -1,
            &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01]
        );
        test_case!(
            -2147483648,
            &[0x80, 0x80, 0x80, 0x80, 0xF8, 0xFF, 0xFF, 0xFF, 0xFF, 0x01]
        );
        test_case!(
            -9223372036854775808,
            &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01]
        );
    }

    #[test]
    fn deserialize_wiki_vg_samples() {
        macro_rules! test_case {
            ($bytes:expr, $expected_value:expr) => {
                assert_eq!(
                    VarLong::deserialize($bytes).unwrap(),
                    (&[] as &[u8], VarLong($expected_value))
                );
            };
        }
        test_case!(&[0x00], 0);
        test_case!(&[0x01], 1);
        test_case!(&[0x7F], 127);
        test_case!(&[0x80, 0x01], 128);
        test_case!(&[0xFF, 0x01], 255);
        test_case!(&[0xDD, 0xC7, 0x01], 25565);
        test_case!(&[0xFF, 0xFF, 0x7F], 2097151);
        test_case!(&[0xFF, 0xFF, 0xFF, 0xFF, 0x07], 2147483647);
        test_case!(
            &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F],
            9223372036854775807
        );
        test_case!(
            &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01],
            -1
        );
        test_case!(
            &[0x80, 0x80, 0x80, 0x80, 0xF8, 0xFF, 0xFF, 0xFF, 0xFF, 0x01],
            -2147483648
        );
        test_case!(
            &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01],
            -9223372036854775808
        );
    }
}

// Strings

impl Serialize for &str {
    fn serialize_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        // Length
        VarInt(
            self.len()
                .try_into()
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?,
        )
        .serialize_into(writer)?;
        // Data
        writer.write_all(self.as_bytes())
    }
}

impl Deserialize for String {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        // TODO The `Verify` error's fine, but there's probably a way of passing along a more
        // descriptive error for invalid UTF-8
        length_value(
            verify(VarInt::deserialize, |len| len.0 >= 0).map(|len| len.0 as usize),
            |slice| {
                std::str::from_utf8(slice)
                    .map(|str| (slice, str.to_string()))
                    .map_err(|_| {
                        nom::Err::Error(IErr::from_error_kind(slice, nom::error::ErrorKind::Verify))
                    })
            },
        )
        .context("String")
        .parse(input)
    }
}

#[cfg(test)]
mod string_tests {
    use super::Deserialize;

    #[test]
    fn deserialize() {
        macro_rules! test_case {
            ($bytes:expr, $expected_value:expr) => {
                assert_eq!(
                    String::deserialize($bytes).unwrap(),
                    (&[] as &[u8], String::from($expected_value))
                );
            };
        }
        test_case!(&[4, b't', b'e', b's', b't'], "test");
    }
}

// String derivative types

// TODO Make newtype, add methods for separating namespace from id
pub type Identifier = String;

pub type Chat = String;

// UUIDs

impl Deserialize for Uuid {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        take(16usize)
            .map(|slice| Uuid::from_slice(slice).unwrap())
            .context("Uuid")
            .parse(input)
    }
}

impl Serialize for Uuid {
    fn serialize_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(self.as_bytes())
    }
}

// Tuples, up to 4 elements currently. Just encoded as each field coming consecutively.

impl<A: Deserialize, B: Deserialize> Deserialize for (A, B) {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        pair(A::deserialize, B::deserialize)(input)
    }
}

impl<A: Deserialize, B: Deserialize, C: Deserialize> Deserialize for (A, B, C) {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        tuple((A::deserialize, B::deserialize, C::deserialize))(input)
    }
}

impl<A: Deserialize, B: Deserialize, C: Deserialize, D: Deserialize> Deserialize for (A, B, C, D) {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        tuple((
            A::deserialize,
            B::deserialize,
            C::deserialize,
            D::deserialize,
        ))(input)
    }
}

impl<A: Deserialize, B: Deserialize, C: Deserialize, D: Deserialize, E: Deserialize> Deserialize
    for (A, B, C, D, E)
{
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        tuple((
            A::deserialize,
            B::deserialize,
            C::deserialize,
            D::deserialize,
            E::deserialize,
        ))(input)
    }
}

// Optionals, uses a bool prefix for whether field exists. There are a few exceptions to this
// format, but it's common enough to be useful here.

impl<T: Deserialize> Deserialize for Option<T> {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        let (rest, is_some) = bool::deserialize(input)?;
        match is_some {
            false => Ok((rest, None)),
            true => T::deserialize
                .context("Option")
                .map(|x| Some(x))
                .parse(rest),
        }
    }
}

impl<T: Serialize> Serialize for Option<T> {
    fn serialize_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        match self {
            None => false.serialize_to(writer),
            Some(x) => {
                true.serialize_to(writer)?;
                x.serialize_to(writer)
            }
        }
    }

    fn serialize_into<W: std::io::Write>(self, writer: &mut W) -> std::io::Result<()> {
        match self {
            None => false.serialize_to(writer),
            Some(x) => {
                true.serialize_to(writer)?;
                x.serialize_into(writer)
            }
        }
    }
}

// Vectors, uses a VarInt for number of entries. Same as with optionals, there are a few exceptions
// but this is a very common pattern.

impl<T: Deserialize> Deserialize for Vec<T> {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        length_count(
            verify(VarInt::deserialize, |len| len.0 >= 0)
                .map(|len| len.0 as usize)
                .context("VecLength"),
            T::deserialize,
        )
        .context("Vec")
        .parse(input)
    }
}

macro_rules! var_int_tagged_parser {
    ($( $tag_value:expr => $deserializer:expr $(,)? )+) => {{
        nom::branch::alt((
            $( nom::sequence::preceded(VarInt::tag($tag_value), $deserializer), )+
        ))
    }}
}

// Slice serialization

impl<T: Serialize> Serialize for &[T] {
    fn serialize_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        for element in *self {
            element.serialize_to(writer)?;
        }
        Ok(())
    }
}

// Array serialization

impl<T: Serialize, const N: usize> Serialize for [T; N] {
    fn serialize_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        for element in self {
            element.serialize_to(writer)?;
        }
        Ok(())
    }

    fn serialize_into<W: std::io::Write>(self, writer: &mut W) -> std::io::Result<()> {
        for element in self {
            element.serialize_into(writer)?;
        }
        Ok(())
    }
}

// Reference serialization

impl<T: Serialize> Serialize for &T {
    fn serialize_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        (*self).serialize_to(writer)
    }
}

// Position

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position(i32, i32, i32);

impl Deserialize for Position {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        i64::deserialize
            .map(|value| {
                Position(
                    (value >> 38) as i32,
                    (value << 52 >> 52) as i32,
                    (value << 26 >> 38) as i32,
                )
            })
            .context("Position")
            .parse(input)
    }
}

#[cfg(test)]
mod position_tests {
    use super::{Deserialize, Position};

    #[test]
    fn deserialize() {
        macro_rules! test_case {
            ($raw_value:expr, $expected_value:expr) => {
                assert_eq!(
                    Position::deserialize(&u64::to_be_bytes($raw_value)).unwrap(),
                    (&[] as &[u8], $expected_value)
                );
            };
        }
        test_case!(0x4607632C15B4833F, Position(18357644, 831, -20882616));
    }
}

// NBT

impl Deserialize for NbtCompound {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        fn parser(input: &[u8]) -> IResult<&[u8], NbtCompound> {
            let mut input_cursor = Cursor::new(input);
            let (nbt, name) =
                quartz_nbt::io::read_nbt(&mut input_cursor, quartz_nbt::io::Flavor::Uncompressed)
                    .map_err(|_| {
                    nom::Err::Error(IErr::from_error_kind(input, nom::error::ErrorKind::Verify))
                })?;
            debug_assert_eq!(&name, "");
            let rest = &input[input_cursor.position() as usize..];
            Ok((rest, nbt))
        }
        parser.context("NbtCompound").parse(input)
    }
}

#[derive(Clone, Debug)]
#[repr(transparent)]
pub struct OptionalNbt(pub Option<NbtCompound>);

impl Deserialize for OptionalNbt {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        if input.len() >= 1 && input[0] == 0 {
            Ok((&input[1..], Self(None)))
        } else {
            NbtCompound::deserialize
                .map(|nbt| Self(Some(nbt)))
                .context("OptionalNbt")
                .parse(input)
        }
    }
}

// BitVec

pub type BitVec = Vec<u64>;

// Angles, represented as steps of 1/256 of a full turn

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[repr(transparent)]
pub struct Angle(u8);

// Direction on wiki.vg

pub use crate::basic_types::AxisDirection;

impl Deserialize for AxisDirection {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        var_int_tagged_parser!(
            0 => success(Self::Down),
            1 => success(Self::Up),
            2 => success(Self::North),
            3 => success(Self::South),
            4 => success(Self::West),
            5 => success(Self::East),
        )
        .context("AxisDirection")
        .parse(input)
    }
}

// Particle

// TODO: Can contain data or be parsed from a string, so convert to new type
pub type Particle = VarInt;

// GlobalPosition, represents an interdimensional location

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct GlobalPosition {
    pub dimension: Identifier,
    pub pos: Position,
}

// EntityId and OptionalEntityId, these uniquely identify entities in the world

pub type EntityId = VarInt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OptionalEntityId(pub Option<EntityId>);

impl Deserialize for OptionalEntityId {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        EntityId::deserialize
            .context("OptionalEntityId")
            .map(|maybe_id| match maybe_id.0 {
                0 => Self(None),
                id => Self(Some(VarInt(id - 1))),
            })
            .parse(input)
    }
}
