use super::prelude::*;
use super::PluginMessage;
use protocol_derive::{Deserialize, PacketRead, PacketWrite, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, PacketRead)]
#[repr(i32)]
pub enum Response {
    ErrorDisconnect { reason: TextComponent } = 0x02,
    PluginMessage(PluginMessage) = 0x01,
    Finish = 0x03,
    KeepAlive(u64) = 0x04,
    Ping(u32) = 0x05,
    ResetChat = 0x06,
    RegistryData(RegistryData) = 0x07,
    RemoveResourcePack(Option<Uuid>) = 0x08,
    AddResourcePack(ResourcePackInfo) = 0x09,
    EnableFeatures(Vec<Identifier>) = 0x0C,
    UpdateTags(Vec<RegistryTags>) = 0x0D,
    ServerDataPacks(Vec<ServerDataPack>) = 0x0E,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct RegistryData {
    pub id: Identifier,
    pub entries: Vec<RegistryEntry>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct RegistryEntry {
    pub id: Identifier,
    pub data: Option<NetworkNbtCompound>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ResourcePackInfo {
    pub pack_uuid: Uuid,
    pub url: String,
    pub hash: String,
    pub forced: bool,
    pub prompt_message: TextComponent,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Property {
    pub name: String,
    pub value: String,
    pub signature: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct RegistryTags {
    pub registry: Identifier,
    pub tags: Vec<Tag>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Tag {
    pub name: Identifier,
    pub ids: Vec<VarInt>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ServerDataPack {
    pub namespace: String,
    pub id: String,
    pub version: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, PacketWrite)]
#[packet_write(id = 0x00)]
pub struct ClientInformation<'a> {
    pub locale: &'a str,
    pub view_distance: u8,
    pub chat_mode: ChatMode,
    pub chat_colors_enabled: bool,
    pub displayed_skin_parts: u8,
    pub main_hand: MainHand,
    pub text_filtering_enabled: bool,
    pub server_listings_allowed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ChatMode {
    Enabled = 0,
    CommandsOnly = 1,
    Hidden = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum MainHand {
    Left = 0,
    Right = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, PacketWrite)]
#[packet_write(id = 0x02)]
pub struct ServerboundPluginMessage<'a> {
    pub channel: &'a Identifier,
    pub data: ProtocolRawSlice<'a, u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, PacketWrite)]
#[packet_write(id = 0x03)]
pub struct AcknowledgeFinish;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, PacketWrite)]
#[packet_write(id = 0x04)]
pub struct KeepAliveResponse(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, PacketWrite)]
#[packet_write(id = 0x05)]
pub struct Pong(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, PacketWrite)]
#[packet_write(id = 0x06)]
pub struct ResourcePackResponse {
    pub uuid: Uuid,
    pub result: ResourcePackResponseResult,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ResourcePackResponseResult {
    DownloadSuccessful = 0,
    Declined = 1,
    DownloadFailed = 2,
    Accepted = 3,
    Downloaded = 4,
    InvalidUrl = 5,
    ReloadFailed = 6,
    Discarded = 7,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, PacketWrite)]
#[repr(transparent)]
#[packet_write(id = 0x07)]
pub struct ClientDataPacks<'a>(pub &'a [ClientDataPack<'a>]);

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ClientDataPack<'a> {
    pub namespace: &'a str,
    pub id: &'a str,
    pub version: &'a str,
}
