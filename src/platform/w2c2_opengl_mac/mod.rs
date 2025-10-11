pub mod libs;
pub mod net;

pub use std::time;

use crate::client::graphics as mac_graphics;
use futures::FutureExt;
use futures::future::BoxFuture;
use portable_std::*;

pub mod exports {
    pub use super::{libs, net, time};
    #[allow(unused)]
    pub(crate) use std::{dbg, eprintln, print, println};
}

// The host can have different endianness to the WebAssembly environment, so we define host-endian
// number types here.
// Should be used whenever a number is used externally by pointer, rather than by argument.

macro_rules! define_host_endian_number_type {
    ($name:ident, $num_type:ty, $alignment:expr) => {
        #[repr(C, align($alignment))]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub struct $name([u8; const { core::mem::size_of::<$num_type>() }]);

        impl $name {
            pub const fn new(x: $num_type) -> Self {
                cfg_if::cfg_if! {
                    if #[cfg(feature = "subplatform_w2c2_opengl_mac_ppc")] {
                        Self(x.to_be_bytes())
                    } else {
                        Self(x.to_le_bytes())
                    }
                }
            }

            pub const fn to_num(self) -> $num_type {
                cfg_if::cfg_if! {
                    if #[cfg(feature = "subplatform_w2c2_opengl_mac_ppc")] {
                        <$num_type>::from_be_bytes(self.0)
                    } else {
                        <$num_type>::from_le_bytes(self.0)
                    }
                }
            }
        }

        impl From<$num_type> for $name {
            fn from(x: $num_type) -> Self {
                Self::new(x)
            }
        }

        impl From<$name> for $num_type {
            fn from(x: $name) -> Self {
                x.to_num()
            }
        }
    };
}

define_host_endian_number_type!(HostU16, u16, 2);
define_host_endian_number_type!(HostU32, u32, 4);
define_host_endian_number_type!(HostF32, f32, 4);

const SERVER_ADDRESS: &str = "192.168.137.1";
const SERVER_PORT: u16 = 25565;

// TODO: Currently this uses async to fake coroutines, but it would be nicer if we could just
// use actual coroutines.
static CLIENT_RUN_TASK: Mutex<Option<BoxFuture<'static, anyhow::Result<()>>>> = Mutex::new(None);
static CONNECTION_TASK: Mutex<Option<BoxFuture<'static, ()>>> = Mutex::new(None);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn client_process_events() -> bool {
    use core::task::{Context, Poll, Waker};
    let mut context = Context::from_waker(Waker::noop());
    // Client task
    {
        let mut client_run_task_lock = CLIENT_RUN_TASK.lock().unwrap();
        let Some(task) = client_run_task_lock.as_mut() else {
            // Report that client has finished running (or that it never started).
            return false;
        };
        libs::winit::event_loop::CURRENT_LOOP_COUNT
            .fetch_add(1, core::sync::atomic::Ordering::SeqCst);
        match task.poll_unpin(&mut context) {
            Poll::Pending => {}
            Poll::Ready(result) => {
                result.unwrap();
                return false;
            }
        }
    }
    // Connection task
    'conn_task: {
        let mut connection_task_lock = CONNECTION_TASK.lock().unwrap();
        let Some(task) = connection_task_lock.as_mut() else {
            break 'conn_task;
        };
        match task.poll_unpin(&mut context) {
            Poll::Pending => {}
            Poll::Ready(()) => *connection_task_lock = None,
        }
    }
    // Report client's still running.
    true
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn client_initialise(width: u32, height: u32) {
    use crate::protocol::prelude::*;
    use crate::protocol::{self, configuration};
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
        // TODO: Session info
        None,
    ))
    .unwrap();
    println!("{login_success_packet:?}");
    // TODO: Refactor this:
    // - Change PlayConnection to have an internal thread sending packets on a channel
    // - Make read_packet pull from the internal channel receiver
    // Then we no longer need an Arc<PlayConnection> with a confusing read_packet method, and we
    // can more easily make a try_read_packet method
    let server_connection = portable_std::Arc::new(server_connection);
    let (clientbound_tx, clientbound_rx) = portable_std::sync::mpsc::channel();
    let connection_task = {
        let clientbound_tx = clientbound_tx.clone();
        let server_connection = server_connection.clone();
        async move {
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
        }
        .boxed()
    };
    *CONNECTION_TASK.lock().unwrap() = Some(connection_task);
    let event_loop = libs::winit::event_loop::EventLoop::new().unwrap();
    let window = &libs::winit::window::Window {};
    let graphics_state = mac_graphics::GraphicsState::new(width, height).unwrap();
    let client_run_task = crate::client::window_run(
        server_connection,
        clientbound_rx,
        clientbound_tx,
        crate::client::WindowRunEmbeddedArgs {
            event_loop,
            window,
            graphics_state,
        },
    )
    .boxed();
    *CLIENT_RUN_TASK.lock().unwrap() = Some(client_run_task);
}
