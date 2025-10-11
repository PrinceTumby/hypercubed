use super::{QuadWord, asm_helpers, interrupts};
use crate::portable_prelude::*;
use core::arch::asm;
use core::task::Poll;

#[derive(Clone, Copy, Debug)]
pub struct Packet<'a>(pub &'a [QuadWord]);

// use core::marker::PhantomData;
//
// #[derive(Clone, Copy, Debug, PartialEq)]
// pub struct Packet<'a> {
//     num_data_quadwords: u32,
//     // quadword_count: u16,
//     ty: PacketType,
//     data_ptr: *const QuadWord,
//     phantom: PhantomData<&'a [QuadWord]>,
// }
//
// #[repr(u16)]
// #[derive(Clone, Copy, Debug, PartialEq, Eq)]
// pub enum PacketType {
//     Normal = 0,
//     Ucab = 1,
//     Spr = 2,
// }
//
// impl Packet<'_> {
//     pub fn new(data: &[QuadWord], ty: PacketType) -> Self {
//         assert!(
//             data.as_ptr().addr() % 64 == 0,
//             "`data` must be 64-byte aligned",
//         );
//         Self {
//             num_data_quadwords: data.len().try_into().unwrap(),
//             // quadword_count: if ty == PacketType::Spr { 0x1000 } else { 0 },
//             ty,
//             data_ptr: match ty {
//                 PacketType::Ucab => data.as_ptr().map_addr(|addr| addr | 0x30000000),
//                 _ => data.as_ptr(),
//             },
//             phantom: PhantomData,
//         }
//     }
// }

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Vif0 = 0,
    Vif1 = 1,
    Gif = 2,
    IpuFrom = 3,
    IpuTo = 4,
    Sif0 = 5,
    Sif1 = 6,
    Sif2 = 7,
    SprFrom = 8,
    SprTo = 9,
}

pub type ChannelHandler = extern "C" fn(cause: i32) -> i32;

static mut INITIALISED_CHANNELS: [bool; 10] = [false; 10];

static mut CHANNEL_HANDLER_ID: [i32; 10] = [0; 10];

#[cfg(debug_assertions)]
static mut CURRENT_FAST_WAIT_CHANNEL: Option<Channel> = None;

/// Tag Address Save 0
const ASR0: [*mut u32; 10] = [
    0x10008040 as *mut u32,
    0x10009040 as *mut u32,
    0x1000A040 as *mut u32,
    core::ptr::null_mut(),
    core::ptr::null_mut(),
    core::ptr::null_mut(),
    core::ptr::null_mut(),
    core::ptr::null_mut(),
    core::ptr::null_mut(),
    core::ptr::null_mut(),
];

/// Tag Address Save 1
const ASR1: [*mut u32; 10] = [
    0x10008050 as *mut u32,
    0x10009050 as *mut u32,
    0x1000A050 as *mut u32,
    core::ptr::null_mut(),
    core::ptr::null_mut(),
    core::ptr::null_mut(),
    core::ptr::null_mut(),
    core::ptr::null_mut(),
    core::ptr::null_mut(),
    core::ptr::null_mut(),
];

/// SPR (Scratchpad RAM) Transfer Address
const SADR: [*mut u32; 10] = [
    0x10008080 as *mut u32,
    0x10009080 as *mut u32,
    0x1000A080 as *mut u32,
    core::ptr::null_mut(),
    core::ptr::null_mut(),
    core::ptr::null_mut(),
    core::ptr::null_mut(),
    core::ptr::null_mut(),
    0x1000D080 as *mut u32,
    0x1000D480 as *mut u32,
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChannelFlags {
    pub transfer_tag: bool,
    pub interrupt_safe: bool,
}

/// # Safety
///
/// The given `handler` must be an interrupt-safe handler.
pub unsafe fn initialise_channel(
    channel: Channel,
    handler: Option<ChannelHandler>,
    flags: ChannelFlags,
) {
    unsafe {
        let channel_index = channel as usize;
        // Ensure channel is shut down before making changes.
        shutdown_channel(channel, flags);
        // Clear any saved DMA tags.
        if !ASR0[channel_index].is_null() {
            ASR0[channel_index].write_volatile(0);
            ASR1[channel_index].write_volatile(0);
        }
        // Clear saved SPR address.
        if !SADR[channel_index].is_null() {
            SADR[channel_index].write_volatile(0);
        }
        // Register handler, if provided.
        if let Some(handler) = handler {
            // Add the handler, store the handler ID.
            CHANNEL_HANDLER_ID[channel_index] = syscalls::add_dmac_handler(channel, handler, 0);
            // Enable the channel interrupt.
            if flags.interrupt_safe {
                syscalls::enable_dmac_with_interrupt(channel);
            } else {
                syscalls::enable_dmac(channel);
            }
        }
        // Mark channel as initialised.
        INITIALISED_CHANNELS[channel_index] = true;
    }
}

/// # Safety
///
/// The provided `flags` must match the flags given to `initialise_channel` to initialise the
/// channel.
pub unsafe fn shutdown_channel(channel: Channel, flags: ChannelFlags) {
    unsafe {
        let channel_index = channel as usize;
        // If the channel isn't initialised, no need to do anything.
        if !INITIALISED_CHANNELS[channel_index] {
            return;
        }
        // Remove registered channel handler, if there is one.
        if CHANNEL_HANDLER_ID[channel_index] != 0 {
            // Disable channel interrupt.
            if flags.interrupt_safe {
                syscalls::disable_dmac_with_interrupt(channel);
            } else {
                syscalls::disable_dmac(channel);
            }
            // Remove handler.
            syscalls::remove_dmac_handler(channel, CHANNEL_HANDLER_ID[channel_index]);
            // Clear the handler ID.
            CHANNEL_HANDLER_ID[channel_index] = 0;
        }
        // Mark channel as now uninitialised.
        INITIALISED_CHANNELS[channel_index] = false;
    }
}

const PCR_REGISTER: *mut u32 = 0x1000E020 as *mut u32;

/// Allows for fast `cpcond0` checking.
///
/// # Safety
/// The channel must have already been initialised using `initialise_channel`.
///
/// This must only be used for a single channel at a time.
/// Currently the only consumer of this is `send_packet_normal_and_wait_fast`, which blocks until
/// the wait completes, so this is a non-issue for now.
/// This condition is also enforced in debug mode, but not in release mode.
unsafe fn setup_channel_fast_waiting(channel: Channel) {
    unsafe {
        PCR_REGISTER.write_volatile(PCR_REGISTER.read_volatile() | (1 << channel as u32));
        asm!("nop", "nop", "nop");
    }
}

pub fn wait_on_channel(channel: Channel) {
    unsafe {
        while is_channel_running(channel) {
            asm!("nop", "nop", "nop", "nop", "nop", "nop");
        }
    }
}

use register::channel_control::is_dma_running as is_channel_running;

/// # Safety
///
/// The provided `packet` must be a valid DMA packet.
pub async unsafe fn send_packet_async(channel: Channel, packet: Packet<'_>) {
    // Wait for the channel to become available.
    futures::future::poll_fn(|_cx| match is_channel_running(channel) {
        false => Poll::Ready(()),
        true => Poll::Pending,
    })
    .await;
    // Start sending the packet.
    unsafe {
        send_packet_normal_no_wait(channel, packet);
    }
    // Wait until the packet's finished sending.
    futures::future::poll_fn(|_cx| match is_channel_running(channel) {
        false => Poll::Ready(()),
        true => Poll::Pending,
    })
    .await;
}

unsafe fn send_packet_normal_no_wait(channel: Channel, packet: Packet) {
    use register::channel_control::{ChannelControlData, Direction, Mode};
    unsafe {
        debug_assert!(!is_channel_running(channel));
        assert!(
            packet.0.as_ptr().addr().is_multiple_of(64),
            "Packet data must be 64-byte aligned",
        );
        register::stat::set(&[channel]);
        // Ensure the entire packet's been written out to main memory.
        asm_helpers::sync_d_cache(packet.0.as_ptr_range());
        register::quadword_count::set(channel, packet.0.len().try_into().unwrap());
        register::memory_address::set(channel, packet.0.as_ptr().addr().try_into().unwrap());
        // Start the transfer.
        register::channel_control::set(
            channel,
            ChannelControlData {
                dir: Direction::FromMemory,
                mode: Mode::Normal,
                transfer_dma_tag_enable: false,
                dma_tag_irq_bit_enable: true,
                start: true,
                ..Default::default()
            },
        );
    }
}

/// If the DMA channel is currently running, this blocks until the channel is free.
///
/// # Safety
/// The channel must have already been initialised using `initialise_channel`.
pub unsafe fn send_packet_normal_and_wait_fast(channel: Channel, packet: Packet) {
    unsafe {
        let channel_index = channel as usize;
        debug_assert!(INITIALISED_CHANNELS[channel_index]);
        #[cfg(debug_assertions)]
        #[expect(clippy::deref_addrof)]
        {
            debug_assert!((*&raw const CURRENT_FAST_WAIT_CHANNEL).is_none());
            CURRENT_FAST_WAIT_CHANNEL = Some(channel);
        }
        if is_channel_running(channel) {
            wait_on_channel(channel);
        }
        setup_channel_fast_waiting(channel);
        send_packet_normal_no_wait(channel, packet);
        asm_helpers::dma_wait_fast();
        #[cfg(debug_assertions)]
        {
            CURRENT_FAST_WAIT_CHANNEL = None;
        }
    }
}

/// If the DMA channel is currently running, this blocks until the channel is free.
///
/// # Safety
/// The channel must have already been initialised using `initialise_channel`.
pub unsafe fn send_packet_normal_and_wait(channel: Channel, packet: Packet) {
    unsafe {
        let channel_index = channel as usize;
        debug_assert!(INITIALISED_CHANNELS[channel_index]);
        if is_channel_running(channel) {
            wait_on_channel(channel);
        }
        send_packet_normal_no_wait(channel, packet);
        wait_on_channel(channel);
    }
}

pub mod register {
    use super::*;

    pub mod stat {
        use super::*;

        const PTR: *mut u32 = 0x1000E010 as *mut _;

        #[expect(clippy::missing_safety_doc)]
        pub unsafe fn set(channel_interrupt_status_clears: &[Channel]) {
            let mut value: u32 = 0;
            for channel in channel_interrupt_status_clears {
                value |= 1 << *channel as u32;
            }
            unsafe {
                PTR.write_volatile(value);
            }
        }
    }

    pub mod channel_control {
        use super::*;

        const CHANNEL_PTRS: [*mut u32; 10] = [
            0x10008000 as *mut _,
            0x10009000 as *mut _,
            0x1000A000 as *mut _,
            0x1000B000 as *mut _,
            0x1000B400 as *mut _,
            0x1000C000 as *mut _,
            0x1000C400 as *mut _,
            0x1000C800 as *mut _,
            0x1000D000 as *mut _,
            0x1000D400 as *mut _,
        ];

        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Direction {
            ToMemory = 0,
            FromMemory = 1,
        }

        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Mode {
            Normal = 0,
            Chain = 1,
            Interleave = 2,
        }

        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct ChannelControlData {
            pub dir: Direction,
            pub mode: Mode,
            /// Ranges from 0..=2.
            pub address_stack_pointer: u8,
            pub transfer_dma_tag_enable: bool,
            pub dma_tag_irq_bit_enable: bool,
            pub start: bool,
            /// Bits 16-31 of the most recently read DMA tag.
            pub tag_bits: u16,
        }

        impl Default for ChannelControlData {
            fn default() -> Self {
                Self {
                    dir: Direction::FromMemory,
                    mode: Mode::Normal,
                    address_stack_pointer: 0,
                    transfer_dma_tag_enable: false,
                    dma_tag_irq_bit_enable: true,
                    start: false,
                    tag_bits: 0,
                }
            }
        }

        #[expect(clippy::missing_safety_doc)]
        pub unsafe fn set(channel: Channel, data: ChannelControlData) {
            let value: u32 = (data.dir as u32)
                | ((data.mode as u32) << 2)
                | ((data.address_stack_pointer.min(2) as u32) << 4)
                | ((data.transfer_dma_tag_enable as u32) << 6)
                | ((data.dma_tag_irq_bit_enable as u32) << 7)
                | ((data.start as u32) << 8)
                | ((data.tag_bits as u32) << 16);
            unsafe {
                CHANNEL_PTRS[channel as usize].write_volatile(value);
            }
        }

        pub fn is_dma_running(channel: Channel) -> bool {
            let value = unsafe { CHANNEL_PTRS[channel as usize].read_volatile() };
            value & (1 << 8) != 0
        }
    }

    pub mod memory_address {
        use super::*;

        const CHANNEL_PTRS: [*mut u32; 10] = [
            0x10008010 as *mut _,
            0x10009010 as *mut _,
            0x1000A010 as *mut _,
            0x1000B010 as *mut _,
            0x1000B410 as *mut _,
            0x1000C010 as *mut _,
            0x1000C410 as *mut _,
            0x1000C810 as *mut _,
            0x1000D010 as *mut _,
            0x1000D410 as *mut _,
        ];

        #[expect(clippy::missing_safety_doc)]
        pub unsafe fn set(channel: Channel, address: u32) {
            unsafe {
                CHANNEL_PTRS[channel as usize].write_volatile(address & !(0xF | (1 << 31)));
            }
        }
    }

    pub mod quadword_count {
        use super::*;

        const CHANNEL_PTRS: [*mut u32; 10] = [
            0x10008020 as *mut _,
            0x10009020 as *mut _,
            0x1000A020 as *mut _,
            0x1000B020 as *mut _,
            0x1000B420 as *mut _,
            0x1000C020 as *mut _,
            0x1000C420 as *mut _,
            0x1000C820 as *mut _,
            0x1000D020 as *mut _,
            0x1000D420 as *mut _,
        ];

        #[expect(clippy::missing_safety_doc)]
        pub unsafe fn set(channel: Channel, value: u16) {
            unsafe {
                CHANNEL_PTRS[channel as usize].write_volatile(value as u32);
            }
        }
    }
}

mod syscalls {
    use super::*;

    unsafe extern "C" {
        #[link_name = "syscall_add_dmac_handler"]
        pub unsafe fn add_dmac_handler(channel: Channel, handler: ChannelHandler, next: i32)
        -> i32;

        #[link_name = "syscall_remove_dmac_handler"]
        pub unsafe fn remove_dmac_handler(channel: Channel, handler_id: i32) -> i32;

        #[link_name = "syscall_enable_dmac"]
        unsafe fn raw_enable_dmac(channel: Channel) -> i32;

        #[link_name = "syscall_disable_dmac"]
        unsafe fn raw_disable_dmac(channel: Channel) -> i32;

        #[link_name = "syscall_i_enable_dmac"]
        unsafe fn raw_enable_dmac_interrupt(channel: Channel) -> i32;

        #[link_name = "syscall_i_disable_dmac"]
        unsafe fn raw_disable_dmac_interrupt(channel: Channel) -> i32;
    }

    pub unsafe fn enable_dmac(channel: Channel) -> i32 {
        unsafe {
            let eie: usize;
            asm!(
                "mfc0 {out}, $12",
                out = lateout(reg) eie,
            );
            if eie & 0x10000 != 0 {
                interrupts::disable();
            }
            let result = raw_enable_dmac(channel);
            asm!("sync", options(nostack));
            if eie & 0x10000 != 0 {
                interrupts::enable();
            }
            result
        }
    }

    pub unsafe fn disable_dmac(channel: Channel) -> i32 {
        unsafe {
            let eie: usize;
            asm!(
                "mfc0 {out}, $12",
                out = lateout(reg) eie,
            );
            if eie & 0x10000 != 0 {
                interrupts::disable();
            }
            let result = raw_disable_dmac(channel);
            asm!("sync", options(nostack));
            if eie & 0x10000 != 0 {
                interrupts::enable();
            }
            result
        }
    }

    pub unsafe fn enable_dmac_with_interrupt(channel: Channel) -> i32 {
        unsafe {
            let result = raw_enable_dmac_interrupt(channel);
            asm!("sync", options(nostack));
            result
        }
    }

    pub unsafe fn disable_dmac_with_interrupt(channel: Channel) -> i32 {
        unsafe {
            let result = raw_disable_dmac_interrupt(channel);
            asm!("sync", options(nostack));
            result
        }
    }
}
