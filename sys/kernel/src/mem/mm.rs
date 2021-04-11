// Memory Manager

// use crate::arch::page::*;
use super::slab::*;
use super::string::*;
use crate::arch::cpu::Cpu;
use crate::sync::spinlock::Spinlock;
use crate::task::scheduler::*;
use alloc::boxed::Box;
use bitflags::*;
use bootprot::*;
use core::alloc::Layout;
use core::fmt::Write;
use core::num::*;
use core::slice;
use core::sync::atomic::*;

static mut MM: MemoryManager = MemoryManager::new();

pub struct MemoryManager {
    total_memory_size: usize,
    reserved_memory_size: usize,
    page_size_min: usize,
    lock: Spinlock,
    dummy_size: AtomicUsize,
    n_free: AtomicUsize,
    pairs: [MemFreePair; Self::MAX_FREE_PAIRS],
    slab: Option<Box<SlabAllocator>>,
    real_bitmap: [u32; 8],
}

impl MemoryManager {
    const MAX_FREE_PAIRS: usize = 1024;
    pub const PAGE_SIZE_MIN: usize = 0x1000;

    const fn new() -> Self {
        Self {
            total_memory_size: 0,
            reserved_memory_size: 0,
            page_size_min: 0x1000,
            lock: Spinlock::new(),
            dummy_size: AtomicUsize::new(0),
            n_free: AtomicUsize::new(0),
            pairs: [MemFreePair::empty(); Self::MAX_FREE_PAIRS],
            slab: None,
            real_bitmap: [0; 8],
        }
    }

    pub(crate) unsafe fn init_first(info: &BootInfo) {
        let shared = Self::shared();
        shared.total_memory_size = info.total_memory_size as usize;

        let mm: &[BootMemoryMapDescriptor] =
            slice::from_raw_parts(info.mmap_base as usize as *const _, info.mmap_len as usize);
        let mut free_count = 0;
        let mut n_free = 0;
        for mem_desc in mm {
            if mem_desc.mem_type == BootMemoryType::Available {
                let size = mem_desc.page_count as usize * Self::PAGE_SIZE_MIN;
                shared.pairs[n_free] = MemFreePair {
                    base: mem_desc.base as usize,
                    size,
                };
                free_count += size;
            }
            n_free += 1;
        }
        shared.n_free.store(n_free, Ordering::SeqCst);
        shared.reserved_memory_size = shared.total_memory_size - free_count;

        if cfg!(any(target_arch = "x86_64")) {
            shared.real_bitmap = info.real_bitmap;
        }

        shared.slab = Some(Box::new(SlabAllocator::new()));
    }

    pub(crate) unsafe fn late_init() {
        // SpawnOption::with_priority(Priority::Realtime).spawn(Self::page_thread, 0, "Page");
    }

    #[allow(dead_code)]
    fn page_thread(_args: usize) {
        // TODO:
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
    pub fn total_memory_size() -> usize {
        let shared = Self::shared();
        shared.total_memory_size
    }

    #[inline]
    pub fn page_size_min(&self) -> usize {
        self.page_size_min
    }

    #[inline]
    pub fn reserved_memory_size() -> usize {
        let shared = Self::shared();
        shared.reserved_memory_size
    }

    #[inline]
    pub fn free_memory_size() -> usize {
        let shared = Self::shared();
        shared.lock.synchronized(|| {
            let mut total = shared.dummy_size.load(Ordering::Relaxed);
            total += shared
                .slab
                .as_ref()
                .map(|v| v.free_memory_size())
                .unwrap_or(0);
            total += shared.pairs[..shared.n_free.load(Ordering::Relaxed)]
                .iter()
                .fold(0, |v, i| v + i.size);
            total
        })
    }

    /// Allocate static pages
    unsafe fn static_alloc(layout: Layout) -> Result<NonZeroUsize, AllocationError> {
        let shared = Self::shared();

        let align_m1 = Self::PAGE_SIZE_MIN - 1;
        let size = (layout.size() + align_m1) & !(align_m1);
        let n_free = shared.n_free.load(Ordering::SeqCst);
        for i in 0..n_free {
            let free_pair = &mut shared.pairs[i];
            if free_pair.size >= size {
                let ptr = free_pair.base;
                free_pair.base += size;
                free_pair.size -= size;
                return Ok(NonZeroUsize::new_unchecked(ptr));
            }
        }
        Err(AllocationError::OutOfMemory)
    }

    /// Allocate kernel memory (old form)
    pub unsafe fn zalloc_legacy(size: usize) -> Result<NonZeroUsize, AllocationError> {
        let shared = Self::shared();
        match Layout::from_size_align(size, shared.page_size_min()) {
            Ok(layout) => Self::zalloc(layout),
            Err(_) => Err(AllocationError::InvalidArgument),
        }
    }

    /// Allocate kernel memory
    pub unsafe fn zalloc(layout: Layout) -> Result<NonZeroUsize, AllocationError> {
        let shared = Self::shared();
        if let Some(slab) = &shared.slab {
            match slab.alloc(layout) {
                Ok(result) => return Ok(result),
                Err(AllocationError::Unsupported) => (),
                Err(err) => return Err(err),
            }
        }
        shared.lock.synchronized(|| Self::static_alloc(layout))
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
                    Ok(_) => Ok(()),
                    Err(_) => {
                        shared.dummy_size.fetch_add(layout.size(), Ordering::SeqCst);
                        Ok(())
                    }
                }
            } else {
                shared.dummy_size.fetch_add(layout.size(), Ordering::SeqCst);
                Ok(())
            }
        } else {
            Ok(())
        }
    }

    /// Allocate a page on real memory
    pub unsafe fn static_alloc_real() -> Option<NonZeroU8> {
        let max_real = 0xA0;
        let shared = Self::shared();
        for i in 1..max_real {
            let result = Cpu::interlocked_test_and_clear(
                &*(&shared.real_bitmap[0] as *const _ as *const AtomicUsize),
                i,
            );
            if result {
                return NonZeroU8::new(i as u8);
            }
        }
        None
    }

    pub fn statistics(sb: &mut StringBuffer) {
        let shared = Self::shared();
        sb.clear();

        let dummy = shared.dummy_size.load(Ordering::Relaxed);
        let free = shared.pairs[..shared.n_free.load(Ordering::Relaxed)]
            .iter()
            .fold(0, |v, i| v + i.size);
        let total = free + dummy;
        writeln!(
            sb,
            "Memory {} MB Pages {} ({} + {})",
            shared.total_memory_size >> 20,
            total / Self::PAGE_SIZE_MIN,
            free / Self::PAGE_SIZE_MIN,
            dummy / Self::PAGE_SIZE_MIN,
        )
        .unwrap();

        for chunk in shared.slab.as_ref().unwrap().statistics().chunks(4) {
            write!(sb, "Slab").unwrap();
            for item in chunk {
                write!(sb, " {:4}: {:3} / {:3}", item.0, item.1, item.2,).unwrap();
            }
            writeln!(sb, "").unwrap();
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct MemFreePair {
    base: usize,
    size: usize,
}

impl MemFreePair {
    const fn empty() -> Self {
        Self { base: 0, size: 0 }
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
