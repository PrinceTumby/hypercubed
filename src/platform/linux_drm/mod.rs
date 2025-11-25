#[cfg(not(feature = "graphics_backend_opengl"))]
compile_error!("The Linux DRM platform currently only supports the OpenGL graphics backend.");

pub mod libs;

pub mod exports {
    pub use super::{DnsResolver, TcpStack, libs, main};
}

use anyhow::{Context, bail};
use clap::Parser;
use portable_std::*;

static DEFAULT_SERVER_ADDRESS: &str = "127.0.0.1:25565";
const DEFAULT_PORT: u16 = 25565;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(default_value_t = String::from(DEFAULT_SERVER_ADDRESS))]
    server_address: String,
}

pub fn main() -> anyhow::Result<()> {
    use crate::protocol::{self, configuration};
    env_logger::init();
    #[cfg(feature = "tracy")]
    {
        use tracing_subscriber::layer::SubscriberExt;
        tracing::subscriber::set_global_default(
            tracing_subscriber::registry().with(tracing_tracy::TracyLayer::default()),
        )
        .unwrap();
    }
    let args = Args::parse();
    let server_socket_address = 'blk: {
        use core::net::{IpAddr, SocketAddr};
        // First, try parsing as an "IP:PORT" socket address.
        let server_socket_address: Result<SocketAddr, _> = args.server_address.parse();
        match server_socket_address {
            Ok(address) => break 'blk address,
            Err(_) => {}
        }
        // If it doesn't parse correctly as a full socket address, try parsing as an IP address.
        let server_ip_address: Result<IpAddr, _> = args.server_address.parse();
        match server_ip_address {
            Ok(ip) => break 'blk SocketAddr::new(ip, DEFAULT_PORT),
            Err(_) => {}
        }
        bail!("Unable to parse server address as either a socket address, or as an IP address");
    };
    let server_ip_string = server_socket_address.ip().to_string();
    let server_port = server_socket_address.port();
    let session_info: Option<protocol::login::SessionInfo> =
        std::fs::read_to_string("session.json")
            .map(|session_json| serde_json::from_str(&session_json).unwrap())
            .ok();
    let (server_connection, login_success_packet) = pollster::block_on(protocol::login::login(
        &server_ip_string,
        server_port,
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
    let event_loop =
        libs::winit::event_loop::EventLoop::new().context("Error while creating event loop")?;
    let window =
        libs::winit::window::Window::new(&event_loop).context("Error while creating window")?;
    let graphics_state = pollster::block_on(crate::client::graphics::GraphicsState::new(
        window.clone(),
        resources::block::register_vanilla_blocks,
    ))?;
    pollster::block_on(crate::client::window_run(
        server_connection,
        clientbound_rx,
        clientbound_tx,
        event_loop,
        window,
        graphics_state,
    ))?;
    Ok(())
}

use std::io::{Read, Write};

pub struct TcpStreamWrapper(pub std::net::TcpStream);

impl embedded_io::ErrorType for TcpStreamWrapper {
    type Error = std::io::Error;
}

impl embedded_io_async::Read for TcpStreamWrapper {
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read_exact(buf)?;
        Ok(buf.len())
    }
}

impl embedded_io_async::Write for TcpStreamWrapper {
    async fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write_all(buf)?;
        Ok(buf.len())
    }

    async fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

pub struct TcpStack;

impl embedded_nal_async::TcpConnect for TcpStack {
    type Error = std::io::Error;
    type Connection<'a> = TcpStreamWrapper;

    async fn connect<'a>(
        &'a self,
        remote: core::net::SocketAddr,
    ) -> Result<Self::Connection<'a>, Self::Error> {
        std::net::TcpStream::connect(remote).map(TcpStreamWrapper)
    }
}

pub struct DnsResolver;

impl embedded_nal_async::Dns for DnsResolver {
    type Error = std::io::Error;

    async fn get_host_by_name(
        &self,
        host: &str,
        addr_type: embedded_nal_async::AddrType,
    ) -> Result<core::net::IpAddr, Self::Error> {
        assert_eq!(addr_type, embedded_nal_async::AddrType::Either);
        std::net::ToSocketAddrs::to_socket_addrs(&(host, 443))?
            .next()
            .ok_or_else(|| std::io::Error::other("host not found"))
            .map(|socket_addr| socket_addr.ip())
    }

    async fn get_host_by_address(
        &self,
        _addr: core::net::IpAddr,
        _result: &mut [u8],
    ) -> Result<usize, Self::Error> {
        Err(std::io::Error::other("unimplemented"))
    }
}
