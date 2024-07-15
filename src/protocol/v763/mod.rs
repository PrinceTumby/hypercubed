#[macro_use]
pub mod basic_types;
pub mod packet;

pub mod prelude {
    pub use super::super::prelude::*;
    pub use super::basic_types::*;
    pub use super::packet::{PacketRead, PacketWrite};
    pub use super::{login, PlayConnection};
}

use super::OFFLINE_PLAYER_NAMESPACE;
use std::collections::VecDeque;
use std::io::prelude::*;
use std::net::TcpStream;
use uuid::Uuid;

#[derive(Debug)]
pub struct PlayConnection {
    stream: TcpStream,
    compression_threshold: Option<usize>,
    packet_queue: VecDeque<packet::play::Clientbound>,
    bundle_queue: VecDeque<packet::play::Clientbound>,
    inside_bundle: bool,
}

impl PlayConnection {
    pub fn new(stream: TcpStream, compression_threshold: Option<usize>) -> Self {
        Self {
            stream,
            compression_threshold,
            packet_queue: VecDeque::new(),
            bundle_queue: VecDeque::new(),
            inside_bundle: false,
        }
    }

    pub fn read_packet(&mut self) -> std::io::Result<packet::play::Clientbound> {
        use packet::play::Clientbound;
        use prelude::*;
        loop {
            if let Some(packet) = self.packet_queue.pop_front() {
                return Ok(packet);
            }
            match self.inside_bundle {
                false => {
                    match Clientbound::read_from(self.compression_threshold, &mut self.stream)? {
                        Clientbound::BundleDelimiter => self.inside_bundle = true,
                        packet => self.packet_queue.push_back(packet),
                    }
                }
                true => match Clientbound::read_from(self.compression_threshold, &mut self.stream)?
                {
                    Clientbound::BundleDelimiter => {
                        self.packet_queue.extend(self.bundle_queue.drain(..));
                        self.inside_bundle = false;
                    }
                    packet => self.bundle_queue.push_back(packet),
                },
            }
        }
    }
}

pub fn login() -> std::io::Result<(PlayConnection, packet::login::LoginSuccess)> {
    use prelude::*;
    let mut stream = TcpStream::connect("localhost:25565")?;
    // Send handshake packet
    packet::handshaking::Handshake {
        protocol_version: 763,
        address: "localhost",
        server_port: 25565,
        next_state: packet::handshaking::HandshakeNextState::Login,
    }
    .write_packet_into(&mut stream, None)?;
    packet::login::LoginStart {
        username: "Sleepman",
        player_uuid: Some(Uuid::new_v3(&OFFLINE_PLAYER_NAMESPACE, b"Sleepman")),
    }
    .write_packet_into(&mut stream, None)?;
    stream.flush()?;
    let mut compression_threshold = None;
    loop {
        match packet::login::Response::read_from(compression_threshold, &mut stream)? {
            packet::login::Response::Success(success_packet) => {
                return Ok((
                    PlayConnection::new(stream, compression_threshold),
                    success_packet,
                ))
            }
            packet::login::Response::SetCompression { threshold } => {
                compression_threshold = match threshold.0 {
                    ..=-1 => None,
                    0.. => Some(threshold.0 as usize),
                };
            }
            packet::login::Response::ErrorDisconnect { reason } => {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, reason))
            }
        }
    }
}
