use super::*;
use core::{
    alloc::{GlobalAlloc, Layout},
    num::NonZeroUsize,
    ptr::null_mut,
};

#[global_allocator]
static mut ALLOCATOR: CustomAlloc = CustomAlloc::new();

pub struct CustomAlloc;

impl CustomAlloc {
    const fn new() -> Self {
        CustomAlloc {}
    }
}

unsafe impl GlobalAlloc for CustomAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        MemoryManager::zalloc(layout)
            .map(|v| v.get() as *mut u8)
            .unwrap_or(null_mut())
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let _ = MemoryManager::zfree(NonZeroUsize::new(ptr as usize), layout);
    }
}

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}
