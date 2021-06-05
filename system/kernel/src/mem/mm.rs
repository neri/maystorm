// Memory Manager

// use crate::arch::page::*;
use super::slab::*;
use crate::{
    arch::page::*,
    sync::spinlock::Spinlock,
    system::System,
    task::scheduler::*,
    {arch::cpu::Cpu, sync::semaphore::Semaphore},
};
use alloc::boxed::Box;
use bitflags::*;
use bootprot::*;
use core::{
    alloc::Layout,
    fmt::Write,
    mem::{size_of, transmute},
    num::*,
    slice,
    sync::atomic::*,
};

use megstd::string::*;

static mut MM: MemoryManager = MemoryManager::new();

pub struct MemoryManager {
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

        let mm: &[BootMemoryMapDescriptor] =
            slice::from_raw_parts(info.mmap_base as usize as *const _, info.mmap_len as usize);
        let mut free_count = 0;
        let mut n_free = 0;
        for mem_desc in mm {
            if mem_desc.mem_type == BootMemoryType::Available {
                let size = mem_desc.page_count as usize * Self::PAGE_SIZE_MIN;
                shared.pairs[n_free] = MemFreePair::new(mem_desc.base as usize, size);
                free_count += size;
            }
            n_free += 1;
        }
        shared.n_free.store(n_free, Ordering::SeqCst);
        shared.reserved_memory_size = info.total_memory_size as usize - free_count;

        if cfg!(any(target_arch = "x86_64")) {
            shared.real_bitmap = info.real_bitmap;
        }

        PageManager::init(info);

        shared.slab = Some(Box::new(SlabAllocator::new()));
    }

    pub(crate) unsafe fn late_init() {
        PageManager::init_late();
        SpawnOption::with_priority(Priority::Realtime).start(Self::page_thread, 0, "Page Manager");
    }

    #[allow(dead_code)]
    fn page_thread(_args: usize) {
        // TODO:
        let sem = Semaphore::new(0);
        loop {
            sem.wait();
        }
    }

    #[inline]
    fn shared() -> &'static mut Self {
        unsafe { &mut MM }
    }

    #[inline]
    pub unsafe fn mmap(request: MemoryMapRequest) -> Option<NonZeroUsize> {
        let va = NonZeroUsize::new(PageManager::mmap(request));
        if Scheduler::is_enabled() {
            // TODO:
            // PageManager::broadcast_invalidate_tlb().unwrap();
        }
        va
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
                .fold(0, |v, i| v + i.size());
            total
        })
    }

    /// Allocate pages
    pub unsafe fn pg_alloc(layout: Layout) -> Option<NonZeroUsize> {
        let shared = Self::shared();

        let align_m1 = Self::PAGE_SIZE_MIN - 1;
        let size = (layout.size() + align_m1) & !(align_m1);
        let n_free = shared.n_free.load(Ordering::SeqCst);
        for i in 0..n_free {
            let free_pair = &shared.pairs[i];
            match free_pair.alloc(size) {
                Ok(v) => return Some(NonZeroUsize::new_unchecked(v)),
                Err(_) => (),
            }
        }
        None
    }

    /// Allocate kernel memory
    pub unsafe fn zalloc(layout: Layout) -> Option<NonZeroUsize> {
        let shared = Self::shared();
        if let Some(slab) = &shared.slab {
            match slab.alloc(layout) {
                Ok(result) => return Some(result),
                Err(AllocationError::Unsupported) => (),
                Err(_err) => return None,
            }
        }
        Self::pg_alloc(layout)
            .and_then(|v| NonZeroUsize::new(PageManager::direct_map(v.get() as PhysicalAddress)))
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
            .fold(0, |v, i| v + i.size());
        let total = free + dummy;

        writeln!(
            sb,
            "Memory {} MB Pages {} ({} + {})",
            System::current_device().total_memory_size() >> 20,
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
    inner: u64,
}

impl MemFreePair {
    const PAGE_SIZE: usize = 0x1000;

    #[inline]
    pub const fn empty() -> Self {
        Self { inner: 0 }
    }

    #[inline]
    pub const fn new(base: usize, size: usize) -> Self {
        let base = (base / Self::PAGE_SIZE) as u64;
        let size = (size / Self::PAGE_SIZE) as u64;
        Self {
            inner: base | (size << 32),
        }
    }

    #[inline]
    pub fn alloc(&self, size: usize) -> Result<usize, ()> {
        let size = (size + Self::PAGE_SIZE - 1) / Self::PAGE_SIZE;

        let p: &AtomicU64 = unsafe { transmute(&self.inner) };
        let mut data = p.load(Ordering::SeqCst);
        loop {
            let (base, limit) = ((data & 0xFFFF_FFFF) as usize, (data >> 32) as usize);
            if limit < size {
                return Err(());
            }
            let new_size = limit - size;
            let new_data = (base as u64) | ((new_size as u64) << 32);

            data = match p.compare_exchange(data, new_data, Ordering::SeqCst, Ordering::Relaxed) {
                Ok(_) => return Ok((base + new_size) * Self::PAGE_SIZE),
                Err(v) => v,
            };
        }
    }

    #[inline]
    fn split(&self) -> (usize, usize) {
        let p: &AtomicU64 = unsafe { transmute(&self.inner) };
        let data = p.load(Ordering::SeqCst);
        ((data & 0xFFFF_FFFF) as usize, (data >> 32) as usize)
    }

    #[inline]
    #[allow(dead_code)]
    pub fn base(&self) -> usize {
        self.split().0 * Self::PAGE_SIZE
    }

    #[inline]
    pub fn size(&self) -> usize {
        self.split().1 * Self::PAGE_SIZE
    }
}

bitflags! {
    pub struct MProtect: usize {
        const READ  = 0x4;
        const WRITE = 0x2;
        const EXEC  = 0x1;
        const NONE  = 0x0;

        const READ_WRITE = Self::READ.bits | Self::WRITE.bits;
        const READ_EXEC = Self::READ.bits | Self::WRITE.bits;
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

#[derive(Debug, Clone, Copy)]
pub enum MemoryMapRequest {
    /// for MMIO (physical_address, length)
    Mmio(PhysicalAddress, usize),
    /// for VRAM (physical_address, length)
    Vram(PhysicalAddress, usize),
    /// for Kernel Mode Heap (base, length, attr)
    Kernel(usize, usize, MProtect),
    /// for User Mode Heap (base, length, attr)
    User(usize, usize, MProtect),
}

impl MemoryMapRequest {
    #[allow(dead_code)]
    fn to_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(&self as *const _ as *const u8, size_of::<Self>()) }
    }
}
