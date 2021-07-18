// Memory Allocator

use super::*;
use core::alloc::{GlobalAlloc, Layout};

#[global_allocator]
static mut ALLOCATOR: CustomAlloc = CustomAlloc::new();

pub struct CustomAlloc {
    _phantom: (),
}

impl CustomAlloc {
    const fn new() -> Self {
        CustomAlloc { _phantom: () }
    }
}

unsafe impl GlobalAlloc for CustomAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        os_alloc(layout.size(), layout.align()) as *mut u8
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        os_dealloc(ptr as usize, layout.size(), layout.align());
    }
}

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}
