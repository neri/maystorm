// My Poop Allocator
use super::memory::*;
use core::alloc::{GlobalAlloc, Layout};

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
    #[track_caller]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        MemoryManager::static_alloc(layout.size()).unwrap().get() as *mut u8
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // println!("DEALLOC {:08x} {}", _ptr as usize, _layout.size());
        for i in 0.._layout.size() {
            _ptr.add(i).write_volatile(0xcc);
        }
    }
}

unsafe impl Sync for CustomAlloc {}

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}
