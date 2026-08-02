#[macro_use]
pub mod basic_types;
pub mod chunk;
pub mod configuration;
pub mod handshaking;
pub mod login;
pub mod play;
pub mod status;
#[cfg(feature = "protocol_verbose")]
pub mod verbose;

#[macro_use]
pub mod prelude {
    pub use super::basic_types::*;
    pub use super::{
        Deserialize, PROTOCOL_VERSION, PacketRead, PacketWrite, PlayConnection, Serialize,
        request_status,
    };

    pub use parsing::*;
    pub type IResult<'a, O> = Result<(InputSpan<'a>, O), nom::Err<IErr<'a>>>;

    pub use nom::error::context as nom_context;

    #[cfg(not(feature = "protocol_verbose"))]
    mod parsing {
        // pub type InputSpan<'a> = &'a [u8];
        pub type InputSpan<'a> = nom_locate::LocatedSpan<&'a [u8]>;
        pub type IErr<'a> = nom::error::Error<InputSpan<'a>>;
    }

    #[cfg(feature = "protocol_verbose")]
    mod parsing {
        use nom_supreme::error::GenericErrorTree;
        use portable_std::Vec;
        use std::error::Error;

        pub type InputSpan<'a> = nom_locate::LocatedSpan<&'a [u8]>;
        pub type ErrorTree<'a> = GenericErrorTree<
            InputSpan<'a>,
            &'static [u8],
            &'static str,
            Box<dyn Error + Send + Sync + 'static>,
        >;
        pub type ByteViewErrorTree<'a> = GenericErrorTree<
            super::super::ByteView<'a>,
            &'static [u8],
            &'static str,
            Box<dyn Error + Send + Sync + 'static>,
        >;
        pub type IErr<'a> = ErrorTree<'a>;
    }
}

// MC 1.21.1
pub const PROTOCOL_VERSION: i32 = 767;
pub const OFFLINE_PLAYER_NAMESPACE: Uuid = uuid!("071e6668-28ee-39de-8f51-f257ec5f77a9");

use crate::platform::net::TcpStream;
use crate::portable_prelude::{eprintln, *};
use cfb8::cipher::{BlockModeDecrypt, BlockModeEncrypt};
use miniz_oxide::deflate::{CompressionLevel, compress_to_vec_zlib};
use miniz_oxide::inflate::decompress_to_vec_zlib_with_limit;
#[cfg(feature = "protocol_verbose")]
use nom_supreme::final_parser::final_parser;
use portable_std::VecDeque;
use portable_std::io::{self, prelude::*};
use portable_std::sync::Mutex;
use prelude::*;
use uuid::{Uuid, uuid};

pub trait Deserialize: Sized {
    fn deserialize(input: InputSpan) -> IResult<Self>;
}

pub trait Serialize: Sized {
    // TODO: Come up with better names for these
    fn serialize_to<W: io::Write>(&self, writer: &mut W) -> io::Result<()>;

    fn serialize_into<O: io::Write>(self, output: &mut O) -> io::Result<()> {
        self.serialize_to(output)
    }
}

pub fn write_uncompressed_buffer<W: io::Write>(buffer: &[u8], writer: &mut W) -> io::Result<()> {
    // Uncompressed packet format, write packet length followed by packet
    VarInt(buffer.len().try_into().map_err(io::Error::other)?).serialize_into(writer)?;
    writer.write_all(buffer)
}

pub fn write_compressed_buffer<W: io::Write>(
    buffer: &[u8],
    writer: &mut W,
    compression_threshold: usize,
) -> io::Result<()> {
    if buffer.len() >= compression_threshold {
        // Compressed packet format, write compressed packet length + length of
        // uncompressed packet length varint, then uncompressed packet length, then the
        // compressed packet
        let mut compressed_buffer = Vec::new();
        VarInt(buffer.len().try_into().map_err(io::Error::other)?)
            .serialize_into(&mut compressed_buffer)?;
        compressed_buffer.extend(compress_to_vec_zlib(
            buffer,
            CompressionLevel::BestSpeed as u8,
        ));
        VarInt(
            compressed_buffer
                .len()
                .try_into()
                .map_err(io::Error::other)?,
        )
        .serialize_into(writer)?;
        writer.write_all(&compressed_buffer)?;
    } else {
        // Compression enabled but packet below threshold, write packet_length + 1 (for
        // size of 0 varint), then write 0 for the uncompressed length, then the
        // uncompressed packet
        VarInt((buffer.len() + 1).try_into().map_err(io::Error::other)?).serialize_into(writer)?;
        VarInt(0).serialize_into(writer)?;
        writer.write_all(buffer)?;
    }
    Ok(())
}

pub trait PacketWrite: Serialize {
    const ID: i32;

    fn write_packet_to<W: io::Write>(
        &self,
        writer: &mut W,
        compression_threshold: Option<usize>,
    ) -> io::Result<()> {
        let mut buffer = Vec::new();
        VarInt(Self::ID).serialize_to(&mut buffer)?;
        self.serialize_to(&mut buffer)?;
        match compression_threshold {
            None => write_uncompressed_buffer(&buffer, writer),
            Some(threshold) => write_compressed_buffer(&buffer, writer, threshold),
        }
    }

    fn write_packet_into<W: io::Write>(
        self,
        writer: &mut W,
        compression_threshold: Option<usize>,
    ) -> io::Result<()> {
        let mut buffer = Vec::new();
        VarInt(Self::ID).serialize_to(&mut buffer)?;
        self.serialize_into(&mut buffer)?;
        match compression_threshold {
            None => write_uncompressed_buffer(&buffer, writer),
            Some(threshold) => write_compressed_buffer(&buffer, writer, threshold),
        }
    }
}

pub trait PacketRead: Deserialize + core::fmt::Debug {
    fn read_from<R: io::Read>(
        compression_threshold: Option<usize>,
        reader: &mut R,
    ) -> io::Result<Self> {
        match compression_threshold {
            None => Self::read_uncompressed_from(reader),
            Some(_) => Self::read_compressed_from(reader),
        }
    }

    fn read_uncompressed_from<R: io::Read>(reader: &mut R) -> io::Result<Self> {
        let (_, packet_length) = read_var_int_as_usize(reader)?;
        let mut raw_packet = vec![0; packet_length];
        reader.read_exact(&mut raw_packet)?;
        #[cfg(not(feature = "protocol_verbose"))]
        {
            let (data_left, packet) =
                Self::deserialize(InputSpan::new(&raw_packet)).map_err(|err| {
                    #[cfg(feature = "full_std")]
                    if let Err(err) = std::fs::write("packet.bin", &raw_packet) {
                        eprintln!("Failed to write packet to file: {err}");
                    }
                    io::Error::other(format!(
                        "failed to deserialise input {}: {err}",
                        ByteView(InputSpan::new(&raw_packet))
                    ))
                })?;
            assert_eq!(*data_left, &[] as &[u8]);
            Ok(packet)
        }
        #[cfg(feature = "protocol_verbose")]
        {
            let result = final_parser(Self::deserialize)(InputSpan::new(&raw_packet)).map_err(
                |err: ErrorTree| {
                    if let Err(err) = std::fs::write("packet.bin", &raw_packet) {
                        eprintln!("Failed to write packet to file: {err}");
                    }
                    let err = convert_error_tree(err);
                    io::Error::new(io::ErrorKind::Other, format!("{err}"))
                },
            );
            result
        }
    }

    fn read_compressed_from<R: io::Read>(reader: &mut R) -> io::Result<Self> {
        let (_, packet_length) = read_var_int_as_usize(reader)?;
        let (data_length_length, data_length) = read_var_int_as_usize(reader)?;
        let uncompressed_packet = match data_length {
            // Packet is already uncompressed, read as normal
            0 => {
                let mut raw_packet = vec![0; packet_length - data_length_length];
                reader.read_exact(&mut raw_packet)?;
                raw_packet
            }
            _ => {
                let mut compressed_packet = vec![0; packet_length - data_length_length];
                reader.read_exact(&mut compressed_packet)?;
                decompress_to_vec_zlib_with_limit(&compressed_packet, data_length)
                    .map_err(|err| io::Error::other(format!("{err}")))?
            }
        };
        #[cfg(not(feature = "protocol_verbose"))]
        {
            let (data_left, packet) = Self::deserialize(InputSpan::new(&uncompressed_packet))
                .map_err(|err| {
                    #[cfg(feature = "full_std")]
                    if let Err(err) = std::fs::write("packet.bin", &uncompressed_packet) {
                        eprintln!("Failed to write packet to file: {err}");
                    }
                    io::Error::other(format!(
                        "failed to deserialise input {}: {err}",
                        ByteView(InputSpan::new(&uncompressed_packet)),
                    ))
                })?;
            assert_eq!(
                *data_left,
                &[] as &[u8],
                "data left after packet - packet: {packet:?}, data_left: {data_left:?}",
            );
            Ok(packet)
        }
        #[cfg(feature = "protocol_verbose")]
        {
            let result = final_parser(Self::deserialize)(InputSpan::new(&uncompressed_packet))
                .map_err(|err: ErrorTree| {
                    if let Err(err) = std::fs::write("packet.bin", &uncompressed_packet) {
                        eprintln!("Failed to write packet to file: {err}");
                    }
                    let err = convert_error_tree(err);
                    io::Error::new(io::ErrorKind::Other, format!("{err}"))
                });
            result
        }
    }
}

// FIXME: This isn't correct, should read in i32 and check it can convert to usize
pub fn read_var_int_as_usize<R: io::Read>(reader: &mut R) -> io::Result<(usize, usize)> {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ByteView<'a>(pub InputSpan<'a>);

impl<'a> From<InputSpan<'a>> for ByteView<'a> {
    fn from(span: InputSpan<'a>) -> Self {
        Self(span)
    }
}

impl<'a> nom::Input for ByteView<'a> {
    type Item = <&'a [u8] as nom::Input>::Item;
    type Iter = <&'a [u8] as nom::Input>::Iter;
    type IterIndices = <&'a [u8] as nom::Input>::IterIndices;

    fn input_len(&self) -> usize {
        self.0.input_len()
    }

    fn take(&self, index: usize) -> Self {
        Self(self.0.take(index))
    }

    fn take_from(&self, index: usize) -> Self {
        Self(self.0.take_from(index))
    }

    fn take_split(&self, index: usize) -> (Self, Self) {
        let (slice_1, slice_2) = self.0.take_split(index);
        (Self(slice_1), Self(slice_2))
    }

    fn position<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Item) -> bool,
    {
        self.0.position(predicate)
    }

    fn iter_elements(&self) -> Self::Iter {
        self.0.iter_elements()
    }

    fn iter_indices(&self) -> Self::IterIndices {
        self.0.iter_indices()
    }

    fn slice_index(&self, count: usize) -> Result<usize, nom::Needed> {
        self.0.slice_index(count)
    }
}

macro_rules! impl_nom_compare {
    (<$($impl_generics:tt),+>, $T:ty, $target:ty) => {
        impl<$($impl_generics),+> nom::Compare<$T> for $target {
            fn compare(&self, t: $T) -> nom::CompareResult {
                self.0.compare(t)
            }

            fn compare_no_case(&self, t: $T) -> nom::CompareResult {
                self.0.compare_no_case(t)
            }
        }
    };
    ($T:ty, $target:ty) => {
        impl nom::Compare<$T> for $target {
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

impl core::fmt::Display for ByteView<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        const CHUNK_SIZE: usize = 16;
        f.write_str("Bytes[\n")?;
        for (i, chunk) in self.0.chunks(CHUNK_SIZE).enumerate() {
            // Address
            let address = self.0.location_offset() + (i * CHUNK_SIZE);
            write!(f, "  {:0>8x}", address)?;
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
            f.write_str(core::str::from_utf8(rendered_chunk.as_slice()).unwrap())?;
            f.write_str("  ")?;
            for character in text_chunk {
                f.write_fmt(format_args!("{character}"))?;
            }
            f.write_str("\n")?;
        }
        f.write_str("]")?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginMessage {
    pub channel: Identifier,
    pub data: Vec<u8>,
}

impl Deserialize for PluginMessage {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        let (rest, channel) = Identifier::deserialize(input)?;
        Ok((
            InputSpan::new(&[]),
            Self {
                channel,
                data: rest.to_vec(),
            },
        ))
    }
}

pub fn request_status(protocol_version: i32, address: &str, port: u16) -> io::Result<String> {
    use status::Response;
    let mut stream = TcpStream::connect(format!("{address}:{port}"))?;
    handshaking::Handshake {
        protocol_version,
        address,
        server_port: port,
        next_state: handshaking::HandshakeNextState::Status,
    }
    .write_packet_into(&mut stream, None)?;
    status::StatusRequest.write_packet_into(&mut stream, None)?;
    stream.flush()?;
    match Response::read_uncompressed_from(&mut stream)? {
        Response::Status(status) => Ok(status),
        Response::ErrorDisconnect { reason } => {
            Err(io::Error::other(format!("Error Disconnect: {reason}")))
        }
        unknown => Err(io::Error::other(format!("Unknown response: {unknown:?}"))),
    }
}

pub type Aes128Cfb8Enc = cfb8::Encryptor<aes::Aes128>;
pub type Aes128Cfb8Dec = cfb8::Decryptor<aes::Aes128>;

pub struct PlayConnection {
    stream: TcpStream,
    compression_threshold: Option<usize>,
    read_state: Mutex<PlayConnectionReadState>,
    write_state: Mutex<PlayConnectionWriteState>,
}

struct PlayConnectionWriteState {
    encryptor: Option<Aes128Cfb8Enc>,
}

struct PlayConnectionReadState {
    decryptor: Option<Aes128Cfb8Dec>,
    packet_queue: VecDeque<play::Clientbound>,
    bundle_queue: VecDeque<play::Clientbound>,
    inside_bundle: bool,
}

struct EncryptedTcpStreamReader<'a> {
    tcp_stream: &'a TcpStream,
    decryptor: &'a mut Aes128Cfb8Dec,
}

impl<'a> io::Read for EncryptedTcpStreamReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let bytes_read = self.tcp_stream.read(buf)?;
        for i in 0..bytes_read {
            self.decryptor.decrypt_block(buf[i..=i].as_mut_array().unwrap().into());
        }
        Ok(bytes_read)
    }
}

struct EncryptedTcpStreamWriter<'a> {
    tcp_stream: &'a TcpStream,
    encryptor: &'a mut Aes128Cfb8Enc,
    encryption_buffer: Vec<u8>,
}

impl<'a> io::Write for EncryptedTcpStreamWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.encryption_buffer.extend_from_slice(buf);
        for i in 0..self.encryption_buffer.len() {
            self.encryptor
                .encrypt_block(self.encryption_buffer[i..=i].as_mut_array().unwrap().into());
        }
        let write_result = self.tcp_stream.write_all(self.encryption_buffer.as_slice());
        self.encryption_buffer.clear();
        write_result.map(|()| buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.tcp_stream.flush()
    }
}

impl PlayConnection {
    pub fn new(
        stream: TcpStream,
        compression_threshold: Option<usize>,
        encryptor: Option<Aes128Cfb8Enc>,
        decryptor: Option<Aes128Cfb8Dec>,
    ) -> Self {
        Self {
            stream,
            compression_threshold,
            read_state: Mutex::new(PlayConnectionReadState {
                decryptor,
                packet_queue: VecDeque::new(),
                bundle_queue: VecDeque::new(),
                inside_bundle: false,
            }),
            write_state: Mutex::new(PlayConnectionWriteState { encryptor }),
        }
    }

    pub fn read_packet(&self) -> io::Result<play::Clientbound> {
        use play::Clientbound;
        use prelude::*;
        let mut read_state_lock = self.read_state.lock().unwrap();
        let read_state = &mut *read_state_lock;
        let packet_queue = &mut read_state.packet_queue;
        let bundle_queue = &mut read_state.bundle_queue;
        let inside_bundle = &mut read_state.inside_bundle;
        let mut decrypting_reader;
        let mut read_stream = match read_state.decryptor.as_mut() {
            None => &mut &self.stream as &mut dyn io::Read,
            Some(decryptor) => {
                decrypting_reader = EncryptedTcpStreamReader {
                    tcp_stream: &self.stream,
                    decryptor,
                };
                &mut decrypting_reader as &mut dyn io::Read
            }
        };
        loop {
            if let Some(packet) = packet_queue.pop_front() {
                return Ok(packet);
            }
            match inside_bundle {
                false => {
                    match Clientbound::read_from(self.compression_threshold, &mut read_stream)? {
                        Clientbound::BundleDelimiter => *inside_bundle = true,
                        packet => packet_queue.push_back(packet),
                    }
                }
                true => {
                    match Clientbound::read_from(self.compression_threshold, &mut read_stream)? {
                        Clientbound::BundleDelimiter => {
                            packet_queue.extend(bundle_queue.drain(..));
                            *inside_bundle = false;
                        }
                        packet => bundle_queue.push_back(packet),
                    }
                }
            }
        }
    }

    pub fn send_packet<P: PacketWrite>(&self, packet: P) -> io::Result<()> {
        let mut write_state_lock = self.write_state.lock().unwrap();
        let write_state = &mut *write_state_lock;
        let mut encrypting_writer;
        let mut write_stream = match write_state.encryptor.as_mut() {
            None => &mut &self.stream as &mut dyn io::Write,
            Some(encryptor) => {
                encrypting_writer = EncryptedTcpStreamWriter {
                    tcp_stream: &self.stream,
                    encryptor,
                    encryption_buffer: Vec::new(),
                };
                &mut encrypting_writer as &mut dyn io::Write
            }
        };
        packet.write_packet_into(&mut write_stream, self.compression_threshold)
    }

    pub fn flush(&self) -> io::Result<()> {
        (&mut &self.stream).flush()
    }
}
