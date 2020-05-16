use core::alloc::{GlobalAlloc, Layout};
use core::intrinsics::*;
use core::ptr::null_mut;

#[global_allocator]
static mut ALLOCATOR: CustomAlloc = CustomAlloc::new(0, 0);

pub fn init(base: usize, limit: usize) {
    unsafe {
        if ALLOCATOR.base != 0 {
            panic!("ALLOCATOR has already been initialized");
        }
        ALLOCATOR = CustomAlloc::new(base, limit);
    }
}

pub struct CustomAlloc {
    base: usize,
    limit: usize,
    rest: usize,
}

impl CustomAlloc {
    const fn new(base: usize, limit: usize) -> Self {
        CustomAlloc {
            base: base,
            limit: limit,
            rest: limit,
        }
    }
}

unsafe impl GlobalAlloc for CustomAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let req_size = layout.size() + layout.align();
        loop {
            let rest = ALLOCATOR.rest;
            if rest < req_size {
                break null_mut();
            }
            let (_, acquired) = atomic_cxchg(&mut ALLOCATOR.rest, rest, rest - req_size);
            if acquired {
                let result = atomic_xadd(&mut ALLOCATOR.base, req_size);
                break result as *const u8 as *mut u8;
            }
        }
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

unsafe impl Sync for CustomAlloc {}

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}
