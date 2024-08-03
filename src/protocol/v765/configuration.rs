use super::prelude::*;
use nom::Parser;
use uuid::Uuid;

#[derive(Clone, Debug, PacketRead)]
pub enum Response {
    PluginMessage(PluginMessage),
    Finish,
    KeepAlive(u64),
    Ping(u32),
    RegistryData(NetworkNbtCompound),
    RemoveResourcePack(Option<Uuid>),
    AddResourcePack(ResourcePackInfo),
    EnableFeatures(Vec<Identifier>),
    UpdateTags(Vec<RegistryTags>),
    ErrorDisconnect { reason: String },
}

impl Deserialize for Response {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        // TODO Same as with packet::status::Response
        var_int_tagged_parser!(
            0x00 => PluginMessage::deserialize.map(Self::PluginMessage),
            0x01 => String::deserialize.map(|reason| Self::ErrorDisconnect { reason }),
            0x03 => u64::deserialize.map(Self::KeepAlive),
            0x04 => u32::deserialize.map(Self::Ping),
            0x05 => NetworkNbtCompound::deserialize.map(Self::RegistryData),
            0x06 => <Option<Uuid>>::deserialize.map(Self::RemoveResourcePack),
            0x07 => ResourcePackInfo::deserialize.map(Self::AddResourcePack),
            0x08 => <Vec<Identifier>>::deserialize.map(Self::EnableFeatures),
            0x09 => <Vec<RegistryTags>>::deserialize.map(Self::UpdateTags),
            0x02 => <()>::deserialize.map(|()| Self::Finish),
        )(input)
    }
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
#[packet_write(id = 0x01)]
pub struct ServerboundPluginMessage<'a> {
    // TODO Change this to Identifier
    pub channel: &'a str,
    pub data: ProtocolRawSlice<'a, u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, PacketWrite)]
#[packet_write(id = 0x02)]
pub struct AcknowledgeFinish;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, PacketWrite)]
#[packet_write(id = 0x03)]
pub struct KeepAliveResponse(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, PacketWrite)]
#[packet_write(id = 0x04)]
pub struct Pong(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, PacketWrite)]
#[packet_write(id = 0x05)]
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

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ResourcePackInfo {
    pub pack_uuid: Uuid,
    pub url: String,
    pub hash: String,
    pub forced: bool,
    pub prompt_message: Chat,
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
