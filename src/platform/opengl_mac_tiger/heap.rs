use core::alloc::{GlobalAlloc, Layout};

#[link(name = "c", kind = "dylib")]
unsafe extern "C" {
    unsafe fn malloc(num_bytes: usize) -> *mut u8;
    unsafe fn free(ptr: *mut u8);
}

#[global_allocator]
static ALLOCATOR: MallocHeapAllocator = MallocHeapAllocator;

struct MallocHeapAllocator;

unsafe impl GlobalAlloc for MallocHeapAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe {
            if layout.align() > 16 {
                todo!("Malloc large alignment - {}", layout.align());
            }
            malloc(layout.size().try_into().unwrap())
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        unsafe {
            free(ptr);
        }
    }
}
