use crate::sys::syscall::*;
use core::alloc::{GlobalAlloc, Layout};

#[global_allocator]
static mut ALLOCATOR: CustomAlloc = CustomAlloc();

pub struct CustomAlloc();

unsafe impl GlobalAlloc for CustomAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        os_alloc(layout.size(), layout.align())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        os_dealloc(ptr, layout.size(), layout.align());
    }
}

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!(
        "Allocation error: {{ size: {}, align: {} }}",
        layout.size(),
        layout.align()
    )
}
