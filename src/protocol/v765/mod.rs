#[macro_use]
pub mod basic_types;
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

use super::OFFLINE_PLAYER_NAMESPACE;
use bytebuffer::ByteBuffer;
use std::collections::VecDeque;
use std::io::prelude::*;
use std::net::TcpStream;
use uuid::Uuid;

#[derive(Debug)]
pub struct PlayConnection {
    stream: TcpStream,
    compression_threshold: Option<usize>,
    packet_queue: VecDeque<play::Clientbound>,
    bundle_queue: VecDeque<play::Clientbound>,
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

    pub fn read_packet(&mut self) -> std::io::Result<play::Clientbound> {
        use play::Clientbound;
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

    pub fn send_packet<P: PacketWrite>(&mut self, packet: P) -> std::io::Result<()> {
        packet.write_packet_into(&mut self.stream, self.compression_threshold)
    }
}

pub fn login(
    address: &str,
    port: u16,
    client_information: configuration::ClientInformation,
) -> std::io::Result<(PlayConnection, login::LoginSuccess)> {
    use prelude::*;
    let mut stream = TcpStream::connect(format!("{address}:{port}"))?;
    // Send handshake packet
    handshaking::Handshake {
        protocol_version: 765,
        address,
        server_port: port,
        next_state: handshaking::HandshakeNextState::Login,
    }
    .write_packet_into(&mut stream, None)?;
    // TODO NEXT Implement configuration, move packet stuff into main folder
    login::LoginStart {
        username: "Sleepman",
        player_uuid: Uuid::new_v3(&OFFLINE_PLAYER_NAMESPACE, b"Sleepman"),
    }
    .write_packet_into(&mut stream, None)?;
    stream.flush()?;
    let mut compression_threshold = None;
    // TODO Implement compressed serialization
    // Login phase
    let success_packet = loop {
        match login::Response::read_from(compression_threshold, &mut stream)? {
            login::Response::Success(success_packet) => {
                login::LoginAcknowledged.write_packet_into(&mut stream, compression_threshold)?;
                break success_packet;
            }
            login::Response::SetCompression { threshold } => {
                log::debug!("Compression threshold set to {}", threshold.0);
                compression_threshold = match threshold.0 {
                    ..=-1 => None,
                    0.. => Some(threshold.0 as usize),
                };
            }
            login::Response::ErrorDisconnect { reason } => {
                return Err(std::io::Error::other(reason));
            }
        }
    };
    // Configuration phase
    loop {
        match configuration::Response::read_from(compression_threshold, &mut stream)? {
            configuration::Response::Finish => {
                client_information.write_packet_into(&mut stream, compression_threshold)?;
                configuration::AcknowledgeFinish
                    .write_packet_into(&mut stream, compression_threshold)?;
                return Ok((
                    PlayConnection::new(stream, compression_threshold),
                    success_packet,
                ));
            }
            configuration::Response::PluginMessage(message) => match message.channel.as_str() {
                "minecraft:brand" => {
                    const CLIENT_BRAND: &str = "rust_minecraft_client";
                    let (extra_data, server_brand) = String::deserialize(&message.data)
                        .map_err(|err| std::io::Error::other(format!("{err}")))?;
                    if extra_data.len() != 0 {
                        return Err(std::io::Error::other(format!(
                            "Invalid extra data while deserializing server brand: {extra_data:?}"
                        )));
                    }
                    log::debug!(
                        "Server sent brand {}, replying as having client brand {}",
                        server_brand,
                        CLIENT_BRAND,
                    );
                    let mut client_brand_data = ByteBuffer::new();
                    String::from(CLIENT_BRAND).serialize_into(&mut client_brand_data)?;
                    configuration::ServerboundPluginMessage {
                        channel: "minecraft:brand",
                        data: client_brand_data.as_bytes(),
                    }
                    .write_packet_into(&mut stream, compression_threshold)?;
                }
                _ => log::warn!(
                    "Received plugin message from unknown channel while configuring: {message:?}"
                ),
            },
            configuration::Response::KeepAlive(id) => {
                configuration::KeepAliveResponse(id).serialize_into(&mut stream)?;
            }
            configuration::Response::Ping(id) => {
                configuration::Pong(id).serialize_into(&mut stream)?;
            }
            configuration::Response::RegistryData(_registry_data) => log::warn!(
                "Server registry data handling currently unimplemented, ignoring for now"
            ),
            configuration::Response::EnableFeatures(features) => {
                if &features != &[String::from("minecraft:vanilla")] {
                    todo!("Support alternative features: {:?}", features);
                }
            }
            configuration::Response::UpdateTags(_registry_tags) => log::warn!(
                "Server registry tags data handling currently unimplemented, ignoring for now"
            ),
            configuration::Response::ErrorDisconnect { reason } => {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, reason))
            }
            other => todo!("Handle configuration packet: {other:?}"),
        }
    }
}
