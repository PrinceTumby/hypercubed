pub mod v763;
pub mod v765;

#[macro_use]
pub mod prelude {
    #[cfg(feature = "protocol_verbose")]
    pub use super::error_tree::ErrorTree;
    pub use super::{request_status, Deserialize, Serialize};
    pub use protocol_derive::{Deserialize, PacketRead, PacketWrite, Serialize};
    pub type IResult<I, O> = Result<(I, O), nom::Err<IErr<I>>>;

    #[cfg(not(feature = "protocol_verbose"))]
    pub type IErr<I> = nom::error::Error<I>;

    #[cfg(feature = "protocol_verbose")]
    pub type IErr<I> = ErrorTree<I>;
}

#[cfg(feature = "protocol_verbose")]
pub mod error_tree {
    use nom_supreme::error::GenericErrorTree;
    use std::error::Error;

    pub type ErrorTree<I> =
        GenericErrorTree<I, &'static [u8], &'static str, Box<dyn Error + Send + Sync + 'static>>;
}

pub const OFFLINE_PLAYER_NAMESPACE: Uuid = uuid!("071e6668-28ee-39de-8f51-f257ec5f77a9");

use prelude::IResult;
use std::io::prelude::*;
use std::net::TcpStream;
use uuid::{uuid, Uuid};

pub trait Deserialize: Sized {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self>;
}

pub trait Serialize: Sized {
    // TODO: Come up with better names for these
    fn serialize_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()>;

    fn serialize_into<O: std::io::Write>(self, output: &mut O) -> std::io::Result<()> {
        self.serialize_to(output)
    }
}

pub fn request_status(protocol_version: i32, address: &str, port: u16) -> std::io::Result<String> {
    // Protocol version 763 was the first one implemented, so we've just been using that for status
    // requests, as it's pretty much version independent.
    use v763::prelude::*;
    let mut stream = TcpStream::connect(format!("{address}:{port}"))?;
    // Send handshake packet
    v763::packet::handshaking::Handshake {
        protocol_version,
        address,
        server_port: port,
        next_state: v763::packet::handshaking::HandshakeNextState::Status,
    }
    .write_packet_into(&mut stream, None)?;
    v763::packet::status::StatusRequest.write_packet_into(&mut stream, None)?;
    stream.flush()?;
    Ok(v763::packet::status::StatusResponse::read_uncompressed_from(&mut stream)?.0)
}
