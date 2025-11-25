use crate::client;
use crate::client::graphics::GraphicsState;
use crate::protocol;
use crate::protocol::prelude::*;
use portable_std::Arc;

pub mod exports {
    pub use super::{libs, main};
}

pub mod libs {
    pub use {egui, winit};
}

const SERVER_ADDRESS: &str = "localhost";
const SERVER_PORT: u16 = 25565;

pub fn main() -> anyhow::Result<()> {
    use protocol::configuration;
    env_logger::init();
    #[cfg(feature = "tracy")]
    {
        use tracing_subscriber::layer::SubscriberExt;
        tracing::subscriber::set_global_default(
            tracing_subscriber::registry().with(tracing_tracy::TracyLayer::default()),
        )
        .unwrap();
    }
    println!(
        "{}",
        request_status(PROTOCOL_VERSION, SERVER_ADDRESS, SERVER_PORT)?
    );
    let session_info: Option<protocol::login::SessionInfo> =
        std::fs::read_to_string("session.json")
            .map(|session_json| serde_json::from_str(&session_json).unwrap())
            .ok();
    let (server_connection, login_success_packet) = pollster::block_on(protocol::login::login(
        SERVER_ADDRESS,
        SERVER_PORT,
        configuration::ClientInformation {
            locale: "en_GB",
            view_distance: 1,
            chat_mode: configuration::ChatMode::Enabled,
            chat_colors_enabled: true,
            displayed_skin_parts: 0x7F,
            main_hand: configuration::MainHand::Right,
            text_filtering_enabled: false,
            server_listings_allowed: true,
        },
        session_info.as_ref(),
    ))?;
    println!("{login_success_packet:?}");
    // TODO: Refactor this:
    // - Change PlayConnection to have an internal thread sending packets on a channel
    // - Make read_packet pull from the internal channel receiver
    // Then we no longer need an Arc<PlayConnection> with a confusing read_packet method, and we
    // can more easily make a try_read_packet method
    let server_connection = Arc::new(server_connection);
    let (clientbound_tx, clientbound_rx) = std::sync::mpsc::channel();
    {
        let clientbound_tx = clientbound_tx.clone();
        let server_connection = server_connection.clone();
        std::thread::spawn(move || {
            loop {
                let packet = match server_connection.read_packet() {
                    Ok(packet) => packet,
                    Err(err) => {
                        eprintln!("Closing connection thread - {err}");
                        break;
                    }
                };
                if clientbound_tx.send(packet).is_err() {
                    break;
                };
            }
        });
    }
    let event_loop = winit::event_loop::EventLoop::new()?;
    let window = Arc::new(winit::window::WindowBuilder::new().build(&event_loop)?);
    pollster::block_on(async move {
        let graphics_state =
            GraphicsState::new(window.clone(), resources::block::register_vanilla_blocks).await?;
        client::window_run(
            server_connection,
            clientbound_rx,
            clientbound_tx,
            event_loop,
            window,
            graphics_state,
        )
        .await
    })?;
    Ok(())
}
