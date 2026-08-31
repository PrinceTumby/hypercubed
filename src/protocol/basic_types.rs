use core::mem::MaybeUninit;
use core::num::NonZeroU32;

use nom::bytes::complete::{take, take_while, take_while1};
use nom::combinator::{recognize, success, verify};
use nom::error::ParseError;
use nom::multi::{length_count, length_data, length_value};
use nom::sequence::pair;
use nom::{Input, Parser};
#[cfg_attr(not(feature = "mini_std"), expect(unused))]
use portable_std::{FastHashMap, HashMap, io};
use protocol_derive::Deserialize;
use resources::RegistryIndex;

use super::prelude::*;
use crate::portable_prelude::*;

// Helper macros

macro_rules! byte_enum_parser {
    ($( $tag_byte:expr => $value:expr $(,)? )+) => {{
        nom::branch::alt((
            $(
                nom::combinator::value($value, nom::bytes::tag(&[$tag_byte][..])),
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
        nom_context(
            "bool",
            byte_enum_parser!(
                0 => false,
                1 => true,
            ),
        )
        .parse(input)
    }
}

impl Serialize for bool {
    fn serialize_to<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&[*self as u8])
    }
}

// Integers

impl Deserialize for u8 {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        nom_context("u8", take(1usize))
            .map(|slice: InputSpan| slice[0])
            .parse(input)
    }
}

impl Deserialize for i8 {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        nom_context("i8", take(1usize))
            .map(|slice: InputSpan| slice[0] as i8)
            .parse(input)
    }
}

impl Serialize for u8 {
    fn serialize_to<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&[*self])
    }
}

impl Serialize for i8 {
    fn serialize_to<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&[*self as u8])
    }
}

macro_rules! impl_number_serialize {
    ($num_type:ty) => {
        impl Serialize for $num_type {
            fn serialize_to<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
                writer.write_all(&Self::to_be_bytes(*self))
            }
        }
    };
}

macro_rules! impl_number_deserialize {
    ($num_type:ty, $deserializer:expr) => {
        impl Deserialize for $num_type {
            fn deserialize(input: InputSpan) -> IResult<Self> {
                nom_context(stringify!($num_type), $deserializer).parse(input)
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
    pub const fn tag<'a>(
        value: i32,
    ) -> impl Parser<InputSpan<'a>, Output = Self, Error = IErr<'a>> {
        move |input: InputSpan<'a>| {
            verify(Self::deserialize, |parsed_value| parsed_value.0 == value)
                .parse(input)
                .map_err(|_| {
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

impl core::fmt::Debug for VarInt {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(&self.0, f)
    }
}

impl Deserialize for VarInt {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        let (rest, slice) = nom_context(
            "VarInt",
            recognize(pair(
                take_while(|byte| byte & 0x80 != 0),
                take(1usize).and_then(take_while1(|byte| byte & 0x80 == 0)),
            )),
        )
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
    fn serialize_to<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
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
    use super::{Deserialize, InputSpan, Serialize, VarInt};
    use io::Cursor;

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
        test_case!(InputSpan::new(&[0x00]), 0);
        test_case!(InputSpan::new(&[0x01]), 1);
        test_case!(InputSpan::new(&[0x7F]), 127);
        test_case!(InputSpan::new(&[0x80, 0x01]), 128);
        test_case!(InputSpan::new(&[0xFF, 0x01]), 255);
        test_case!(InputSpan::new(&[0xDD, 0xC7, 0x01]), 25565);
        test_case!(InputSpan::new(&[0xFF, 0xFF, 0x7F]), 2097151);
        test_case!(InputSpan::new(&[0xFF, 0xFF, 0xFF, 0xFF, 0x07]), 2147483647);
        test_case!(InputSpan::new(&[0xFF, 0xFF, 0xFF, 0xFF, 0x0F]), -1);
        test_case!(InputSpan::new(&[0x80, 0x80, 0x80, 0x80, 0x08]), -2147483648);
    }
}

// Variable sized i64s

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct VarLong(pub i64);

#[cfg(not(feature = "protocol_verbose"))]
impl VarLong {
    pub const fn tag<'a>(
        value: i64,
    ) -> impl Parser<InputSpan<'a>, Output = Self, Error = IErr<'a>> {
        move |input: InputSpan<'a>| {
            verify(Self::deserialize, |parsed_value| parsed_value.0 == value)
                .parse(input)
                .map_err(|_| {
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

impl core::fmt::Debug for VarLong {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(&self.0, f)
    }
}

impl Deserialize for VarLong {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        let (rest, slice) = nom_context(
            "VarLong",
            recognize(pair(
                take_while(|byte| byte & 0x80 != 0),
                take(1usize).and_then(take_while1(|byte| byte & 0x80 == 0)),
            )),
        )
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
    fn serialize_to<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
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
    use super::{Deserialize, InputSpan, Serialize, VarLong};
    use io::Cursor;

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
        test_case!(InputSpan::new(&[0x00]), 0);
        test_case!(InputSpan::new(&[0x01]), 1);
        test_case!(InputSpan::new(&[0x7F]), 127);
        test_case!(InputSpan::new(&[0x80, 0x01]), 128);
        test_case!(InputSpan::new(&[0xFF, 0x01]), 255);
        test_case!(InputSpan::new(&[0xDD, 0xC7, 0x01]), 25565);
        test_case!(InputSpan::new(&[0xFF, 0xFF, 0x7F]), 2097151);
        test_case!(InputSpan::new(&[0xFF, 0xFF, 0xFF, 0xFF, 0x07]), 2147483647);
        test_case!(
            InputSpan::new(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F]),
            9223372036854775807
        );
        test_case!(
            InputSpan::new(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01]),
            -1
        );
        test_case!(
            InputSpan::new(&[0x80, 0x80, 0x80, 0x80, 0xF8, 0xFF, 0xFF, 0xFF, 0xFF, 0x01]),
            -2147483648
        );
        test_case!(
            InputSpan::new(&[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01]),
            -9223372036854775808
        );
    }
}

// Strings

impl Serialize for &str {
    fn serialize_to<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        // Length
        VarInt(self.len().try_into().map_err(io::Error::other)?).serialize_into(writer)?;
        // Data
        writer.write_all(self.as_bytes())
    }
}

impl Deserialize for String {
    fn deserialize<'a>(input: InputSpan<'a>) -> IResult<'a, Self> {
        // TODO: The `Verify` error's fine, but there's probably a way of passing along a more
        // descriptive error for invalid UTF-8
        nom_context(
            "String",
            length_data(verify(VarInt::deserialize, |len| len.0 >= 0).map(|len| len.0 as usize))
                .map_res(|slice: InputSpan<'a>| core::str::from_utf8(&slice))
                .map(String::from),
        )
        .parse(input)
    }
}

impl Serialize for String {
    fn serialize_to<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        self.as_str().serialize_to(writer)
    }
}

#[cfg(test)]
mod string_tests {
    use super::{Deserialize, InputSpan};

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
        test_case!(InputSpan::new(&[4, b't', b'e', b's', b't']), "test");
    }
}

// String derivative types

// Identifiers

pub use resources::Identifier;

impl Deserialize for Identifier {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        nom_context("Identifier", String::deserialize)
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
    fn serialize_to<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        let namespace_len = self.get_namespace().len() + 1;
        let path_name_len = self.get_path_name().len();
        let path_prefixes_len = self
            .get_path_prefix_segments()
            .iter()
            .fold(0, |acc, seg| acc + seg.len() + 1);
        let total_len = namespace_len + path_prefixes_len + path_name_len;
        VarInt(total_len.try_into().unwrap()).serialize_into(writer)?;
        write!(writer, "{}:", self.get_namespace())?;
        for path_prefix_segment in self.get_path_prefix_segments() {
            write!(writer, "{}/", path_prefix_segment)?;
        }
        write!(writer, "{}", self.get_path_name())?;
        Ok(())
    }
}

#[cfg(test)]
mod identifier_tests {
    use super::super::ByteView;
    use super::{InputSpan, Serialize};
    use io::Cursor;
    use resources::identifier;

    #[test]
    fn serialize() {
        macro_rules! test_case {
            ($value:expr, $expected_bytes:expr) => {{
                let mut buffer = Cursor::new(Vec::new());
                $value.serialize_into(&mut buffer).unwrap();
                let left_byte_view = ByteView(InputSpan::new(buffer.get_ref()));
                let right_byte_view = ByteView(InputSpan::new($expected_bytes));
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
    fn deserialize<'a>(input: InputSpan<'a>) -> IResult<'a, Self> {
        nom_context("Uuid", take(16usize))
            .map(|slice: InputSpan<'a>| Uuid::from_slice(slice.as_ref()).unwrap())
            .parse(input)
    }
}

impl Serialize for Uuid {
    fn serialize_to<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(self.as_bytes())
    }
}

// Tuples. Just encoded as each field deserialised consecutively.

macro_rules! impl_tuple_deserialize {
    ( $( $type_param:ident ),+ ) => {
        impl<$( $type_param: Deserialize ),+> Deserialize for ( $( $type_param, )+ ) {
            fn deserialize(input: InputSpan) -> IResult<Self> {
                (
                    $( $type_param::deserialize, )+
                ).parse(input)
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
            true => nom_context("Option", T::deserialize)
                .map(|x| Some(x))
                .parse(rest),
        }
    }
}

impl<T: Serialize> Serialize for Option<T> {
    fn serialize_to<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            None => false.serialize_to(writer),
            Some(x) => {
                true.serialize_to(writer)?;
                x.serialize_to(writer)
            }
        }
    }

    fn serialize_into<W: io::Write>(self, writer: &mut W) -> io::Result<()> {
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
        nom_context(core::any::type_name::<Self>(), T::deserialize)
            .map(|v| Box::new(v))
            .parse(input)
    }
}

// Vectors, uses a VarInt for number of entries. Same as with optionals, there are a few exceptions
// but this is a very common pattern.

impl<T: Deserialize> Deserialize for Vec<T> {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        nom_context(
            core::any::type_name::<Self>(),
            length_count(
                verify(nom_context("VecLength", VarInt::deserialize), |len| {
                    len.0 >= 0
                })
                .map(|len| len.0 as usize),
                nom_context("VecElement", T::deserialize),
            ),
        )
        .parse(input)
    }
}

// HashMaps, encoded as a Vec<(Key, Value)>.

impl<K, V, S> Deserialize for HashMap<K, V, S>
where
    K: Deserialize + core::hash::Hash + Eq,
    V: Deserialize,
    S: core::hash::BuildHasher + Default,
{
    fn deserialize(input: InputSpan) -> IResult<Self> {
        nom_context(core::any::type_name::<Self>(), <Vec<(K, V)>>::deserialize)
            .map(|entries| entries.into_iter().collect())
            .parse(input)
    }
}

#[cfg(feature = "mini_std")]
impl<K, V> Deserialize for FastHashMap<K, V>
where
    K: Deserialize + core::hash::Hash + Eq,
    V: Deserialize,
{
    fn deserialize(input: InputSpan) -> IResult<Self> {
        nom_context(core::any::type_name::<Self>(), <Vec<(K, V)>>::deserialize)
            .map(|entries| entries.into_iter().collect())
            .parse(input)
    }
}

// Slice serialization

impl<T: Serialize> Serialize for &[T] {
    fn serialize_to<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        // Length
        VarInt(self.len().try_into().map_err(io::Error::other)?).serialize_into(writer)?;
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
    fn serialize_to<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
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
        Ok((input.take_from(input.len()), Self(input.to_vec())))
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
    fn serialize_to<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        for element in self {
            element.serialize_to(writer)?;
        }
        Ok(())
    }

    fn serialize_into<W: io::Write>(self, writer: &mut W) -> io::Result<()> {
        for element in self {
            element.serialize_into(writer)?;
        }
        Ok(())
    }
}

// Reference serialization

impl<T: Serialize> Serialize for &T {
    fn serialize_to<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
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
        nom_context("Position", i64::deserialize)
            .map(|value| Position {
                x: (value >> 38) as i32,
                y: (value << 52 >> 52) as i32,
                z: (value << 26 >> 38) as i32,
            })
            .parse(input)
    }
}

#[cfg(test)]
mod position_tests {
    use super::{Deserialize, InputSpan, Position};

    #[test]
    fn deserialize() {
        macro_rules! test_case {
            ($raw_value:expr, $expected_value:expr) => {
                assert_eq!(
                    Position::deserialize(InputSpan::new(&u64::to_be_bytes($raw_value))).unwrap(),
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

pub use crab_nbt::Nbt;

impl Deserialize for Nbt {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        struct NbtParser;

        impl<'a> Parser<InputSpan<'a>> for NbtParser {
            type Output = Nbt;
            type Error = IErr<'a>;

            fn process<OM: nom::OutputMode>(
                &mut self,
                input: InputSpan<'a>,
            ) -> nom::PResult<OM, InputSpan<'a>, Self::Output, Self::Error> {
                let mut current_slice = input.as_ref();
                let nbt = Nbt::read(&mut current_slice).unwrap();
                nom::bytes::take(input.len() - current_slice.len())
                    .map(|_| nbt.clone())
                    .process::<OM>(input)
            }
        }
        nom_context("Nbt", NbtParser).parse(input)
    }
}

// HACK: Protocol version 764 removed the empty name from the root for network NBT compounds, so we
// add it back in here for quartz_nbt to deserialize as before.
#[derive(Clone, Debug, PartialEq)]
#[repr(transparent)]
pub struct NetworkNbt(pub Nbt);

impl Deserialize for NetworkNbt {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        struct NetworkNbtParser;

        impl<'a> Parser<InputSpan<'a>> for NetworkNbtParser {
            type Output = NetworkNbt;
            type Error = IErr<'a>;

            fn process<OM: nom::OutputMode>(
                &mut self,
                input: InputSpan<'a>,
            ) -> nom::PResult<OM, InputSpan<'a>, Self::Output, Self::Error> {
                let mut current_slice = input.as_ref();
                let nbt = Nbt::read_unnamed(&mut current_slice).unwrap();
                nom::bytes::take(input.len() - current_slice.len())
                    .map(|_| NetworkNbt(nbt.clone()))
                    .process::<OM>(input)
            }
        }
        nom_context("NetworkNbt", NetworkNbtParser).parse(input)
    }
}

#[derive(Clone, Debug, PartialEq)]
#[repr(transparent)]
pub struct OptionalNbt(pub Option<Nbt>);

impl Deserialize for OptionalNbt {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        if input.len() >= 1 && input[0] == 0 {
            Ok((input.take_from(1), Self(None)))
        } else {
            nom_context("OptionalNbt", NetworkNbt::deserialize)
                .map(|net_nbt| Self(Some(net_nbt.0)))
                .parse(input)
        }
    }
}

// BitSet

pub use fixedbitset::FixedBitSet as BitSet;

impl Deserialize for BitSet {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        use smallvec::SmallVec;
        nom_context("BitSet", <Vec<u64>>::deserialize)
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
            .parse(input)
    }
}

// Angles, represented as steps of 1/256 of a full turn

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[repr(transparent)]
pub struct Angle(u8);

impl Angle {
    pub const fn degrees(&self) -> f32 {
        self.0 as f32 * (256.0 / 360.0)
    }
}

// Direction on wiki.vg

pub use crate::basic_types::AxisDirection;

impl Deserialize for AxisDirection {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        nom_context(
            "AxisDirection",
            var_int_tagged_parser!(
                0 => success(Self::Down),
                1 => success(Self::Up),
                2 => success(Self::North),
                3 => success(Self::South),
                4 => success(Self::West),
                5 => success(Self::East),
            ),
        )
        .parse(input)
    }
}

// GlobalPosition, represents an interdimensional location

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct GlobalPosition {
    pub dimension: Identifier,
    pub pos: Position,
}

// Registry indices

impl Deserialize for RegistryIndex {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        nom_context("RegistryIndex", VarInt::deserialize)
            .map_res(|idx| idx.0.try_into().map(Self))
            .parse(input)
    }
}

// EntityId and OptionalEntityId, these uniquely identify entities in the world

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct EntityId(pub u32);

impl EntityId {
    pub const fn placeholder() -> Self {
        Self(u32::MAX)
    }
}

impl Deserialize for EntityId {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        nom::combinator::verify(
            nom_context("EntityId", VarInt::deserialize),
            |&VarInt(id)| id >= 0,
        )
        .map(|VarInt(id)| Self(id as u32))
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
        nom_context("OptionalEntityId", EntityId::deserialize)
            .map(|raw| Self(NonZeroU32::new(raw.0)))
            .parse(input)
    }
}

// TODO: Implement Display
#[derive(Clone, Debug, PartialEq)]
pub enum TextComponent {
    Basic(String),
    Compound(NetworkNbt),
}

impl Deserialize for TextComponent {
    fn deserialize<'a>(input: InputSpan<'a>) -> IResult<'a, Self> {
        if input.len() < 1 {
            Err(nom::Err::Error(IErr::from_error_kind(
                input,
                nom::error::ErrorKind::Verify,
            )))
        } else {
            match input[0] {
                // TODO: The `Verify` error's fine, but there's probably a way of passing along a
                // more descriptive error for invalid UTF-8
                8 => nom_context(
                    "TextComponent",
                    length_value(
                        u16::deserialize.map(|len| len as usize),
                        |slice: InputSpan<'a>| {
                            core::str::from_utf8(slice.as_ref())
                                .map(|str| (slice, str.to_string()))
                                .map_err(|_| {
                                    nom::Err::Error(IErr::from_error_kind(
                                        slice,
                                        nom::error::ErrorKind::Verify,
                                    ))
                                })
                        },
                    ),
                )
                .map(Self::Basic)
                .parse(input.take_from(1)),
                10 => nom_context("TextComponent", NetworkNbt::deserialize)
                    .map(Self::Compound)
                    .parse(input),
                _ => Err(nom::Err::Error(IErr::from_error_kind(
                    input,
                    nom::error::ErrorKind::Verify,
                ))),
            }
        }
    }
}
