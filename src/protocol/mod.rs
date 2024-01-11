pub mod v763;
pub mod v765;

#[macro_use]
pub mod prelude {
    #[cfg(feature = "protocol_verbose")]
    pub use super::error_tree::ErrorTree;
    pub use super::{Deserialize, Serialize};
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

use prelude::IResult;
use uuid::{uuid, Uuid};

const OFFLINE_PLAYER_NAMESPACE: Uuid = uuid!("071e6668-28ee-39de-8f51-f257ec5f77a9");

pub trait Deserialize: Sized {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self>;
}

pub trait Serialize: Sized {
    // TODO Come up with better names for these methods
    fn serialize_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()>;

    fn serialize_into<O: std::io::Write>(self, output: &mut O) -> std::io::Result<()> {
        self.serialize_to(output)
    }
}
