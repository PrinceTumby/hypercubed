use super::prelude::*;
use ahash::AHashMap;
use nom::bytes::complete::{take, take_while, take_while1};
use nom::combinator::{recognize, success, verify};
use nom::error::ParseError;
use nom::multi::{length_count, length_value};
use nom::sequence::{pair, tuple};
use nom::{Parser, Slice};
use nom_supreme::parser_ext::ParserExt;
use protocol_derive::Deserialize;
use std::collections::HashMap;
use std::io::Cursor;
use std::mem::MaybeUninit;
use std::num::NonZeroU32;

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

macro_rules! var_int_tagged_parser {
    ($( $tag_value:expr => $deserializer:expr $(,)? )+) => {{
        nom::branch::alt((
            $(
                nom::sequence::preceded(
                    VarInt::tag($tag_value),
                    nom::combinator::cut($deserializer),
                ),
            )+
        ))
    }}
}

// Unit

impl Deserialize for () {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        Ok((input, ()))
    }
}

// Boolean

impl Deserialize for bool {
    fn deserialize(input: InputSpan) -> IResult<Self> {
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
    fn deserialize(input: InputSpan) -> IResult<Self> {
        take(1usize)
            .map(|slice: InputSpan| slice[0])
            .context("u8")
            .parse(input)
    }
}

impl Deserialize for i8 {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        take(1usize)
            .map(|slice: InputSpan| slice[0] as i8)
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
            fn deserialize(input: InputSpan) -> IResult<Self> {
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
    pub const fn tag<'a>(value: i32) -> impl Parser<InputSpan<'a>, Self, IErr<'a>> {
        move |input: InputSpan<'a>| {
            verify(Self::deserialize, |parsed_value| parsed_value.0 == value)(input).map_err(|_| {
                nom::Err::Error(IErr::from_error_kind(input, nom::error::ErrorKind::Tag))
            })
        }
    }
}

#[cfg(feature = "protocol_verbose")]
impl VarInt {
    pub const fn tag<'a>(value: i32) -> impl Parser<InputSpan<'a>, Self, IErr<'a>> {
        move |input: InputSpan<'a>| {
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
    fn deserialize(input: InputSpan) -> IResult<Self> {
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

// FIXME: All the tests need to be fixed to work with LocatedSpan, currently they assume &[u8].
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
                    (InputSpan::new(&[]), VarInt($expected_value))
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
    pub const fn tag<'a>(value: i64) -> impl Parser<InputSpan<'a>, Self, IErr<'a>> {
        move |input: InputSpan<'a>| {
            verify(Self::deserialize, |parsed_value| parsed_value.0 == value)(input).map_err(|_| {
                nom::Err::Error(IErr::from_error_kind(input, nom::error::ErrorKind::Tag))
            })
        }
    }
}

#[cfg(feature = "protocol_verbose")]
impl VarLong {
    pub const fn tag<'a>(value: i64) -> impl Parser<InputSpan<'a>, Self, IErr<'a>> {
        move |input: InputSpan<'a>| {
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
    fn deserialize(input: InputSpan) -> IResult<Self> {
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
                    (InputSpan::new(&[]), VarLong($expected_value))
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
    fn deserialize<'a>(input: InputSpan<'a>) -> IResult<Self> {
        // TODO: The `Verify` error's fine, but there's probably a way of passing along a more
        // descriptive error for invalid UTF-8
        length_value(
            verify(VarInt::deserialize, |len| len.0 >= 0).map(|len| len.0 as usize),
            |slice: InputSpan<'a>| {
                std::str::from_utf8(slice.as_ref())
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

impl Serialize for String {
    fn serialize_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        self.as_str().serialize_to(writer)
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
                    (InputSpan::new(&[]), String::from($expected_value))
                );
            };
        }
        test_case!(&[4, b't', b'e', b's', b't'], "test");
    }
}

// String derivative types

// Identifiers

pub use crate::resource::Identifier;

impl Deserialize for Identifier {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        String::deserialize
            .context("Identifier")
            .parse(input)
            .and_then(|(rest, string)| {
                let identifier = Identifier::parse(&string).map_err(|err| {
                    nom::Err::Error(nom::error::FromExternalError::from_external_error(
                        input,
                        nom::error::ErrorKind::Verify,
                        err,
                    ))
                });
                Ok((rest, identifier?))
            })
    }
}

impl Serialize for Identifier {
    fn serialize_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let namespace_len = self.namespace.len() + 1;
        let path_name_len = self.path_name.len();
        let path_prefixes_len = self
            .path_prefix_segments
            .iter()
            .fold(0, |acc, seg| acc + seg.len() + 1);
        let total_len = namespace_len + path_prefixes_len + path_name_len;
        VarInt(total_len.try_into().unwrap()).serialize_into(writer)?;
        write!(writer, "{}:", &self.namespace)?;
        for path_prefix_segment in &self.path_prefix_segments {
            write!(writer, "{}/", path_prefix_segment)?;
        }
        write!(writer, "{}", &self.path_name)?;
        Ok(())
    }
}

#[cfg(test)]
mod identifier_tests {
    use super::super::ByteView;
    use super::Serialize;
    use crate::identifier;
    use std::io::Cursor;

    #[test]
    fn serialize() {
        macro_rules! test_case {
            ($value:expr, $expected_bytes:expr) => {{
                let mut buffer = Cursor::new(Vec::new());
                $value.serialize_into(&mut buffer).unwrap();
                let left_byte_view = ByteView(buffer.get_ref());
                let right_byte_view = ByteView($expected_bytes);
                assert_eq!(
                    left_byte_view, right_byte_view,
                    "\n  left: {left_byte_view}\n right: {right_byte_view}"
                );
            }};
        }
        test_case!(identifier!("minecraft:brand"), b"\x0Fminecraft:brand");
    }
}

// UUIDs

pub use uuid::Uuid;

impl Deserialize for Uuid {
    fn deserialize<'a>(input: InputSpan<'a>) -> IResult<Self> {
        take(16usize)
            .map(|slice: InputSpan<'a>| Uuid::from_slice(slice.as_ref()).unwrap())
            .context("Uuid")
            .parse(input)
    }
}

impl Serialize for Uuid {
    fn serialize_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(self.as_bytes())
    }
}

// Tuples. Just encoded as each field deserialised consecutively.

macro_rules! impl_tuple_deserialize {
    ( $( $type_param:ident ),+ ) => {
        impl<$( $type_param: Deserialize ),+> Deserialize for ( $( $type_param, )+ ) {
            fn deserialize(input: InputSpan) -> IResult<Self> {
                tuple((
                    $( $type_param::deserialize, )+
                ))(input)
            }
        }
    };
}

impl_tuple_deserialize!(A);
impl_tuple_deserialize!(A, B);
impl_tuple_deserialize!(A, B, C);
impl_tuple_deserialize!(A, B, C, D);
impl_tuple_deserialize!(A, B, C, D, E);
impl_tuple_deserialize!(A, B, C, D, E, F);
impl_tuple_deserialize!(A, B, C, D, E, F, G);
impl_tuple_deserialize!(A, B, C, D, E, F, G, H);
impl_tuple_deserialize!(A, B, C, D, E, F, G, H, I);
impl_tuple_deserialize!(A, B, C, D, E, F, G, H, I, J);
impl_tuple_deserialize!(A, B, C, D, E, F, G, H, I, J, K);
impl_tuple_deserialize!(A, B, C, D, E, F, G, H, I, J, K, L);
impl_tuple_deserialize!(A, B, C, D, E, F, G, H, I, J, K, L, M);
impl_tuple_deserialize!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
impl_tuple_deserialize!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
impl_tuple_deserialize!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);
impl_tuple_deserialize!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q);
impl_tuple_deserialize!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R);
impl_tuple_deserialize!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S);
impl_tuple_deserialize!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T);

// Optionals, uses a bool prefix for whether field exists. There are a few exceptions to this
// format, but it's common enough to be useful here.

impl<T: Deserialize> Deserialize for Option<T> {
    fn deserialize(input: InputSpan) -> IResult<Self> {
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

// Box, just deserializes as the inner value.

impl<T: Deserialize> Deserialize for Box<T> {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        T::deserialize
            .map(|v| Box::new(v))
            .context(std::any::type_name::<Self>())
            .parse(input)
    }
}

// Vectors, uses a VarInt for number of entries. Same as with optionals, there are a few exceptions
// but this is a very common pattern.

impl<T: Deserialize> Deserialize for Vec<T> {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        length_count(
            verify(VarInt::deserialize.context("VecLength"), |len| len.0 >= 0)
                .map(|len| len.0 as usize)
                .context("VecLength"),
            T::deserialize.context("VecElement"),
        )
        .context(std::any::type_name::<Self>())
        .parse(input)
    }
}

// HashMaps, encoded as a Vec<(Key, Value)>.

impl<K, V, S> Deserialize for HashMap<K, V, S>
where
    K: Deserialize + std::hash::Hash + Eq,
    V: Deserialize,
    S: std::hash::BuildHasher + Default,
{
    fn deserialize(input: InputSpan) -> IResult<Self> {
        <Vec<(K, V)>>::deserialize
            .map(|entries| entries.into_iter().collect())
            .context(std::any::type_name::<Self>())
            .parse(input)
    }
}

impl<K, V> Deserialize for AHashMap<K, V>
where
    K: Deserialize + std::hash::Hash + Eq,
    V: Deserialize,
{
    fn deserialize(input: InputSpan) -> IResult<Self> {
        <Vec<(K, V)>>::deserialize
            .map(|entries| entries.into_iter().collect())
            .context(std::any::type_name::<Self>())
            .parse(input)
    }
}

// Slice serialization

impl<T: Serialize> Serialize for &[T] {
    fn serialize_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        // Length
        VarInt(
            self.len()
                .try_into()
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?,
        )
        .serialize_into(writer)?;
        // Data
        for element in *self {
            element.serialize_to(writer)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProtocolRawSlice<'a, T: Serialize>(pub &'a [T]);

impl<T: Serialize> Serialize for ProtocolRawSlice<'_, T> {
    fn serialize_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        for element in self.0 {
            element.serialize_to(writer)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtocolRawBytes(pub Vec<u8>);

impl Deserialize for ProtocolRawBytes {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        Ok((input.slice(input.len()..), Self(input.as_ref().to_owned())))
    }
}

// Arrays

impl<T: Deserialize, const N: usize> Deserialize for [T; N] {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        let mut items = [const { MaybeUninit::uninit() }; N];
        let mut rest = input;
        for i in 0..items.len() {
            match T::deserialize(rest) {
                Ok((new_rest, value)) => {
                    items[i] = MaybeUninit::new(value);
                    rest = new_rest;
                }
                Err(err) => unsafe {
                    // SAFETY: We're only dropping array elements up to `i`, which have already
                    // been initialised.
                    for item in items.iter_mut().take(i) {
                        item.assume_init_drop();
                    }
                    return Err(err);
                },
            }
        }
        // SAFETY: All array elements have been initialised by this point.
        unsafe { Ok((rest, items.map(|value| value.assume_init()))) }
    }
}

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
pub struct Position {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Deserialize for Position {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        i64::deserialize
            .map(|value| Position {
                x: (value >> 38) as i32,
                y: (value << 52 >> 52) as i32,
                z: (value << 26 >> 38) as i32,
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
                    (InputSpan::new(&[]), $expected_value)
                );
            };
        }
        test_case!(
            0x4607632C15B4833F,
            Position {
                x: 18357644,
                y: 831,
                z: -20882616
            }
        );
    }
}

// NBT

pub use quartz_nbt::NbtCompound;

impl Deserialize for NbtCompound {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        fn parser(input: InputSpan) -> IResult<NbtCompound> {
            let mut input_cursor = Cursor::new(input);
            let (nbt, name) =
                quartz_nbt::io::read_nbt(&mut input_cursor, quartz_nbt::io::Flavor::Uncompressed)
                    .map_err(|_| {
                    nom::Err::Error(IErr::from_error_kind(input, nom::error::ErrorKind::Verify))
                })?;
            debug_assert_eq!(&name, "");
            let rest = input.slice(input_cursor.position() as usize..);
            Ok((rest, nbt))
        }
        parser.context("NbtCompound").parse(input)
    }
}

// HACK: Protocol version 764 removed the empty name from the root for network NBT compounds, so we
// add it back in here for quartz_nbt to deserialize as before.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[repr(transparent)]
pub struct NetworkNbtCompound(pub NbtCompound);

impl Deserialize for NetworkNbtCompound {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        fn parser(input: InputSpan) -> IResult<NbtCompound> {
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
            let rest = input.slice(input_cursor.position() as usize - 2..);
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
    fn deserialize(input: InputSpan) -> IResult<Self> {
        if input.len() >= 1 && input[0] == 0 {
            Ok((input.slice(1..), Self(None)))
        } else {
            NetworkNbtCompound::deserialize
                .map(|net_nbt| Self(Some(net_nbt.0)))
                .context("OptionalNbt")
                .parse(input)
        }
    }
}

// BitSet

pub use fixedbitset::FixedBitSet as BitSet;

impl Deserialize for BitSet {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        use smallvec::SmallVec;
        <Vec<u64>>::deserialize
            .map(|longs| {
                Self::with_capacity_and_blocks(
                    longs.len() * 64,
                    longs.into_iter().flat_map(|long| match usize::BITS {
                        64 => SmallVec::from_buf_and_len([long as usize, 0], 1),
                        32 => SmallVec::from_buf_and_len([long as usize, (long >> 32) as usize], 2),
                        _ => todo!(),
                    }),
                )
            })
            .context("BitSet")
            .parse(input)
    }
}

// Angles, represented as steps of 1/256 of a full turn

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[repr(transparent)]
pub struct Angle(u8);

// Direction on wiki.vg

pub use crate::basic_types::AxisDirection;

impl Deserialize for AxisDirection {
    fn deserialize(input: InputSpan) -> IResult<Self> {
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

// GlobalPosition, represents an interdimensional location

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct GlobalPosition {
    pub dimension: Identifier,
    pub pos: Position,
}

// EntityId and OptionalEntityId, these uniquely identify entities in the world

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct EntityId(pub u32);

impl EntityId {
    pub const fn placeholder() -> Self {
        Self(u32::MAX)
    }
}

impl Deserialize for EntityId {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        VarInt::deserialize
            .verify(|&VarInt(id)| id >= 0)
            .map(|VarInt(id)| Self(id as u32))
            .context("EntityId")
            .parse(input)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct OptionalEntityId(Option<NonZeroU32>);

impl OptionalEntityId {
    pub fn get(&self) -> Option<EntityId> {
        self.0.map(|x| EntityId(x.get() - 1))
    }
}

impl Deserialize for OptionalEntityId {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        EntityId::deserialize
            .map(|raw| Self(NonZeroU32::new(raw.0)))
            .context("OptionalEntityId")
            .parse(input)
    }
}

// TODO: Implement Display
#[derive(Clone, Debug, PartialEq)]
pub enum TextComponent {
    Basic(String),
    Compound(NetworkNbtCompound),
}

impl Deserialize for TextComponent {
    fn deserialize<'a>(input: InputSpan<'a>) -> IResult<Self> {
        if input.len() < 1 {
            Err(nom::Err::Error(IErr::from_error_kind(
                input,
                nom::error::ErrorKind::Verify,
            )))
        } else {
            match input[0] {
                // TODO: The `Verify` error's fine, but there's probably a way of passing along a
                // more descriptive error for invalid UTF-8
                8 => length_value(
                    u16::deserialize.map(|len| len as usize),
                    |slice: InputSpan<'a>| {
                        std::str::from_utf8(slice.as_ref())
                            .map(|str| (slice, str.to_string()))
                            .map_err(|_| {
                                nom::Err::Error(IErr::from_error_kind(
                                    slice,
                                    nom::error::ErrorKind::Verify,
                                ))
                            })
                    },
                )
                .map(Self::Basic)
                .context("TextComponent")
                .parse(input.slice(1..)),
                10 => NetworkNbtCompound::deserialize
                    .map(Self::Compound)
                    .context("TextComponent")
                    .parse(input),
                _ => Err(nom::Err::Error(IErr::from_error_kind(
                    input,
                    nom::error::ErrorKind::Verify,
                ))),
            }
        }
    }
}
