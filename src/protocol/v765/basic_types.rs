pub use crate::protocol::v763::basic_types::{
    Angle, AxisDirection, BitVec, EntityId, GlobalPosition, Identifier, OptionalEntityId, Particle,
    Position, VarInt, VarLong,
};

use super::prelude::*;
use nom::error::ParseError;
use nom::multi::length_value;
use nom::Parser;
use nom_supreme::parser_ext::ParserExt;
use quartz_nbt::NbtCompound;
use std::io::Cursor;

// HACK: Protocol version 764 removed the empty name from the root for network NBT compounds, so we
// add it back in here for quartz_nbt to deserialize as before

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[repr(transparent)]
pub struct NetworkNbtCompound(pub NbtCompound);

impl Deserialize for NetworkNbtCompound {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        fn parser(input: &[u8]) -> IResult<&[u8], NbtCompound> {
            if input.len() < 2 {
                return Err(nom::Err::Error(IErr::from_error_kind(
                    input,
                    nom::error::ErrorKind::Verify,
                )));
            }
            let mut modified_input = Vec::with_capacity(input.len() + 2);
            modified_input.extend_from_slice(&[input[0], 0x00, 0x00]);
            modified_input.extend_from_slice(&input[1..]);
            let mut input_cursor = Cursor::new(&modified_input);
            let (nbt, name) =
                quartz_nbt::io::read_nbt(&mut input_cursor, quartz_nbt::io::Flavor::Uncompressed)
                    .map_err(|_| {
                    nom::Err::Error(IErr::from_error_kind(input, nom::error::ErrorKind::Verify))
                })?;
            debug_assert_eq!(&name, "");
            let rest = &input[input_cursor.position() as usize - 2..];
            Ok((rest, nbt))
        }
        parser
            .map(NetworkNbtCompound)
            .context("NbtCompound")
            .parse(input)
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[repr(transparent)]
pub struct OptionalNbt(pub Option<NbtCompound>);

impl Deserialize for OptionalNbt {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        if input.len() >= 1 && input[0] == 0 {
            Ok((&input[1..], Self(None)))
        } else {
            NetworkNbtCompound::deserialize
                .map(|net_nbt| Self(Some(net_nbt.0)))
                .context("OptionalNbt")
                .parse(input)
        }
    }
}

// TODO: Implement Display
#[derive(Clone, Debug, PartialEq)]
pub enum Chat {
    Basic(String),
    Compound(NetworkNbtCompound),
}

impl Deserialize for Chat {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        if input.len() < 1 {
            Err(nom::Err::Error(IErr::from_error_kind(
                input,
                nom::error::ErrorKind::Verify,
            )))
        } else {
            match input[0] {
                // TODO: The `Verify` error's fine, but there's probably a way of passing along a
                // more descriptive error for invalid UTF-8
                8 => length_value(u16::deserialize.map(|len| len as usize), |slice| {
                    std::str::from_utf8(slice)
                        .map(|str| (slice, str.to_string()))
                        .map_err(|_| {
                            nom::Err::Error(IErr::from_error_kind(
                                slice,
                                nom::error::ErrorKind::Verify,
                            ))
                        })
                })
                .map(Self::Basic)
                .context("Chat")
                .parse(&input[1..]),
                10 => NetworkNbtCompound::deserialize
                    .map(Self::Compound)
                    .context("Chat")
                    .parse(input),
                _ => Err(nom::Err::Error(IErr::from_error_kind(
                    input,
                    nom::error::ErrorKind::Verify,
                ))),
            }
        }
    }
}

// TODO: Figure out a way to re-export macros from another module so we don't have to do this

macro_rules! byte_enum_parser {
    ($( $tag_byte:expr => $value:expr $(,)? )+) => {{
        nom::branch::alt((
            $(
                nom::combinator::value($value, nom_supreme::tag::complete::tag(&[$tag_byte][..])),
            )+
        ))
    }}
}

macro_rules! var_int_tagged_parser {
    ($( $tag_value:expr => $deserializer:expr $(,)? )+) => {{
        nom::branch::alt((
            $( nom::sequence::preceded(VarInt::tag($tag_value), $deserializer), )+
        ))
    }}
}
