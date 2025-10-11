use embedded_alloc::TlsfHeap;

#[global_allocator]
static HEAP: TlsfHeap = TlsfHeap::empty();

pub unsafe fn init() {
    unsafe {
        let start_address = &raw const syscalls::HEAP_START as usize;
        let end_address = syscalls::init_thread_heap(start_address, -1);
        HEAP.init(start_address, end_address - start_address);
    }
}

mod syscalls {
    use super::*;

    unsafe extern "C" {
        pub static HEAP_START: u8;

        #[link_name = "syscall_init_heap"]
        pub unsafe fn init_thread_heap(start: usize, size: i32) -> usize;
    }
}
