// Page Manager

// use core::mem::*;
use super::alloc;
use bootprot::*;
use core::num::*;

#[allow(dead_code)]
static mut SHARED: PageManager = PageManager::new();

pub struct PageManager {}

impl PageManager {
    const fn new() -> Self {
        Self {}
    }

    pub(crate) unsafe fn init(info: &BootInfo) {
        alloc::init(info.static_start as usize, info.free_memory as usize);

        #[cfg(any(target_arch = "x86_64"))]
        alloc::CustomAlloc::init_real(info.real_bitmap);
    }

    pub fn direct_map(base: usize, _size: usize) -> Option<NonZeroUsize> {
        NonZeroUsize::new(base)
    }
}
