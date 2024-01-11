use super::super::prelude::*;
use nom::Parser;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Response {
    Status(StatusResponse),
    Ping(PingRequestResponse),
    // TODO Replace this with a dedicated Chat type
    ErrorDisconnect { reason: String },
}

// TODO Move this into the Deserialize derive macro
impl Deserialize for Response {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        // TODO Change PacketRead trait to not have an ID property, derive macro will just generate
        // an implementation instead. Makes below todo easier.
        // TODO Update this to use the IDs from the traits.
        var_int_tagged_parser!(
            0x00 => StatusResponse::deserialize.map(Self::Status),
            0x01 => PingRequestResponse::deserialize.map(Self::Ping),
            0x1A => String::deserialize.map(|reason| Self::ErrorDisconnect { reason }),
        )(input)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, PacketWrite)]
#[packet_write(id = 0x00)]
pub struct StatusRequest;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, PacketRead)]
#[packet_read(id = 0x00)]
#[repr(transparent)]
pub struct StatusResponse(pub String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, PacketWrite)]
#[packet_write(id = 0x01)]
// #[packet_read(id = 0x01)]
#[repr(transparent)]
pub struct PingRequestResponse {
    pub payload: u64,
}
