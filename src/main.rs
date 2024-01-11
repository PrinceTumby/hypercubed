#![warn(clippy::all)]

pub mod client;
pub mod protocol;
pub mod resource;
pub mod world;

use protocol::prelude::*;

fn main() -> anyhow::Result<()> {
    env_logger::init();
    println!("{}", request_status()?);
    let (mut connection, login_success_packet) = login()?;
    println!("{login_success_packet:?}");
    loop {
        use protocol::packet::play::Clientbound;
        match connection.read_packet()? {
            Clientbound::ErrorDisconnect { reason } => println!("Disconnected: {reason}"),
            Clientbound::BundleDelimiter => unreachable!(),
            Clientbound::LoginPlay(_packet) => println!("Login play: {{<skipped>}}"),
            Clientbound::UpdateRecipes(_recipes) => println!("Update recipes: [<skipped>]"),
            Clientbound::UpdateTags(_tags) => println!("Update tags: [<skipped>]"),
            Clientbound::DeclareCommands(_) => println!("Declare commands: [<todo>]"),
            Clientbound::UpdateRecipeBook(_) => println!("Update recipes: {{<skipped>}}"),
            Clientbound::ServerData(data) => println!("Server MOTD: {}", data.motd),
            Clientbound::ChunkDataAndUpdateLight(_) => {
                println!("Chunk data and light update: {{<skipped>}}")
            }
            other => println!("{other:?}"),
        }
    }
}

// fn main() -> anyhow::Result<()> {
//     env_logger::init();
//     // tracing_subscriber::fmt::fmt()
//     //     .with_max_level(tracing::Level::TRACE)
//     //     .pretty()
//     //     .init();
//     pollster::block_on(client::window_run())
// }

// TODO NEXT: Make a global palette:
// - Block registration order is in world/level/block/Blocks.java
// - IDs are sequential once all states for a block have been generated
