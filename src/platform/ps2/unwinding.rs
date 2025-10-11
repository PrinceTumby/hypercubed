use crate::portable_prelude::*;
use core::arch::asm;

#[expect(unused)]
const MAX_FRAMES: usize = 64;

#[inline(never)]
fn print_stack_trace() {
    eprintln!("TODO: Stack trace printing");
}

static mut DISABLE_TRACE_LOGGING: bool = false;
static mut PANIC_DEPTH: usize = 0;

#[inline(never)]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    unsafe {
        eprintln!("{info}");
        let current_panic_depth = PANIC_DEPTH;
        PANIC_DEPTH += 1;
        match current_panic_depth {
            0 => {}
            1 => DISABLE_TRACE_LOGGING = true,
            _ => loop {
                asm!("nop", "nop", "nop", "nop", "nop", "nop",);
            },
        }
        if !DISABLE_TRACE_LOGGING {
            print_stack_trace();
        }
        loop {
            asm!("nop", "nop", "nop", "nop", "nop", "nop",);
        }
    }
}
