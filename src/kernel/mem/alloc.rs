// My Poop Allocator
use crate::*;
use core::alloc::{GlobalAlloc, Layout};
use core::ffi::c_void;
use core::intrinsics::*;
use core::ptr::*;

#[global_allocator]
static mut ALLOCATOR: CustomAlloc = CustomAlloc::new(0, 0);

pub fn init(base: usize, rest: usize) {
    unsafe {
        if ALLOCATOR.base != 0 {
            panic!("ALLOCATOR has already been initialized");
        }
        ALLOCATOR = CustomAlloc::new(base, rest);
    }
}

pub struct CustomAlloc {
    base: usize,
    rest: usize,
}

impl CustomAlloc {
    const fn new(base: usize, rest: usize) -> Self {
        let fixed_base = (base + 15) & !15;
        CustomAlloc {
            base: fixed_base,
            rest: rest + base - fixed_base,
        }
    }

    pub unsafe fn zalloc(size: usize) -> Option<NonNull<c_void>> {
        let size = (size + 15) & !15;
        loop {
            let rest = ALLOCATOR.rest;
            if rest < size {
                break None;
            }
            let (_, acquired) = atomic_cxchg(&mut ALLOCATOR.rest, rest, rest - size);
            if acquired {
                let result = atomic_xadd(&mut ALLOCATOR.base, size);
                break NonNull::new(result as *const c_void as *mut c_void);
            }
        }
    }
}

unsafe impl GlobalAlloc for CustomAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.align() > 16 {
            panic!("Unsupported align {:?}", layout);
        }
        let req_size = (layout.size() + 0xF) & !0xF;
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
