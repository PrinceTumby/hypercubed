#[macro_use]
pub mod basic_types;
pub mod chunk;
pub mod configuration;
pub mod handshaking;
pub mod login;
pub mod play;
pub mod status;

#[macro_use]
pub mod prelude {
    pub use super::basic_types::*;
    pub use super::{
        Deserialize, PROTOCOL_VERSION, PacketRead, PacketWrite, PlayConnection, Serialize,
        request_status,
    };

    pub use parsing::*;
    pub type IResult<'a, O> = Result<(InputSpan<'a>, O), nom::Err<IErr<'a>>>;

    #[cfg(not(feature = "protocol_verbose"))]
    mod parsing {
        // pub type InputSpan<'a> = &'a [u8];
        pub type InputSpan<'a> = nom_locate::LocatedSpan<&'a [u8]>;
        pub type IErr<'a> = nom::error::Error<InputSpan<'a>>;
    }

    #[cfg(feature = "protocol_verbose")]
    mod parsing {
        use nom_supreme::error::GenericErrorTree;
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

use bytebuffer::ByteBuffer;
use cfb8::cipher::{BlockDecryptMut, BlockEncryptMut};
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use nom::{Compare, InputLength, InputTake};
#[cfg(feature = "protocol_verbose")]
use nom_supreme::final_parser::final_parser;
use prelude::*;
use std::collections::VecDeque;
use std::io::prelude::*;
use std::net::TcpStream;
use std::sync::Mutex;
use uuid::{Uuid, uuid};

pub trait Deserialize: Sized {
    fn deserialize(input: InputSpan) -> IResult<Self>;
}

pub trait Serialize: Sized {
    // TODO: Come up with better names for these
    fn serialize_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()>;

    fn serialize_into<O: std::io::Write>(self, output: &mut O) -> std::io::Result<()> {
        self.serialize_to(output)
    }
}

pub fn write_uncompressed_buffer<W: std::io::Write>(
    buffer: &mut ByteBuffer,
    writer: &mut W,
) -> std::io::Result<()> {
    // Uncompressed packet format, write packet length followed by packet
    VarInt(
        buffer
            .len()
            .try_into()
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?,
    )
    .serialize_into(writer)?;
    std::io::copy(buffer, writer)?;
    Ok(())
}

pub fn write_compressed_buffer<W: std::io::Write>(
    buffer: &mut ByteBuffer,
    writer: &mut W,
    compression_threshold: usize,
) -> std::io::Result<()> {
    if buffer.len() >= compression_threshold {
        // Compressed packet format, write compressed packet length + length of
        // uncompressed packet length varint, then uncompressed packet length, then the
        // compressed packet
        let mut compressed_buffer = ByteBuffer::new();
        VarInt(
            buffer
                .len()
                .try_into()
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?,
        )
        .serialize_into(&mut compressed_buffer)?;
        let mut compressor = ZlibEncoder::new(buffer, Compression::fast());
        std::io::copy(&mut compressor, &mut compressed_buffer)?;
        VarInt(
            compressed_buffer
                .len()
                .try_into()
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?,
        )
        .serialize_into(writer)?;
        std::io::copy(&mut compressed_buffer, writer)?;
    } else {
        // Compression enabled but packet below threshold, write packet_length + 1 (for
        // size of 0 varint), then write 0 for the uncompressed length, then the
        // uncompressed packet
        VarInt(
            (buffer.len() + 1)
                .try_into()
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?,
        )
        .serialize_into(writer)?;
        VarInt(0).serialize_into(writer)?;
        std::io::copy(buffer, writer)?;
    }
    Ok(())
}

pub trait PacketWrite: Serialize {
    const ID: i32;

    fn write_packet_to<W: std::io::Write>(
        &self,
        writer: &mut W,
        compression_threshold: Option<usize>,
    ) -> std::io::Result<()> {
        let mut buffer = ByteBuffer::new();
        VarInt(Self::ID).serialize_to(&mut buffer)?;
        self.serialize_to(&mut buffer)?;
        match compression_threshold {
            None => write_uncompressed_buffer(&mut buffer, writer),
            Some(threshold) => write_compressed_buffer(&mut buffer, writer, threshold),
        }
    }

    fn write_packet_into<W: std::io::Write>(
        self,
        writer: &mut W,
        compression_threshold: Option<usize>,
    ) -> std::io::Result<()> {
        let mut buffer = ByteBuffer::new();
        VarInt(Self::ID).serialize_to(&mut buffer)?;
        self.serialize_into(&mut buffer)?;
        match compression_threshold {
            None => write_uncompressed_buffer(&mut buffer, writer),
            Some(threshold) => write_compressed_buffer(&mut buffer, writer, threshold),
        }
    }
}

pub trait PacketRead: Deserialize + std::fmt::Debug {
    fn read_from<R: std::io::Read>(
        compression_threshold: Option<usize>,
        reader: &mut R,
    ) -> std::io::Result<Self> {
        match compression_threshold {
            None => Self::read_uncompressed_from(reader),
            Some(_) => Self::read_compressed_from(reader),
        }
    }

    // TODO: Convert asserts into useful errors, fix data_left check using all_consuming
    fn read_uncompressed_from<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let (_, packet_length) = read_var_int_as_usize(reader)?;
        let mut raw_packet = vec![0; packet_length];
        reader.read_exact(&mut raw_packet)?;
        #[cfg(not(feature = "protocol_verbose"))]
        {
            let (data_left, packet) =
                Self::deserialize(InputSpan::new(&raw_packet)).map_err(|err| {
                    if let Err(err) = std::fs::write("packet.bin", &raw_packet) {
                        eprintln!("Failed to write packet to file: {err}");
                    }
                    std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!(
                            "failed to deserialise input {}: {err}",
                            ByteView(InputSpan::new(&raw_packet))
                        ),
                    )
                })?;
            assert_eq!(data_left.as_ref(), &[] as &[u8]);
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
                    std::io::Error::new(std::io::ErrorKind::Other, format!("{err}"))
                },
            );
            result
        }
    }

    fn read_compressed_from<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
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
                let mut decompressor = ZlibDecoder::new(compressed_packet.as_slice());
                let mut raw_packet = vec![0; data_length];
                decompressor.read_exact(&mut raw_packet)?;
                raw_packet
            }
        };
        #[cfg(not(feature = "protocol_verbose"))]
        {
            let (data_left, packet) = Self::deserialize(InputSpan::new(&uncompressed_packet))
                .map_err(|err| {
                    if let Err(err) = std::fs::write("packet.bin", &uncompressed_packet) {
                        eprintln!("Failed to write packet to file: {err}");
                    }
                    std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!(
                            "failed to deserialise input {}: {err}",
                            ByteView(InputSpan::new(&uncompressed_packet)),
                        ),
                    )
                })?;
            assert_eq!(
                data_left.as_ref(),
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
                    std::io::Error::new(std::io::ErrorKind::Other, format!("{err}"))
                });
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
fn convert_error_tree(error: ErrorTree) -> ByteViewErrorTree {
    match error {
        ErrorTree::Base { location, kind } => ByteViewErrorTree::Base {
            location: location.into(),
            kind,
        },
        ErrorTree::Stack { base, contexts } => ByteViewErrorTree::Stack {
            base: Box::new(convert_error_tree(*base)),
            contexts: contexts
                .into_iter()
                .map(|(location, stack_context)| (location.into(), stack_context))
                .collect(),
        },
        ErrorTree::Alt(errors) => {
            ByteViewErrorTree::Alt(errors.into_iter().map(convert_error_tree).collect())
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ByteView<'a>(pub InputSpan<'a>);

impl<'a> From<InputSpan<'a>> for ByteView<'a> {
    fn from(span: InputSpan<'a>) -> Self {
        Self(span)
    }
}

impl InputTake for ByteView<'_> {
    fn take(&self, count: usize) -> Self {
        Self(self.0.take(count))
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
            f.write_str(std::str::from_utf8(rendered_chunk.as_slice()).unwrap())?;
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

pub fn request_status(protocol_version: i32, address: &str, port: u16) -> std::io::Result<String> {
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
            Err(std::io::Error::other(format!("Error Disconnect: {reason}")))
        }
        unknown => Err(std::io::Error::other(format!(
            "Unknown response: {unknown:?}"
        ))),
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

impl<'a> Read for EncryptedTcpStreamReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let bytes_read = self.tcp_stream.read(buf)?;
        for i in 0..bytes_read {
            self.decryptor.decrypt_block_mut((&mut buf[i..=i]).into());
        }
        Ok(bytes_read)
    }
}

struct EncryptedTcpStreamWriter<'a> {
    tcp_stream: &'a TcpStream,
    encryptor: &'a mut Aes128Cfb8Enc,
    encryption_buffer: Vec<u8>,
}

impl<'a> Write for EncryptedTcpStreamWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.encryption_buffer.extend_from_slice(buf);
        for i in 0..self.encryption_buffer.len() {
            self.encryptor
                .encrypt_block_mut((&mut self.encryption_buffer[i..=i]).into());
        }
        let write_result = self.tcp_stream.write_all(self.encryption_buffer.as_slice());
        self.encryption_buffer.clear();
        write_result.map(|()| buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
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

    pub fn read_packet(&self) -> std::io::Result<play::Clientbound> {
        use play::Clientbound;
        use prelude::*;
        let mut read_state_lock = self.read_state.lock().unwrap();
        let read_state = &mut *read_state_lock;
        let packet_queue = &mut read_state.packet_queue;
        let bundle_queue = &mut read_state.bundle_queue;
        let inside_bundle = &mut read_state.inside_bundle;
        let mut decrypting_reader;
        let mut read_stream = match read_state.decryptor.as_mut() {
            None => &mut &self.stream as &mut dyn Read,
            Some(decryptor) => {
                decrypting_reader = EncryptedTcpStreamReader {
                    tcp_stream: &self.stream,
                    decryptor,
                };
                &mut decrypting_reader as &mut dyn Read
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

    pub fn send_packet<P: PacketWrite>(&self, packet: P) -> std::io::Result<()> {
        let mut write_state_lock = self.write_state.lock().unwrap();
        let write_state = &mut *write_state_lock;
        let mut encrypting_writer;
        let mut write_stream = match write_state.encryptor.as_mut() {
            None => &mut &self.stream as &mut dyn Write,
            Some(encryptor) => {
                encrypting_writer = EncryptedTcpStreamWriter {
                    tcp_stream: &self.stream,
                    encryptor,
                    encryption_buffer: Vec::new(),
                };
                &mut encrypting_writer as &mut dyn Write
            }
        };
        packet.write_packet_into(&mut write_stream, self.compression_threshold)
    }

    pub fn flush(&self) -> std::io::Result<()> {
        (&mut &self.stream).flush()
    }
}
