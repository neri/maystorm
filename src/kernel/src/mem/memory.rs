// Memory Manager

// use crate::arch::page::*;
use bitflags::*;
use bootprot::*;
use core::num::*;
use core::sync::atomic::*;

static mut MM: MemoryManager = MemoryManager::new();

pub struct MemoryManager {
    total_memory_size: u64,
    static_start: AtomicUsize,
    static_rest: AtomicUsize,
    #[cfg(any(target_arch = "x86_64"))]
    real_bitmap: [u32; 8],
}

impl MemoryManager {
    const fn new() -> Self {
        Self {
            total_memory_size: 0,
            static_start: AtomicUsize::new(0),
            static_rest: AtomicUsize::new(0),
            #[cfg(any(target_arch = "x86_64"))]
            real_bitmap: [0; 8],
        }
    }

    pub(crate) unsafe fn init(info: &BootInfo) {
        let shared = Self::shared();
        shared.total_memory_size = info.total_memory_size;
        shared
            .static_start
            .store(info.static_start as usize, Ordering::Relaxed);
        shared
            .static_rest
            .store(info.free_memory as usize, Ordering::Relaxed);

        if cfg!(any(target_arch = "x86_64")) {
            shared.real_bitmap = info.real_bitmap;
        }
    }

    #[inline]
    fn shared() -> &'static mut Self {
        unsafe { &mut MM }
    }

    pub fn direct_map(base: usize, size: usize, prot: MProtect) -> Option<NonZeroUsize> {
        // TODO:
        let _ = size;
        let _ = prot;
        NonZeroUsize::new(base)
    }

    #[inline]
    pub fn total_memory_size(&self) -> u64 {
        self.total_memory_size
    }

    // Allocate static page
    unsafe fn static_alloc(size: usize) -> Option<NonZeroUsize> {
        let shared = Self::shared();
        let page_mask = 0xFFF;
        let size = (size + page_mask * 2 + 1) & !page_mask;
        loop {
            let rest = shared.static_rest.load(Ordering::Relaxed);
            if rest < size {
                return None;
            }
            if shared
                .static_rest
                .compare_exchange_weak(rest, rest - size, Ordering::SeqCst, Ordering::Relaxed)
                .is_ok()
            {
                let result = shared.static_start.fetch_add(size, Ordering::SeqCst);
                return NonZeroUsize::new(result);
            }
        }
    }

    pub unsafe fn zalloc(size: usize) -> Option<NonZeroUsize> {
        let page_mask = 0x3FFFF;
        let size = (size + page_mask * 2 + 1) & !page_mask;
        Self::static_alloc(size)
    }

    pub unsafe fn zfree(base: Option<NonZeroUsize>, size: usize) -> Result<(), ()> {
        if let Some(base) = base.map(|v| v.get()) {
            let ptr = base as *mut u8;
            for i in 0..size {
                ptr.add(i).write_volatile(0xcc);
            }
        }
        Ok(())
    }

    /// Allocate a page on real memory
    #[cfg(any(target_arch = "x86_64"))]
    pub unsafe fn static_alloc_real() -> Option<NonZeroU8> {
        let max_real = 0xA0;
        let shared = Self::shared();
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

bitflags! {
    pub struct MProtect: usize {
        const READ  = 0x1;
        const WRITE = 0x2;
        const EXEC  = 0x4;
        const NONE  = 0x0;
    }
}
