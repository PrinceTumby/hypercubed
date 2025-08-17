use super::interrupts;
use core::arch::asm;

unsafe extern "C" {
    pub unsafe fn write_u64(ptr: *mut u64, low: u32, high: u32);

    pub unsafe fn sync_p();

    pub unsafe fn disable_interrupts() -> bool;

    pub unsafe fn enable_interrupts() -> bool;

    pub unsafe fn dma_wait_fast();

    unsafe fn _sync_d_cache(start: *const u8, end: *const u8);
}

#[inline]
pub fn sync_d_cache<T>(range: core::ops::Range<*const T>) {
    unsafe {
        let eie: usize;
        asm!(
            "mfc0 {out}, $12",
            out = lateout(reg) eie,
        );
        if eie & 0x10000 != 0 {
            interrupts::disable();
        }
        _sync_d_cache(range.start as *const u8, range.end as *const u8);
        if eie & 0x10000 != 0 {
            interrupts::enable();
        }
    }
}
