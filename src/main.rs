#![warn(clippy::all)]

pub mod basic_types;
pub mod client;
pub mod protocol;
pub mod resource;
pub mod world;

use protocol::v765::prelude::*;
use std::sync::Arc;

#[global_allocator]
static GLOBAL_ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

const SERVER_ADDRESS: &str = "localhost";
const SERVER_PORT: u16 = 25565;

fn main() -> anyhow::Result<()> {
    use protocol::v765::configuration;
    env_logger::init();
    #[cfg(feature = "tracy")]
    {
        use tracing_subscriber::layer::SubscriberExt;
        tracing::subscriber::set_global_default(
            tracing_subscriber::registry().with(tracing_tracy::TracyLayer::default()),
        )
        .unwrap();
    }
    println!("{}", request_status(765, SERVER_ADDRESS, SERVER_PORT)?);
    let session_info: Option<login::SessionInfo> = std::fs::read_to_string("session.json")
        .map(|session_json| serde_json::from_str(&session_json).unwrap())
        .ok();
    let (server_connection, login_success_packet) = login::login(
        SERVER_ADDRESS,
        SERVER_PORT,
        configuration::ClientInformation {
            locale: "en_GB",
            view_distance: 8,
            chat_mode: configuration::ChatMode::Enabled,
            chat_colors_enabled: true,
            displayed_skin_parts: 0x7F,
            main_hand: configuration::MainHand::Right,
            text_filtering_enabled: false,
            server_listings_allowed: true,
        },
        session_info.as_ref(),
    )?;
    println!("{login_success_packet:?}");
    // TODO: Refactor this:
    // - Change PlayConnection to have an internal thread sending packets on a channel
    // - Make read_packet pull from the internal channel receiver
    // Then we no longer need an Arc<PlayConnection> with a confusing read_packet method, and we
    // can more easily make a try_read_packet method
    let server_connection = Arc::new(server_connection);
    let (clientbound_tx, clientbound_rx) = std::sync::mpsc::channel();
    {
        let server_connection = server_connection.clone();
        std::thread::spawn(move || loop {
            let packet = server_connection
                .read_packet()
                .map_err(|err| format!("{err:.02X?}"))
                .unwrap();
            if let Err(_) = clientbound_tx.send(packet) {
                break;
            };
        });
    }
    pollster::block_on(client::window_run(server_connection, clientbound_rx))?;
    Ok(())
}
