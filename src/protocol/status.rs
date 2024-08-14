use super::prelude::*;
use protocol_derive::{Deserialize, PacketRead, PacketWrite, Serialize};

#[derive(Clone, Debug, Deserialize, PacketRead, PartialEq, Eq)]
#[repr(i32)]
pub enum Response {
    Status(String) = 0x00,
    Ping(u64) = 0x01,
    // TODO Replace this with a dedicated Chat type
    ErrorDisconnect { reason: String } = 0x1A,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, PacketWrite)]
#[packet_write(id = 0x00)]
pub struct StatusRequest;
