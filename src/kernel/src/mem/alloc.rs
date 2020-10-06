// My Poop Allocator
use super::memory::*;
use core::alloc::{GlobalAlloc, Layout};
use core::num::NonZeroUsize;

#[global_allocator]
static mut ALLOCATOR: CustomAlloc = CustomAlloc::new();

pub struct CustomAlloc {
    _dummy: (),
}

impl CustomAlloc {
    const fn new() -> Self {
        CustomAlloc { _dummy: () }
    }
}

unsafe impl GlobalAlloc for CustomAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        MemoryManager::zalloc_layout(layout)
            .map(|v| v.get())
            .unwrap_or(0) as *mut u8
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let _ = MemoryManager::zfree(NonZeroUsize::new(ptr as usize), layout);
    }
}

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}
