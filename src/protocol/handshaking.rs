use super::prelude::*;
use protocol_derive::PacketWrite;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PacketWrite)]
#[packet_write(id = 0x00)]
pub struct Handshake<'a> {
    pub protocol_version: i32,
    pub address: &'a str,
    pub server_port: u16,
    pub next_state: HandshakeNextState,
}

impl Serialize for Handshake<'_> {
    fn serialize_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        VarInt(self.protocol_version).serialize_to(writer)?;
        self.address.serialize_to(writer)?;
        self.server_port.serialize_to(writer)?;
        self.next_state.serialize_to(writer)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandshakeNextState {
    Status = 1,
    Login = 2,
}

impl Serialize for HandshakeNextState {
    fn serialize_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        VarInt(*self as i32).serialize_into(writer)
    }
}
