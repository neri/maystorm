// Memory Manager

// use crate::arch::page::*;
use super::slab::*;
use super::string::*;
use crate::task::scheduler::*;
use alloc::boxed::Box;
use bitflags::*;
use bootprot::*;
use core::alloc::Layout;
use core::fmt::Write;
use core::num::*;
use core::sync::atomic::*;

static mut MM: MemoryManager = MemoryManager::new();

pub struct MemoryManager {
    total_memory_size: u64,
    page_size_min: usize,
    static_start: AtomicUsize,
    static_free: AtomicUsize,
    static_total: usize,
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
            static_total: 0,
            slab: None,
            #[cfg(any(target_arch = "x86_64"))]
            real_bitmap: [0; 8],
        }
    }

    pub(crate) unsafe fn init_first(info: &BootInfo) {
        let shared = Self::shared();
        shared.total_memory_size = info.total_memory_size;
        shared.static_total = info.free_memory as usize;
        shared
            .static_start
            .store(info.static_start as usize, Ordering::Relaxed);
        shared
            .static_free
            .store(info.free_memory as usize, Ordering::Relaxed);

        if cfg!(any(target_arch = "x86_64")) {
            shared.real_bitmap = info.real_bitmap;
        }
    }

    pub(crate) unsafe fn init2() {
        let shared = Self::shared();
        shared.slab = Some(Box::new(SlabAllocator::new()));
    }

    pub(crate) unsafe fn init_late() {
        SpawnOption::new()
            .priority(Priority::Realtime)
            .new_pid()
            .spawn_f(Self::page_thread, 0, "Page");
    }

    fn page_thread(_args: usize) {
        loop {
            Timer::usleep(1000_000);
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
    unsafe fn static_alloc(layout: Layout) -> Result<NonZeroUsize, AllocationError> {
        let shared = Self::shared();
        let page_mask = shared.page_size_min() - 1;
        let size = (layout.size() + page_mask * 2 + 1) & !page_mask;
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

    /// Allocate kernel memory (old form)
    pub unsafe fn zalloc(size: usize) -> Result<NonZeroUsize, AllocationError> {
        let shared = Self::shared();
        match Layout::from_size_align(size, shared.page_size_min()) {
            Ok(layout) => Self::zalloc_layout(layout),
            Err(_) => Err(AllocationError::InvalidArgument),
        }
    }

    /// Allocate kernel memory
    pub unsafe fn zalloc_layout(layout: Layout) -> Result<NonZeroUsize, AllocationError> {
        let shared = Self::shared();
        if let Some(slab) = &shared.slab {
            match slab.alloc(layout) {
                Ok(result) => return Ok(result),
                Err(AllocationError::Unsupported) => (),
                Err(err) => return Err(err),
            }
        }
        Self::static_alloc(layout)
    }

    /// Deallocate kernel memory
    pub unsafe fn zfree(
        base: Option<NonZeroUsize>,
        layout: Layout,
    ) -> Result<(), DeallocationError> {
        if let Some(base) = base {
            let ptr = base.get() as *mut u8;
            ptr.write_bytes(0xCC, layout.size());

            let shared = Self::shared();
            if let Some(slab) = &shared.slab {
                match slab.free(base, layout) {
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

    pub fn statistics(sb: &mut StringBuffer) {
        let shared = Self::shared();
        sb.clear();
        writeln!(sb, "Total: {} KB", shared.total_memory_size / 1024).unwrap();
        writeln!(
            sb,
            "Kernel: {} / {} KB",
            shared.static_free.load(Ordering::Relaxed) / 1024,
            shared.static_total / 1024,
        )
        .unwrap();
        // for slab in shared.slab.as_ref().unwrap().statistics() {
        //     writeln!(sb, "Slab {:4}: {:3} / {:3}", slab.0, slab.1, slab.2).unwrap();
        // }
    }

    // pub unsafe fn exhaust() {
    //     Self::shared().static_free.store(0, Ordering::SeqCst);
    // }
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
