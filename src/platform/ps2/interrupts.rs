use super::asm_helpers;

pub use asm_helpers::{disable_interrupts as disable, enable_interrupts as enable};

pub type InterruptHandler = extern "C" fn(cause: i32) -> i32;

#[repr(transparent)]
pub struct HandlerId(i32);

#[repr(i32)]
pub enum InterruptType {
    Gs = 0,
    VBlankStart = 2,
    VBlankEnd = 3,
    Vif0 = 4,
    Vif1 = 5,
    Vu0 = 6,
    Vu1 = 7,
    Ipu = 8,
    Timer0 = 9,
    Timer1 = 10,
    Timer2 = 11,
    SFifo = 13,
    Vu0Watchdog = 14,
}

pub use syscalls::add_interrupt_handler as add_handler;

mod syscalls {
    use super::*;

    unsafe extern "C" {
        #[link_name = "syscall_add_intc_handler"]
        unsafe fn raw_add_intc_handler(
            int_cause: i32,
            handler: InterruptHandler,
            next: i32,
            arg: usize,
            flag: i32,
        ) -> i32;

        #[link_name = "syscall_remove_intc_handler"]
        unsafe fn raw_remove_intc_handler(int_cause: i32, handler_id: i32) -> i32;
    }

    pub unsafe fn add_interrupt_handler(
        interrupt_type: InterruptType,
        handler: InterruptHandler,
        // TODO: Add queue preference
    ) -> Result<HandlerId, ()> {
        unsafe {
            let code = raw_add_intc_handler(
                interrupt_type as i32,
                handler,
                // Add to back of queue
                -1,
                // Don't know what these do, so just leave as zero.
                0,
                0,
            );
            match code {
                -1 => Err(()),
                _ => Ok(HandlerId(code)),
            }
        }
    }
}
