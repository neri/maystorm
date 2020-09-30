// Memory Manager

// use crate::arch::page::*;
use super::slab::*;
use alloc::boxed::Box;
use bitflags::*;
use bootprot::*;
use core::num::*;
use core::sync::atomic::*;

static mut MM: MemoryManager = MemoryManager::new();

pub struct MemoryManager {
    total_memory_size: u64,
    page_size_min: usize,
    static_start: AtomicUsize,
    static_free: AtomicUsize,
    slab: Option<Box<SlabAllocator>>,
    #[cfg(any(target_arch = "x86_64"))]
    real_bitmap: [u32; 8],
}

impl MemoryManager {
    const fn new() -> Self {
        Self {
            total_memory_size: 0,
            page_size_min: 0x1000,
            static_start: AtomicUsize::new(0),
            static_free: AtomicUsize::new(0),
            slab: None,
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
            .static_free
            .store(info.free_memory as usize, Ordering::Relaxed);
        shared.slab = Some(SlabAllocator::new());

        if cfg!(any(target_arch = "x86_64")) {
            shared.real_bitmap = info.real_bitmap;
        }
    }

    #[inline]
    fn shared() -> &'static mut Self {
        unsafe { &mut MM }
    }

    pub unsafe fn direct_map(
        base: usize,
        size: usize,
        prot: MProtect,
    ) -> Result<NonZeroUsize, AllocationError> {
        // TODO:
        let _ = size;
        let _ = prot;
        NonZeroUsize::new(base).ok_or(AllocationError::InvalidArgument)
    }

    #[inline]
    pub fn total_memory_size(&self) -> u64 {
        self.total_memory_size
    }

    #[inline]
    pub fn page_size_min(&self) -> usize {
        self.page_size_min
    }

    /// Allocate static pages
    unsafe fn static_alloc(size: usize) -> Result<NonZeroUsize, AllocationError> {
        let shared = Self::shared();
        let page_mask = 0xFFF;
        let size = (size + page_mask * 2 + 1) & !page_mask;
        loop {
            let left = shared.static_free.load(Ordering::Relaxed);
            if left < size {
                return Err(AllocationError::OutOfMemory);
            }
            if shared
                .static_free
                .compare_exchange_weak(left, left - size, Ordering::SeqCst, Ordering::Relaxed)
                .is_ok()
            {
                let result = shared.static_start.fetch_add(size, Ordering::SeqCst);
                return NonZeroUsize::new(result).ok_or(AllocationError::Unexpected);
            }
        }
    }

    /// Allocate kernel memory
    pub unsafe fn zalloc(size: usize) -> Result<NonZeroUsize, AllocationError> {
        let shared = Self::shared();
        if let Some(slab) = &shared.slab {
            match slab.alloc(size) {
                Ok(result) => return Ok(result),
                Err(AllocationError::Unsupported) => (),
                Err(err) => return Err(err),
            }
        }
        Self::static_alloc(size)
    }

    /// Deallocate kernel memory
    pub unsafe fn zfree(base: Option<NonZeroUsize>, size: usize) -> Result<(), DeallocationError> {
        if let Some(base) = base {
            let ptr = base.get() as *mut u8;
            ptr.write_bytes(0xCC, size);

            let shared = Self::shared();
            if let Some(slab) = &shared.slab {
                match slab.free(base, size) {
                    Ok(_) => (),
                    Err(err) => return Err(err),
                }
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

    pub unsafe fn exhaust() {
        Self::shared().static_free.store(0, Ordering::SeqCst);
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

#[derive(Debug, Copy, Clone)]
pub enum AllocationError {
    Unexpected,
    OutOfMemory,
    InvalidArgument,
    Unsupported,
}

#[derive(Debug, Copy, Clone)]
pub enum DeallocationError {
    Unexpected,
    InvalidArgument,
    Unsupported,
}
