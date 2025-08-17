use bitfield::bitfield;
use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::iter::Iterator;
use core::mem::{align_of, size_of};
use core::ptr::{self, NonNull};

#[cfg(target_pointer_width = "64")]
bitfield! {
    #[repr(transparent)]
    struct Block(u64);
    len_internal, set_len_internal: 61, 0;
    pub used, set_used: 62;
    pub has_next, set_has_next: 63;
}

#[cfg(target_pointer_width = "32")]
bitfield! {
    #[repr(transparent)]
    struct Block(u32);
    len_internal, set_len_internal: 29, 0;
    pub used, set_used: 30;
    pub has_next, set_has_next: 31;
}

impl Block {
    #[cfg(target_pointer_width = "64")]
    const LEN_MASK: u64 = 0x3FFF_FFFF_FFFF_FFFF;
    #[cfg(target_pointer_width = "32")]
    const LEN_MASK: u32 = 0x3FFF_FFFF;

    #[cfg(target_pointer_width = "64")]
    pub fn new(len: usize, used: bool, has_next: bool) -> Self {
        Self(len as u64 & Self::LEN_MASK | (used as u64) << 62 | (has_next as u64) << 63)
    }

    #[cfg(target_pointer_width = "32")]
    pub fn new(len: usize, used: bool, has_next: bool) -> Self {
        Self(len as u32 & Self::LEN_MASK | (used as u32) << 30 | (has_next as u32) << 31)
    }

    pub fn len(&self) -> usize {
        self.len_internal() as usize
    }

    pub fn set_len(&mut self, value: usize) {
        #[cfg(target_pointer_width = "64")]
        self.set_len_internal(value as u64);
        #[cfg(target_pointer_width = "32")]
        self.set_len_internal(value as u32);
    }

    /// Returns the start address of the inner block.
    pub fn start_address(&self) -> usize {
        self as *const Self as usize + size_of::<Self>()
    }

    pub unsafe fn get_next(&self) -> Option<NonNull<Self>> {
        unsafe {
            if !self.has_next() {
                return None;
            }
            let address = self as *const Self as usize + size_of::<Self>() + self.len();
            Some(NonNull::new(address as *mut Self).unwrap_unchecked())
        }
    }

    pub unsafe fn iter_mut(&mut self) -> BlockIterator {
        BlockIterator {
            current_block: Some(NonNull::from(self)),
        }
    }
}

struct BlockIterator {
    current_block: Option<NonNull<Block>>,
}

impl Iterator for BlockIterator {
    type Item = NonNull<Block>;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let current_block = self.current_block?;
            self.current_block = current_block.as_ref().get_next();
            Some(current_block)
        }
    }
}

struct KernelHeapAllocator {
    pub list_head: UnsafeCell<Option<NonNull<Block>>>,
}

unsafe impl Sync for KernelHeapAllocator {}

unsafe impl GlobalAlloc for KernelHeapAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe {
            let maybe_list_head_lock = &mut *self.list_head.get();
            let Some(list_head) = maybe_list_head_lock.map(|mut ptr| ptr.as_mut()) else {
                return ptr::null_mut();
            };
            // Scan through list to find free space large enough
            for mut current_block_ptr in list_head.iter_mut() {
                let current_block = current_block_ptr.as_mut();
                if current_block.used() {
                    continue;
                }
                let unaligned_start_addr = current_block.start_address();
                let start_addr = unaligned_start_addr.next_multiple_of(layout.align());
                let max_addr = unaligned_start_addr + (current_block.len() - 1);
                let end_addr = start_addr + (layout.size() - 1);
                if end_addr > max_addr {
                    continue;
                }
                // Found a suitable block, reserve
                current_block.set_used(true);
                // If enough space, split block into used and free blocks, otherwise keep block as is
                let new_block_addr = (end_addr + 1).next_multiple_of(align_of::<Block>());
                let new_space_start = new_block_addr + size_of::<Block>();
                if new_space_start < max_addr {
                    current_block.set_len(new_block_addr - unaligned_start_addr);
                    *(new_block_addr as *mut Block) = Block::new(
                        max_addr - new_space_start + 1,
                        false,
                        current_block.has_next(),
                    );
                    current_block.set_has_next(true);
                }
                return start_addr as *mut u8;
            }
            // Space not found, return failure
            ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        unsafe {
            let search_addr = ptr.addr();
            let list_head = (*self.list_head.get()).unwrap().as_mut();
            let mut maybe_previous_block_ptr: Option<NonNull<Block>> = None;
            for mut current_block_ptr in list_head.iter_mut() {
                let current_block = current_block_ptr.as_mut();
                let min_addr = current_block.start_address();
                let max_addr = min_addr + (current_block.len() - 1);
                // Check if block contains allocation
                if min_addr <= search_addr && search_addr <= max_addr {
                    // Check for double free in debug mode
                    debug_assert!(current_block.used());
                    current_block.set_used(false);
                    // Merge forward if next block is free
                    match current_block.get_next().map(|mut ptr| ptr.as_mut()) {
                        Some(next_block) if !next_block.used() => {
                            current_block.set_len(
                                current_block.len() + size_of::<Block>() + next_block.len(),
                            );
                            current_block.set_has_next(next_block.has_next());
                        }
                        _ => {}
                    }
                    // Merge backward if next block is free
                    match maybe_previous_block_ptr.map(|mut ptr| ptr.as_mut()) {
                        Some(previous_block) if !previous_block.used() => {
                            previous_block.set_len(
                                previous_block.len() + size_of::<Block>() + current_block.len(),
                            );
                            previous_block.set_has_next(current_block.has_next());
                        }
                        _ => {}
                    }
                    return;
                }
                maybe_previous_block_ptr = Some(current_block_ptr);
            }
        }
    }
}

#[global_allocator]
static ALLOCATOR: KernelHeapAllocator = KernelHeapAllocator {
    list_head: UnsafeCell::new(None),
};

/// Initialises an area of memory for use as heap space.
///
/// # Safety
/// The caller guarantees this function is only called once.
pub unsafe fn init_heap(start_address: usize, length: usize) {
    unsafe {
        let new_block_addr = start_address.next_multiple_of(align_of::<Block>());
        let new_block_ptr = new_block_addr as *mut Block;
        new_block_ptr.write(Block::new(
            (start_address + length) - new_block_addr - size_of::<Block>(),
            false,
            false,
        ));
        *ALLOCATOR.list_head.get() = Some(NonNull::new_unchecked(new_block_ptr));
    }
}

#[expect(clippy::missing_safety_doc)]
pub unsafe fn get_allocation_size(search_addr: usize) -> Option<usize> {
    unsafe {
        let list_head = (*ALLOCATOR.list_head.get()).unwrap().as_mut();
        for current_block_ptr in list_head.iter_mut() {
            let current_block = current_block_ptr.as_ref();
            let min_addr = current_block.start_address();
            let max_addr = min_addr + (current_block.len() - 1);
            // Check if block contains allocation
            if min_addr <= search_addr && search_addr <= max_addr {
                // Check block is actually used in debug mode
                debug_assert!(current_block.used());
                return Some(current_block.len());
            }
        }
        None
    }
}
