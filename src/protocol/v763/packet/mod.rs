pub mod handshaking;
pub mod login;
pub mod play;
pub mod status;

use super::prelude::*;
use bytebuffer::ByteBuffer;
use flate2::read::ZlibDecoder;
use nom::{Compare, InputLength, InputTake};
#[cfg(feature = "protocol_verbose")]
use nom_supreme::final_parser::final_parser;
use std::io::prelude::*;

pub trait PacketWrite: Serialize {
    const ID: i32;

    fn write_uncompressed_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let mut buffer = ByteBuffer::new();
        VarInt(Self::ID).serialize_to(&mut buffer)?;
        self.serialize_to(&mut buffer)?;
        VarInt(
            buffer
                .len()
                .try_into()
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?,
        )
        .serialize_into(writer)?;
        std::io::copy(&mut buffer, writer)?;
        Ok(())
    }

    fn write_uncompressed_into<W: std::io::Write>(self, writer: &mut W) -> std::io::Result<()> {
        let mut buffer = ByteBuffer::new();
        VarInt(Self::ID).serialize_into(&mut buffer)?;
        self.serialize_into(&mut buffer)?;
        VarInt(
            buffer
                .len()
                .try_into()
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?,
        )
        .serialize_into(writer)?;
        std::io::copy(&mut buffer, writer)?;
        Ok(())
    }
}

pub trait PacketRead: Deserialize {
    /// If provided, ID VarInt will be removed from packet data passed to deserialize. Otherwise,
    /// is passed through.
    const ID: Option<i32> = None;

    fn read_from<R: std::io::Read>(
        compression_threshold: Option<usize>,
        reader: &mut R,
    ) -> std::io::Result<Self> {
        match compression_threshold {
            None => Self::read_uncompressed_from(reader),
            Some(_) => Self::read_compressed_from(reader),
        }
    }

    // TODO Convert asserts into useful errors, fix data_left check using all_consuming
    fn read_uncompressed_from<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let (_, packet_length) = read_var_int_as_usize(reader)?;
        let mut raw_packet = match Self::ID {
            None => vec![0; packet_length],
            Some(id) => {
                let (id_length, packet_id) = read_var_int_as_usize(reader)?;
                assert_eq!(packet_id, id as usize);
                vec![0; packet_length - id_length]
            }
        };
        reader.read_exact(&mut raw_packet)?;
        #[cfg(not(feature = "protocol_verbose"))]
        {
            let (data_left, packet) = Self::deserialize(&raw_packet)
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err.to_owned()))?;
            assert_eq!(data_left, &[] as &[u8]);
            Ok(packet)
        }
        #[cfg(feature = "protocol_verbose")]
        {
            let result =
                final_parser(Self::deserialize)(&raw_packet).map_err(|err: ErrorTree<&[u8]>| {
                    if let Err(err) = std::fs::write("packet.bin", &raw_packet) {
                        eprintln!("Failed to write packet to file: {err}");
                    }
                    let err = convert_error_tree(err);
                    std::io::Error::new(std::io::ErrorKind::Other, format!("{err}"))
                });
            result
        }
    }

    fn read_compressed_from<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let (_, packet_length) = read_var_int_as_usize(reader)?;
        let (data_length_length, data_length) = read_var_int_as_usize(reader)?;
        let uncompressed_packet = match data_length {
            // Packet is already uncompressed, read as normal
            0 => {
                let mut raw_packet = match Self::ID {
                    None => vec![0; packet_length - data_length_length],
                    Some(id) => {
                        let (id_length, packet_id) = read_var_int_as_usize(reader)?;
                        assert_eq!(packet_id, id as usize);
                        vec![0; packet_length - id_length - data_length_length]
                    }
                };
                reader.read_exact(&mut raw_packet)?;
                raw_packet
            }
            _ => {
                let mut compressed_packet = vec![0; packet_length - data_length_length];
                reader.read_exact(&mut compressed_packet)?;
                let mut decompressor = ZlibDecoder::new(compressed_packet.as_slice());
                let mut raw_packet = match Self::ID {
                    None => vec![0; data_length],
                    Some(id) => {
                        let (id_length, packet_id) = read_var_int_as_usize(&mut decompressor)?;
                        assert_eq!(packet_id, id as usize);
                        vec![0; data_length - id_length]
                    }
                };
                decompressor.read_exact(&mut raw_packet)?;
                raw_packet
            }
        };
        #[cfg(not(feature = "protocol_verbose"))]
        {
            let (data_left, packet) = Self::deserialize(&uncompressed_packet)
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err.to_owned()))?;
            assert_eq!(data_left, &[] as &[u8]);
            Ok(packet)
        }
        #[cfg(feature = "protocol_verbose")]
        {
            let result = final_parser(Self::deserialize)(&uncompressed_packet).map_err(
                |err: ErrorTree<&[u8]>| {
                    if let Err(err) = std::fs::write("packet.bin", &uncompressed_packet) {
                        eprintln!("Failed to write packet to file: {err}");
                    }
                    let err = convert_error_tree(err);
                    std::io::Error::new(std::io::ErrorKind::Other, format!("{err}"))
                },
            );
            result
        }
    }
}

// FIXME This isn't correct, should read in i32 and check it can convert to usize
pub fn read_var_int_as_usize<R: std::io::Read>(reader: &mut R) -> std::io::Result<(usize, usize)> {
    let mut value: usize = 0;
    let mut position = 0;
    let mut bytes_read = 0;
    loop {
        let mut byte_slice = [0; 1];
        reader.read_exact(&mut byte_slice)?;
        bytes_read += 1;
        value |= (byte_slice[0] as usize & 0x7F) << position;
        if byte_slice[0] & 0x80 == 0 {
            break;
        }
        position += 7;
        assert!(position < 32);
    }
    Ok((bytes_read, value))
}

#[cfg(feature = "protocol_verbose")]
fn convert_error_tree(error: ErrorTree<&[u8]>) -> ErrorTree<ByteView> {
    match error {
        ErrorTree::Base { location, kind } => ErrorTree::Base {
            location: location.into(),
            kind,
        },
        ErrorTree::Stack { base, contexts } => ErrorTree::Stack {
            base: Box::new(convert_error_tree(*base)),
            contexts: contexts
                .into_iter()
                .map(|(location, stack_context)| (location.into(), stack_context))
                .collect(),
        },
        ErrorTree::Alt(errors) => {
            ErrorTree::Alt(errors.into_iter().map(convert_error_tree).collect())
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct ByteView<'a>(pub &'a [u8]);

impl<'a> From<&'a [u8]> for ByteView<'a> {
    fn from(value: &'a [u8]) -> Self {
        Self(value)
    }
}

impl AsRef<[u8]> for ByteView<'_> {
    fn as_ref(&self) -> &[u8] {
        self.0
    }
}

impl InputTake for ByteView<'_> {
    fn take(&self, count: usize) -> Self {
        Self(InputTake::take(&self.0, count))
    }

    fn take_split(&self, count: usize) -> (Self, Self) {
        let (slice_1, slice_2) = self.0.take_split(count);
        (Self(slice_1), Self(slice_2))
    }
}

macro_rules! impl_nom_compare {
    (<$($impl_generics:tt),+>, $T:ty, $target:ty) => {
        impl<$($impl_generics),+> Compare<$T> for $target {
            fn compare(&self, t: $T) -> nom::CompareResult {
                self.0.compare(t)
            }

            fn compare_no_case(&self, t: $T) -> nom::CompareResult {
                self.0.compare_no_case(t)
            }
        }
    };
    ($T:ty, $target:ty) => {
        impl Compare<$T> for $target {
            fn compare(&self, t: $T) -> nom::CompareResult {
                self.0.compare(t)
            }

            fn compare_no_case(&self, t: $T) -> nom::CompareResult {
                self.0.compare_no_case(t)
            }
        }
    };
}

impl_nom_compare!(<'a, 'b>, &'b [u8], ByteView<'a>);
impl_nom_compare!([u8; 0], ByteView<'_>);
impl_nom_compare!([u8; 1], ByteView<'_>);
impl_nom_compare!([u8; 2], ByteView<'_>);
impl_nom_compare!([u8; 3], ByteView<'_>);
impl_nom_compare!([u8; 4], ByteView<'_>);
impl_nom_compare!([u8; 5], ByteView<'_>);
impl_nom_compare!([u8; 6], ByteView<'_>);
impl_nom_compare!([u8; 7], ByteView<'_>);
impl_nom_compare!([u8; 8], ByteView<'_>);
impl_nom_compare!([u8; 9], ByteView<'_>);
impl_nom_compare!([u8; 10], ByteView<'_>);

impl InputLength for ByteView<'_> {
    fn input_len(&self) -> usize {
        self.0.input_len()
    }
}

impl std::fmt::Display for ByteView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const CHUNK_SIZE: usize = 16;
        f.write_str("Bytes[\n")?;
        for (i, chunk) in self.0.chunks(CHUNK_SIZE).enumerate() {
            // Address
            write!(f, "  {:0>8x}", i * CHUNK_SIZE)?;
            // Chunk bytes
            let mut rendered_chunk = [b' '; CHUNK_SIZE * 3];
            for (i, &byte) in chunk.iter().enumerate() {
                write!(&mut rendered_chunk[i * 3..], "{byte:0>2X}").unwrap();
            }
            // Chunk ASCII text
            let mut text_chunk = [' '; CHUNK_SIZE];
            for (character, &byte) in text_chunk.iter_mut().zip(chunk.iter()) {
                *character = match byte {
                    0 => '.',
                    ascii_char if ascii_char.is_ascii() && !ascii_char.is_ascii_control() => {
                        char::from_u32(byte as u32).unwrap()
                    }
                    byte => char::from_u32(0x2800 + byte as u32).unwrap(),
                };
            }
            f.write_str("  ")?;
            f.write_str(std::str::from_utf8(rendered_chunk.as_slice()).unwrap())?;
            f.write_str("  ")?;
            for character in text_chunk {
                f.write_fmt(format_args!("{character}"))?;
            }
            f.write_str("\n")?;
        }
        f.write_str("]")?;
        Ok(())
        // let mut list = f.debug_list();
        // for chunk in self.0.chunks(16) {
        //     let mut rendered_chunk = [0u8; 32];
        //     for (i, &byte) in chunk.iter().enumerate() {
        //         write!(&mut rendered_chunk[i / 2..], "{byte:X}").unwrap();
        //     }
        //     list.entry(&std::str::from_utf8(rendered_chunk.as_slice()).unwrap());
        // }
        // list.finish()
    }
}
