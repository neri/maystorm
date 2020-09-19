// My Poop Allocator
use core::alloc::{GlobalAlloc, Layout};
use core::ffi::c_void;
use core::intrinsics::*;
use core::num::*;
use core::ptr::*;
// use core::sync::atomic::*;

#[global_allocator]
static mut ALLOCATOR: CustomAlloc = CustomAlloc::new(0, 0);

pub(super) fn init(base: usize, rest: usize) {
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
    real_bitmap: [u32; 8],
}

impl CustomAlloc {
    const fn new(base: usize, rest: usize) -> Self {
        let fixed_base = (base + 15) & !15;
        CustomAlloc {
            base: fixed_base,
            rest: rest + base - fixed_base,
            real_bitmap: [0; 8],
        }
    }

    #[cfg(any(target_arch = "x86_64"))]
    pub(crate) unsafe fn init_real(bitmap: [u32; 8]) {
        let shared = &mut ALLOCATOR;
        shared.real_bitmap = bitmap;
    }

    /// Allocate Unowned Memory
    pub unsafe fn zalloc(size: usize) -> Option<NonNull<c_void>> {
        let size = (size + 15) & !15;
        loop {
            let rest = ALLOCATOR.rest;
            if rest < size {
                break None;
            }
            if atomic_cxchg(&mut ALLOCATOR.rest, rest, rest - size).1 {
                let result = atomic_xadd(&mut ALLOCATOR.base, size);
                break NonNull::new(result as *const c_void as *mut c_void);
            }
        }
    }

    /// Allocate Page
    pub unsafe fn z_alloc_page(size: usize) -> Option<NonZeroUsize> {
        let page_mask = 0xFFF;
        let size = (size + page_mask * 2 + 1) & !page_mask;
        loop {
            let rest = ALLOCATOR.rest;
            if rest < size {
                break None;
            }
            if atomic_cxchg(&mut ALLOCATOR.rest, rest, rest - size).1 {
                let result = atomic_xadd(&mut ALLOCATOR.base, size);
                break NonZeroUsize::new(result & !page_mask);
            }
        }
    }

    /// Allocate a page on real memory
    #[cfg(any(target_arch = "x86_64"))]
    pub unsafe fn z_alloc_real() -> Option<NonZeroU8> {
        let max_real = 0xA0;
        let shared = &mut ALLOCATOR;
        for i in 1..max_real {
            let mut result: u32;
            asm!("
            lock btr [{0}], {1:e}
            sbb {2:e}, {2:e}
            ", in(reg) &shared.real_bitmap[0], in(reg) i, lateout(reg) result, );
            if result != 0 {
                return NonZeroU8::new(i as u8);
            }
        }
        None
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
