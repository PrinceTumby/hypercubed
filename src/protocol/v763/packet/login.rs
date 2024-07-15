use super::super::prelude::*;
use nom::Parser;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, PacketRead)]
pub enum Response {
    Success(LoginSuccess),
    SetCompression { threshold: VarInt },
    ErrorDisconnect { reason: String },
}

impl Deserialize for Response {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        // TODO Same as with packet::status::Response
        var_int_tagged_parser!(
            0x00 => String::deserialize.map(|reason| Self::ErrorDisconnect { reason }),
            0x03 => VarInt::deserialize.map(|threshold| Self::SetCompression { threshold }),
            0x02 => LoginSuccess::deserialize.map(Self::Success),
        )(input)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, PacketWrite)]
#[packet_write(id = 0x00)]
pub struct LoginStart<'a> {
    pub username: &'a str,
    pub player_uuid: Option<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
// #[packet_read(id = 0x02)]
pub struct LoginSuccess {
    pub player_uuid: Uuid,
    pub username: String,
    pub properties: Vec<Property>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Property {
    pub name: String,
    pub value: String,
    pub signature: Option<String>,
}
