// Memory Manager

use super::alloc;
use crate::arch::page::*;
use bitflags::*;
use bootprot::*;
use core::num::*;

static mut MM: MemoryManager = MemoryManager::new();

pub struct MemoryManager {
    total_memory_size: u64,
}

impl MemoryManager {
    const fn new() -> Self {
        Self {
            total_memory_size: 0,
        }
    }

    pub(crate) unsafe fn init(info: &BootInfo) {
        Self::shared().total_memory_size = info.total_memory_size;

        alloc::init(info.static_start as usize, info.free_memory as usize);

        arch_page_init(&info);
    }

    #[inline]
    fn shared() -> &'static mut Self {
        unsafe { &mut MM }
    }

    pub fn direct_map(base: usize, _size: usize, _prot: MProtect) -> Option<NonZeroUsize> {
        NonZeroUsize::new(base)
    }

    #[inline]
    pub fn total_memory_size(&self) -> u64 {
        self.total_memory_size
    }
}

bitflags! {
    pub struct MProtect: usize {
        const READ  = 0x1;
        const WRITE = 0x2;
        const EXEC  = 0x4;
        const NONE  = 0x0;
    }
}
