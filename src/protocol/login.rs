use super::prelude::*;
use super::{
    configuration, Aes128Cfb8Dec, Aes128Cfb8Enc, EncryptedTcpStreamReader,
    EncryptedTcpStreamWriter, PlayConnection, OFFLINE_PLAYER_NAMESPACE,
};
use crate::identifier;
use bytebuffer::ByteBuffer;
use protocol_derive::{Deserialize, PacketRead, PacketWrite, Serialize};
use std::io::{Read, Write};
use std::net::TcpStream;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, PacketRead)]
#[repr(i32)]
pub enum Response {
    ErrorDisconnect { reason: String } = 0x00,
    EncryptionRequest(EncryptionRequest) = 0x01,
    Success(LoginSuccess) = 0x02,
    SetCompression { threshold: VarInt } = 0x03,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct EncryptionRequest {
    pub server_id: String,
    pub public_key: Vec<u8>,
    pub verify_token: Vec<u8>,
    pub should_authenticate: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct LoginSuccess {
    pub player_uuid: Uuid,
    pub username: String,
    pub properties: Vec<Property>,
    pub strict_error_handling: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Property {
    pub name: String,
    pub value: String,
    pub signature: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, PacketWrite)]
#[packet_write(id = 0x00)]
pub struct LoginStart<'a> {
    pub username: &'a str,
    pub player_uuid: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, PacketWrite)]
#[packet_write(id = 0x01)]
pub struct EncryptionResponse<'a> {
    pub encrypted_shared_secret: &'a [u8],
    pub encrypted_verify_token: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, PacketWrite)]
#[packet_write(id = 0x03)]
pub struct LoginAcknowledged;

#[derive(Clone, Debug, serde::Deserialize)]
pub struct SessionInfo {
    #[serde(rename = "id")]
    pub player_uuid: Uuid,
    #[serde(rename = "name")]
    pub player_name: String,
    #[serde(rename = "skinUrl")]
    pub skin_url: String,
    #[serde(rename = "accessToken")]
    pub access_token: String,
}

#[derive(Clone, Copy, Debug, serde::Serialize)]
struct SessionJoinInfo<'a> {
    #[serde(rename = "accessToken")]
    pub access_token: &'a str,
    #[serde(rename = "selectedProfile")]
    pub player_uuid: &'a uuid::fmt::Simple,
    #[serde(rename = "serverId")]
    pub server_hash: &'a str,
}

pub fn login(
    address: &str,
    port: u16,
    client_information: configuration::ClientInformation,
    session_information: Option<&SessionInfo>,
) -> std::io::Result<(PlayConnection, LoginSuccess)> {
    use super::{configuration, handshaking};
    let tcp_stream = TcpStream::connect(format!("{address}:{port}"))?;
    let mut read_stream = &mut &tcp_stream as &mut dyn Read;
    let mut write_stream = &mut &tcp_stream as &mut dyn Write;
    // Send handshake packet
    handshaking::Handshake {
        protocol_version: PROTOCOL_VERSION,
        address,
        server_port: port,
        next_state: handshaking::HandshakeNextState::Login,
    }
    .write_packet_into(&mut write_stream, None)?;
    match session_information {
        Some(session) => LoginStart {
            username: &session.player_name,
            player_uuid: session.player_uuid,
        }
        .write_packet_into(&mut write_stream, None)?,
        None => LoginStart {
            username: "Sleepman",
            player_uuid: Uuid::new_v3(&OFFLINE_PLAYER_NAMESPACE, b"Sleepman"),
        }
        .write_packet_into(&mut write_stream, None)?,
    }
    write_stream.flush()?;
    let mut compression_threshold = None;
    let mut decryptor = None;
    let mut decrypting_reader;
    let mut encryptor = None;
    let mut encrypting_writer;
    // Login phase
    let success_packet = loop {
        match Response::read_from(compression_threshold, &mut read_stream)? {
            Response::Success(success_packet) => {
                LoginAcknowledged.write_packet_into(&mut write_stream, compression_threshold)?;
                break success_packet;
            }
            Response::EncryptionRequest(request_info) => {
                use cfb8::cipher::KeyIvInit;
                use rand::Rng;
                use rsa::pkcs8::DecodePublicKey;
                let mut thread_rng = rand::thread_rng();
                let pub_key = rsa::RsaPublicKey::from_public_key_der(&request_info.public_key)
                    .map_err(std::io::Error::other)?;
                let shared_secret: [u8; 16] = thread_rng.gen();
                // Send session information to session server
                if request_info.should_authenticate {
                    let (access_token, player_uuid) = session_information
                        .as_ref()
                        .map(|info| (info.access_token.as_str(), info.player_uuid.as_simple()))
                        .expect("session information required for authentication");
                    let server_hash = {
                        use num_bigint::BigInt;
                        use sha1::{Digest, Sha1};
                        let mut hasher = Sha1::new();
                        hasher.update(&request_info.server_id);
                        hasher.update(&shared_secret);
                        hasher.update(&request_info.public_key);
                        BigInt::from_signed_bytes_be(&hasher.finalize()).to_str_radix(16)
                    };
                    let server_hash = server_hash.as_str();
                    let join_info = SessionJoinInfo {
                        access_token,
                        player_uuid,
                        server_hash,
                    };
                    let auth_response = reqwest::blocking::Client::new()
                        .post("https://sessionserver.mojang.com/session/minecraft/join")
                        .json(&join_info)
                        .send()
                        .map_err(std::io::Error::other)?;
                    if !auth_response.status().is_success() {
                        let status = auth_response.status();
                        return Err(std::io::Error::other(format!(
                            "authentication failure - {status:?}: {}",
                            auth_response.text().unwrap_or_else(|_| format!("")),
                        )));
                    }
                }
                // Send encryption info to server
                let encrypted_shared_secret = pub_key
                    .encrypt(&mut thread_rng, rsa::Pkcs1v15Encrypt, &shared_secret)
                    .map_err(std::io::Error::other)?;
                let encrypted_verify_token = pub_key
                    .encrypt(
                        &mut thread_rng,
                        rsa::Pkcs1v15Encrypt,
                        &request_info.verify_token,
                    )
                    .map_err(std::io::Error::other)?;
                EncryptionResponse {
                    encrypted_shared_secret: &encrypted_shared_secret,
                    encrypted_verify_token: &encrypted_verify_token,
                }
                .write_packet_into(&mut write_stream, None)?;
                // Enable encryption
                decryptor = Some(Aes128Cfb8Dec::new(
                    &shared_secret.into(),
                    &shared_secret.into(),
                ));
                decrypting_reader = EncryptedTcpStreamReader {
                    tcp_stream: &tcp_stream,
                    decryptor: decryptor.as_mut().unwrap(),
                };
                encryptor = Some(Aes128Cfb8Enc::new(
                    &shared_secret.into(),
                    &shared_secret.into(),
                ));
                encrypting_writer = EncryptedTcpStreamWriter {
                    tcp_stream: &tcp_stream,
                    encryptor: encryptor.as_mut().unwrap(),
                    encryption_buffer: Vec::new(),
                };
                read_stream = &mut decrypting_reader as &mut dyn Read;
                write_stream = &mut encrypting_writer as &mut dyn Write;
            }
            Response::SetCompression { threshold } => {
                log::debug!("Compression threshold set to {}", threshold.0);
                compression_threshold = match threshold.0 {
                    ..=-1 => None,
                    0.. => Some(threshold.0 as usize),
                };
            }
            Response::ErrorDisconnect { reason } => {
                return Err(std::io::Error::other(reason));
            }
        }
    };
    // Configuration phase
    loop {
        use configuration::{ClientDataPack, ClientDataPacks, Response, ServerboundPluginMessage};
        match configuration::Response::read_from(compression_threshold, &mut read_stream)? {
            Response::ErrorDisconnect { reason } => {
                return Err(std::io::Error::other(format!("{reason:?}")))
            }
            Response::Finish => {
                client_information.write_packet_into(&mut write_stream, compression_threshold)?;
                configuration::AcknowledgeFinish
                    .write_packet_into(&mut write_stream, compression_threshold)?;
                return Ok((
                    PlayConnection::new(tcp_stream, compression_threshold, encryptor, decryptor),
                    success_packet,
                ));
            }
            Response::PluginMessage(message) => match &message.channel {
                chan if chan == &identifier!("minecraft:brand") => {
                    const CLIENT_BRAND: &str = "rust_minecraft_client";
                    let (extra_data, server_brand) =
                        String::deserialize(InputSpan::new(&message.data))
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
                    ServerboundPluginMessage {
                        channel: &identifier!("minecraft:brand"),
                        data: ProtocolRawSlice(client_brand_data.as_bytes()),
                    }
                    .write_packet_into(&mut write_stream, compression_threshold)?;
                }
                _ => log::warn!(
                    "Received plugin message from unknown channel while configuring: {message:?}"
                ),
            },
            Response::KeepAlive(id) => {
                configuration::KeepAliveResponse(id).serialize_into(&mut write_stream)?;
            }
            Response::Ping(id) => {
                configuration::Pong(id).serialize_into(&mut write_stream)?;
            }
            Response::RegistryData(registry_data) => {
                log::warn!(
                    "Server registry data handling currently unimplemented, ignoring for now"
                );
                log::debug!("Server registry data: {registry_data:?}");
            }
            Response::EnableFeatures(features) => {
                if &features != &[identifier!("minecraft:vanilla")] {
                    todo!("Support alternative features: {:?}", features);
                }
            }
            Response::UpdateTags(_registry_tags) => log::warn!(
                "Server registry tags data handling currently unimplemented, ignoring for now"
            ),
            Response::ServerDataPacks(data_packs) => {
                log::warn!(
                    "{}, {}",
                    "Server data pack checking currently unimplemented",
                    "responding as only having <minecraft:core>",
                );
                log::debug!("Server data packs: {data_packs:?}");
                ClientDataPacks(&[ClientDataPack {
                    namespace: "minecraft",
                    id: "core",
                    version: "1.21.1",
                }])
                .write_packet_into(&mut write_stream, compression_threshold)?;
            }
            other => todo!("Handle configuration packet: {other:?}"),
        }
    }
}
