pub mod app;
pub mod heap;
pub mod libs;
pub mod net;
pub mod time;

use crate::client::graphics as mac_graphics;
use core::ffi::c_int;
use portable_std::*;

pub mod exports {
    #[allow(unused)]
    pub(crate) use super::{dbg, eprintln, print, println};
    pub use super::{libs, net, time};
}

unsafe extern "C" {
    pub unsafe fn printf(fmt: *const core::ffi::c_char, ...);
    pub unsafe fn abort() -> !;
}

#[derive(Clone, Copy)]
pub struct PrintfWriter;

impl core::fmt::Write for PrintfWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        unsafe {
            const CHUNK_SIZE: usize = 128;
            let mut buffer = [0u8; CHUNK_SIZE + 1];
            for chunk in s.as_bytes().chunks(CHUNK_SIZE) {
                buffer[0..chunk.len()].copy_from_slice(chunk);
                buffer[chunk.len()] = 0;
                printf(c"%s".as_ptr(), &raw const buffer);
            }
        }
        Ok(())
    }
}

#[expect(unused)]
macro_rules! print {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        write!(&mut $crate::platform::opengl_mac_tiger::PrintfWriter, $($arg)*).unwrap();
    }};
}

#[allow(unused)]
macro_rules! println {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        writeln!(&mut $crate::platform::opengl_mac_tiger::PrintfWriter, $($arg)*).unwrap();
    }};
}

#[allow(unused)]
macro_rules! dbg {
    () => {
        $crate::platform::opengl_mac_tiger::println!(
            "[{}:{}:{}]",
            ::core::file!(),
            ::core::line!(),
            ::core::column!(),
        )
    };
    ($val:expr $(,)?) => {
        match $val {
            tmp => {
                $crate::platform::opengl_mac_tiger::println!(
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
        ($($crate::platform::opengl_mac_tiger::dbg!($val)),+,)
    };
}

pub(crate) use {dbg, print, println, println as eprintln};

// const SERVER_ADDRESS: &str = "192.168.137.1";
// const SERVER_PORT: u16 = 25565;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn main(_argc: c_int, _argv: *const *const u8) -> c_int {
    unsafe {
        app::run();
        0
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    unsafe {
        static mut DISABLE_TRACE_LOGGING: bool = false;
        static mut PANIC_DEPTH: usize = 0;
        eprintln!("{info}");
        let current_panic_depth = PANIC_DEPTH;
        PANIC_DEPTH += 1;
        match current_panic_depth {
            0 => {}
            1 => DISABLE_TRACE_LOGGING = true,
            _ => abort(),
        }
        // if !DISABLE_TRACE_LOGGING {
        //     print_stack_trace();
        // }
        abort();
    }
}
