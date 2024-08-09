#[macro_use]
pub mod basic_types;
pub mod chunk;
pub mod configuration;
pub mod login;
pub mod play;

pub mod prelude {
    pub use super::super::prelude::*;
    pub use super::basic_types::*;
    pub use super::{login, PacketRead, PacketWrite, PlayConnection, PluginMessage};
}

pub use crate::protocol::v763::packet::{
    handshaking, read_var_int_as_usize, status, ByteView, PacketRead, PacketWrite, PluginMessage,
};

use cfb8::cipher::{BlockDecryptMut, BlockEncryptMut};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Mutex;

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
