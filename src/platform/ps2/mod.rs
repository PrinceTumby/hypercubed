pub mod asm_helpers;
pub mod compiler_builtins;
pub mod dma;
// TODO: Merge `draw` into `display`
pub mod draw;
pub mod gif;
pub mod display;
pub mod gs;
pub mod heap;
pub mod interrupts;
pub mod libs;
pub mod net;
pub mod time;
pub mod unwinding;

use portable_std::*;
use crate::client::graphics as ps2_graphics;

pub mod exports {
    pub use super::{net, libs, time};
    #[allow(unused)]
    pub(crate) use super::{dbg, print, println, eprintln};
}

#[expect(unused)]
macro_rules! print {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        write!(&mut $crate::platform::ps2::SyscallWriter, $($arg)*).unwrap();
    }};
}

#[allow(unused)]
macro_rules! println {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        writeln!(&mut $crate::platform::ps2::SyscallWriter, $($arg)*).unwrap();
    }};
}

#[allow(unused)]
macro_rules! dbg {
    () => {
        $crate::platform::ps2::println!(
            "[{}:{}:{}]",
            ::core::file!(),
            ::core::line!(),
            ::core::column!(),
        )
    };
    ($val:expr $(,)?) => {
        match $val {
            tmp => {
                $crate::platform::ps2::println!(
                    "[{}:{}:{}] {} = {:#?}",
                    ::core::file!(),
                    ::core::line!(),
                    ::core::column!(),
                    ::core::stringify!($val),
                    &tmp,
                );
                tmp
            }
        }
    };
    ($($val:expr),+ $(,)?) => {
        ($($crate::dbg!($val)),+,)
    };
}

pub(crate) use {dbg, print, println, println as eprintln};

#[repr(C)]
#[derive(Clone, Copy)]
pub union QuadWord {
    pub qword: u128,
    pub dwords: [u64; 2],
    pub words: [u32; 4],
    pub hwords: [u16; 8],
    pub bytes: [u8; 16],
}

impl core::fmt::Debug for QuadWord {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        unsafe { write!(f, "QuadWord({:?})", self.dwords) }
    }
}

unsafe extern "C" {
    // #[link_name = "__heap_start"]
    // static HEAP_START: u8;
}

unsafe extern "C" {
    pub fn syscall_pcsx2_printf(fmt: *const core::ffi::c_char, ...);
}

#[derive(Clone, Copy)]
pub struct SyscallWriter;

impl core::fmt::Write for SyscallWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        unsafe {
            const CHUNK_SIZE: usize = 128;
            let mut buffer = [0u8; CHUNK_SIZE + 1];
            for chunk in s.as_bytes().chunks(128) {
                buffer[0..chunk.len()].copy_from_slice(chunk);
                buffer[chunk.len()] = 0;
                syscall_pcsx2_printf(c"%s".as_ptr(), &raw const buffer);
            }
        }
        Ok(())
    }
}

pub fn block_on<F: IntoFuture>(fut: F) -> F::Output {
    use core::task::{Context, Poll, Waker};
    let mut fut = core::pin::pin!(fut.into_future());
    // TODO:
    // - Write a simple Waker, so that we can halt the CPU if we're waiting on a task that can
    //   take interrupts.
    // - Should be able to just have a stack pinned AtomicBool or something, with a PhantomPinned.
    let mut context = Context::from_waker(Waker::noop());
    loop {
        match fut.as_mut().poll(&mut context) {
            Poll::Pending => {}
            Poll::Ready(value) => return value,
        }
        // HACK: Work around R5900 short loop bug.
        //       Should ensure the loop branch delay slot is left unfilled.
        unsafe {
            core::arch::asm!("nop");
        }
    }
}

#[repr(C)]
pub struct SysArgs {
    pub argc: u32,
    pub argv: [*const u8; 16],
    pub payload: [u8; 256],
}

#[repr(C)]
pub struct StartArgs {
    pub pid: u32,
    pub sys_args: SysArgs,
}

/// Main thread arguments, passed in by the kernel.
pub static mut SYS_ARGS: SysArgs = SysArgs {
    argc: 0,
    argv: [core::ptr::null(); 16],
    payload: [0; 256],
};

/// Arguments passed into `__start`, used by `ps2link`.
pub static mut START_ARGS_PTR: Option<&'static StartArgs> = None;

#[expect(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ps2_entrypoint() -> ! {
    // TODO:
    // - Make `window_run` take window and graphics state parameters if no_std, and skip init
    // - Call `window_run`, so we can start some gameplay!
    println!("Hello from Rust!");
    unsafe {
        // Initialise heap
        heap::init();
        // Initialise GIF DMA channel
        dma::initialise_channel(dma::Channel::Gif, None, dma::ChannelFlags::default());
        // Initialise GS, framebuffer, and Z buffer
        const WIDTH: u32 = 640;
        const HEIGHT: u32 = 512;
        let framebuffer = draw::Framebuffer::new(draw::FramebufferInitArgs {
            width: WIDTH,
            height: HEIGHT,
            pixel_format: display::PixelStorageMethod::Psm32,
            mask: 0,
            placement_preference: display::vram::Placement::PreferLowest,
        })
        .unwrap();
        let z_buffer = draw::ZBuffer::new(draw::ZBufferInitArgs {
            width: WIDTH,
            height: HEIGHT,
            pixel_format: display::PixelStorageMethod::PsmZ32,
            depth_test_method: gs::DepthTestMethod::GreaterThanOrEqual,
            mask: false,
            placement_preference: display::vram::Placement::PreferLowest,
        })
        .unwrap();
        // Initialise the screen and tie the first framebuffer to the read circuits
        display::initialise(&framebuffer, 0, 0, true);
        // Setup GS
        {
            use display::PixelStorageMethod;
            use gs::tag::*;
            use gs::*;
            let gif_packet = gif::Packet::packed_from_gs_reg_array([
                &SetFramebuffer1::new(&framebuffer),
                &SetZBuffer1::new(&z_buffer),
                &SetUseMainPrimitive(true),
                &SetXYOffset1 {
                    x: Fixed28P4::from_u32(2048 - (framebuffer.width() / 2)),
                    y: Fixed28P4::from_u32(2048 - (framebuffer.height() / 2)),
                },
                &SetScissor1::new(
                    0,
                    framebuffer.width() as u16 - 1,
                    0,
                    framebuffer.height() as u16 - 1,
                ),
                &SetDitheringEnabled(false),
                &SetColourClampingEnabled(true),
                &SetAlphaCorrection1(match framebuffer.pixel_format() {
                    PixelStorageMethod::Psm16 | PixelStorageMethod::Psm16S => {
                        AlphaCorrectionMode::Rgba16
                    }
                    _ => AlphaCorrectionMode::Rgba32,
                }),
                &Finished,
            ]);
            dma::send_packet_normal_and_wait_fast(dma::Channel::Gif, gif_packet.as_dma_packet());
            println!("Setup GS!");
        }
        // block_on(render_triangle(&framebuffer, &z_buffer));
        block_on(run_client_main(&framebuffer, &z_buffer)).unwrap();
        println!("Finished!");
        loop {
            core::arch::asm!("nop", "nop", "nop", "nop", "nop", "nop");
        }
    }
}

const SERVER_ADDRESS: &str = "192.168.68.121";
const SERVER_PORT: u16 = 25565;

pub async fn run_client_main(
    framebuffer: &draw::Framebuffer,
    z_buffer: &draw::ZBuffer,
) -> anyhow::Result<()> {
    use crate::protocol::{self, configuration};
    use crate::protocol::prelude::*;
    println!(
        "{}",
        request_status(PROTOCOL_VERSION, SERVER_ADDRESS, SERVER_PORT)?
    );
    let (server_connection, login_success_packet) = protocol::login::login(
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
    )
    .await?;
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
    };
    let event_loop = libs::winit::event_loop::EventLoop::new()?;
    let window = &libs::winit::window::Window {};
    let graphics_state = ps2_graphics::GraphicsState::new(window).await?;
    let (run_result, ()) = futures::join!(
        crate::client::window_run(
            todo!(),
            todo!(),
            todo!(),
            crate::client::WindowRunEmbeddedArgs {
                event_loop,
                window,
                graphics_state,
            },
        ),
        connection_task,
    );
    run_result
}

#[expect(clippy::missing_safety_doc)]
pub async unsafe fn render_triangle(
    framebuffer: &draw::Framebuffer,
    z_buffer: &draw::ZBuffer,
) -> ! {
    unsafe {
        let mut current_x_offset: u16 = 2048;
        loop {
            use gs::tag::*;
            use gs::*;
            let clear_rect_pixel_testing = {
                let mut out = SetPixelTesting1(0);
                out.set_alpha_testing_enabled(false);
                out.set_alpha_test_method(AlphaTestMethod::NotEqualAlphaRef);
                out.set_alpha_ref(0x00);
                out.set_alpha_fail_mode(AlphaFailMode::UpdateZBuffer);
                out.set_dest_alpha_testing_enabled(false);
                out.set_dest_alpha_pass_enabled(false);
                out.set_depth_test_enabled(false);
                out.set_depth_test_method(DepthTestMethod::AlwaysPass);
                out
            };
            let clear_rect_primitive = {
                let mut out = SetPrimitive(0);
                out.set_primitive_type(PrimitiveType::Triangle);
                out.set_gouraud_shading_enabled(false);
                out.set_texture_mapping_enabled(false);
                out.set_fog_enabled(false);
                out.set_alpha_blending_enabled(false);
                out.set_antialiasing_enabled(false);
                out.set_use_uv_coords(false);
                out.set_use_context_2_regs(false);
                out.set_fix_fragment_value_enabled(false);
                out
            };
            let pixel_testing = {
                let mut out = SetPixelTesting1(0);
                out.set_alpha_testing_enabled(true);
                out.set_alpha_test_method(AlphaTestMethod::NotEqualAlphaRef);
                out.set_alpha_ref(0x00);
                out.set_alpha_fail_mode(AlphaFailMode::UpdateZBuffer);
                out.set_dest_alpha_testing_enabled(false);
                out.set_dest_alpha_pass_enabled(false);
                out.set_depth_test_enabled(true);
                out.set_depth_test_method(z_buffer.depth_test_method);
                out
            };
            let primitive = {
                let mut out = SetPrimitive(0);
                out.set_primitive_type(PrimitiveType::Triangle);
                out.set_gouraud_shading_enabled(true);
                out.set_texture_mapping_enabled(false);
                out.set_fog_enabled(false);
                out.set_alpha_blending_enabled(false);
                out.set_antialiasing_enabled(false);
                out.set_use_uv_coords(false);
                out.set_use_context_2_regs(false);
                out.set_fix_fragment_value_enabled(false);
                out
            };
            let fb_width = framebuffer.width() as u16;
            let fb_height = framebuffer.height() as u16;
            let x_offset = {
                let offset = current_x_offset;
                current_x_offset += 1;
                if current_x_offset > 2248 {
                    current_x_offset = 1848;
                }
                offset
            };
            const Y_OFFSET: u16 = 2048;
            const SCALE: u16 = 200;
            let gif_packet = gif::Packet::packed_from_gs_reg_array([
                &clear_rect_pixel_testing,
                &clear_rect_primitive,
                &SetRGBAQ {
                    r: 0x00,
                    g: 0x00,
                    b: 0x00,
                    a: 0xFF,
                    q: 0x00,
                },
                &KickVertexXYZ2 {
                    x: Fixed12P4::from_u16(2048 - (fb_width / 2)),
                    y: Fixed12P4::from_u16(2048 - (fb_height / 2)),
                    z: 0,
                },
                &KickVertexXYZ2 {
                    x: Fixed12P4::from_u16(2048 + (fb_width / 2) - 1),
                    y: Fixed12P4::from_u16(2048 - (fb_height / 2)),
                    z: 0,
                },
                &KickVertexXYZ2 {
                    x: Fixed12P4::from_u16(2048 - (fb_width / 2)),
                    y: Fixed12P4::from_u16(2048 + (fb_height / 2) - 1),
                    z: 0,
                },
                &KickVertexXYZ2 {
                    x: Fixed12P4::from_u16(2048 + (fb_width / 2) - 1),
                    y: Fixed12P4::from_u16(2048 - (fb_height / 2)),
                    z: 0,
                },
                &KickVertexXYZ2 {
                    x: Fixed12P4::from_u16(2048 - (fb_width / 2)),
                    y: Fixed12P4::from_u16(2048 + (fb_height / 2) - 1),
                    z: 0,
                },
                &KickVertexXYZ2 {
                    x: Fixed12P4::from_u16(2048 + (fb_width / 2) - 1),
                    y: Fixed12P4::from_u16(2048 + (fb_height / 2) - 1),
                    z: 0,
                },
                &pixel_testing,
                &primitive,
                &SetRGBAQ {
                    r: 0xFF,
                    g: 0x00,
                    b: 0x00,
                    a: 0xFF,
                    q: 0x00,
                },
                &KickVertexXYZ2 {
                    x: Fixed12P4::from_u16(x_offset - (SCALE / 2)),
                    y: Fixed12P4::from_u16((SCALE / 2) + Y_OFFSET),
                    z: 0,
                },
                &SetRGBAQ {
                    r: 0x00,
                    g: 0xFF,
                    b: 0x00,
                    a: 0xFF,
                    q: 0x00,
                },
                &KickVertexXYZ2 {
                    x: Fixed12P4::from_u16(x_offset),
                    y: Fixed12P4::from_u16(Y_OFFSET - (SCALE / 2)),
                    z: 0,
                },
                &SetRGBAQ {
                    r: 0x00,
                    g: 0x00,
                    b: 0xFF,
                    a: 0xFF,
                    q: 0x00,
                },
                &KickVertexXYZ2 {
                    x: Fixed12P4::from_u16((SCALE / 2) + x_offset),
                    y: Fixed12P4::from_u16((SCALE / 2) + Y_OFFSET),
                    z: 0,
                },
                &Finished,
            ]);
            futures::join!(
                dma::send_packet_async(dma::Channel::Gif, gif_packet.as_dma_packet()),
                display::gs::csr::wait_for_drawing_finished_async(),
                display::gs::csr::wait_for_vsync_async(),
            );
            core::arch::asm!("nop", "nop", "nop", "nop");
        }
    }
}
