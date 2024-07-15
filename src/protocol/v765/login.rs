pub use crate::protocol::v763::packet::login::{LoginSuccess, Property, Response};

use super::prelude::*;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, PacketWrite)]
#[packet_write(id = 0x00)]
pub struct LoginStart<'a> {
    pub username: &'a str,
    pub player_uuid: Uuid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, PacketWrite)]
#[packet_write(id = 0x03)]
pub struct LoginAcknowledged;
