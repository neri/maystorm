use super::slab::*;
use crate::{
    arch::cpu::Cpu,
    arch::page::*,
    sync::{
        fifo::EventQueue,
        semaphore::Semaphore,
        spinlock::{SpinMutex, SpinMutexGuard},
    },
    system::System,
    task::scheduler::*,
};
use alloc::{boxed::Box, sync::Arc};
use bitflags::*;
use bootprot::*;
use core::{
    alloc::Layout,
    cell::UnsafeCell,
    ffi::c_void,
    fmt::Write,
    mem::MaybeUninit,
    mem::{size_of, transmute},
    num::*,
    slice,
    sync::atomic::*,
};
use megstd::string::*;

pub use crate::arch::page::{NonNullPhysicalAddress, PhysicalAddress};

static mut MM: UnsafeCell<MemoryManager> = UnsafeCell::new(MemoryManager::new());

pub struct MemoryManager {
    reserved_memory_size: usize,
    page_size_min: usize,
    dummy_size: AtomicUsize,
    n_free: AtomicUsize,
    pairs: SpinMutex<[MemFreePair; Self::MAX_FREE_PAIRS]>,
    slab: Option<Box<SlabAllocator>>,
    real_bitmap: [u32; 8],
    fifo: MaybeUninit<EventQueue<Arc<AsyncMmapRequest>>>,
}

impl MemoryManager {
    const MAX_FREE_PAIRS: usize = 1024;
    pub const PAGE_SIZE_MIN: usize = 0x1000;

    const fn new() -> Self {
        Self {
            reserved_memory_size: 0,
            page_size_min: 0x1000,
            dummy_size: AtomicUsize::new(0),
            n_free: AtomicUsize::new(0),
            pairs: SpinMutex::new([MemFreePair::empty(); Self::MAX_FREE_PAIRS]),
            slab: None,
            real_bitmap: [0; 8],
            fifo: MaybeUninit::uninit(),
        }
    }

    pub unsafe fn init_first(info: &BootInfo) {
        let shared = Self::shared_mut();

        let mm: &[BootMemoryMapDescriptor] =
            slice::from_raw_parts(info.mmap_base as usize as *const _, info.mmap_len as usize);
        let mut free_count = 0;
        let mut n_free = 0;
        let mut pairs = shared.pairs();
        for mem_desc in mm {
            if mem_desc.mem_type == BootMemoryType::Available {
                let size = mem_desc.page_count as usize * Self::PAGE_SIZE_MIN;
                pairs[n_free] = MemFreePair::new(mem_desc.base as usize, size);
                free_count += size;
            }
            n_free += 1;
        }
        drop(pairs);
        shared.n_free.store(n_free, Ordering::SeqCst);
        shared.reserved_memory_size = info.total_memory_size as usize - free_count;

        if cfg!(any(target_arch = "x86_64")) {
            shared.real_bitmap = info.real_bitmap;
        }

        PageManager::init(info);

        shared.slab = Some(Box::new(SlabAllocator::new()));

        shared.fifo.write(EventQueue::new(100));
    }

    pub unsafe fn late_init() {
        PageManager::init_late();
        SpawnOption::with_priority(Priority::Realtime).start(Self::_page_thread, 0, "Page Manager");
    }

    #[allow(dead_code)]
    fn _page_thread(_args: usize) {
        let shared = Self::shared();
        let fifo = unsafe { &*shared.fifo.as_ptr() };
        while let Some(event) = fifo.wait_event() {
            let result = unsafe { PageManager::mmap(event.request) };
            PageManager::broadcast_invalidate_tlb().unwrap();
            event.result.store(result, Ordering::SeqCst);
            event.sem.signal();
        }
    }

    #[inline]
    unsafe fn shared_mut() -> &'static mut Self {
        MM.get_mut()
    }

    #[inline]
    fn shared() -> &'static Self {
        unsafe { &*MM.get() }
    }

    #[inline]
    fn pairs(&self) -> SpinMutexGuard<[MemFreePair; Self::MAX_FREE_PAIRS]> {
        self.pairs.lock()
    }

    #[inline]
    pub unsafe fn mmap(request: MemoryMapRequest) -> Option<NonZeroUsize> {
        if Scheduler::is_enabled() {
            let fifo = &*Self::shared().fifo.as_ptr();
            let event = Arc::new(AsyncMmapRequest {
                request,
                result: AtomicUsize::new(0),
                sem: Semaphore::new(0),
            });
            match fifo.post(event.clone()) {
                Ok(_) => (),
                Err(_) => todo!(),
            }
            event.sem.wait();
            NonZeroUsize::new(event.result.load(Ordering::SeqCst))
        } else {
            NonZeroUsize::new(PageManager::mmap(request))
        }
    }

    #[inline]
    pub fn direct_map<T: Sized>(pa: PhysicalAddress) -> *mut T {
        unsafe { transmute(PageManager::direct_map(pa)) }
    }

    #[inline]
    pub unsafe fn invalidate_cache(p: usize) {
        PageManager::invalidate_cache(p);
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
        let mut total = shared.dummy_size.load(Ordering::Relaxed);
        total += shared.pairs()[..shared.n_free.load(Ordering::Relaxed)]
            .iter()
            .fold(0, |v, i| v + i.size());
        // total += shared
        //     .slab
        //     .as_ref()
        //     .map(|v| v.free_memory_size())
        //     .unwrap_or(0);
        total
    }

    /// Allocate pages
    #[must_use]
    pub unsafe fn pg_alloc(layout: Layout) -> Option<NonZeroUsize> {
        if layout.align() > Self::PAGE_SIZE_MIN {
            return None;
        }
        let shared = Self::shared();
        let align_m1 = Self::PAGE_SIZE_MIN - 1;
        let size = (layout.size() + align_m1) & !(align_m1);
        let n_free = shared.n_free.load(Ordering::SeqCst);
        let pairs = shared.pairs();
        for i in 0..n_free {
            let free_pair = &pairs[i];
            match free_pair.alloc(size) {
                Ok(v) => return NonZeroUsize::new(v),
                Err(_) => (),
            }
        }
        None
    }

    pub unsafe fn pg_dealloc(base: NonZeroUsize, layout: Layout) {
        let shared = Self::shared();
        let _ = base;
        // let align_m1 = Self::PAGE_SIZE_MIN - 1;
        // let size = (layout.size() + align_m1) & !(align_m1);
        // let mut pairs = shared.pairs();
        shared.dummy_size.fetch_add(layout.size(), Ordering::SeqCst);
    }

    #[must_use]
    pub unsafe fn alloc_pages(size: usize) -> Option<NonZeroUsize> {
        let result = Self::pg_alloc(Layout::from_size_align_unchecked(size, Self::PAGE_SIZE_MIN));
        if let Some(p) = result {
            let p = Self::direct_map::<c_void>(p.get() as PhysicalAddress);
            p.write_bytes(0, size);
        }
        result
    }

    #[inline]
    #[must_use]
    pub unsafe fn alloc_dma<T>(len: usize) -> Option<(PhysicalAddress, *mut T)> {
        Self::alloc_pages(size_of::<T>() * len).map(|v| {
            let pa = v.get() as PhysicalAddress;
            (pa, Self::direct_map(pa))
        })
    }

    /// Allocate kernel memory
    #[must_use]
    pub unsafe fn zalloc(layout: Layout) -> Option<NonZeroUsize> {
        let shared = Self::shared();
        if let Some(slab) = &shared.slab {
            match slab.alloc(layout) {
                Ok(result) => return Some(result),
                Err(AllocationError::Unsupported) => (),
                Err(_err) => return None,
            }
        }
        Self::zalloc2(layout)
    }

    #[must_use]
    pub unsafe fn zalloc2(layout: Layout) -> Option<NonZeroUsize> {
        Self::pg_alloc(layout)
            .and_then(|v| NonZeroUsize::new(PageManager::direct_map(v.get() as PhysicalAddress)))
    }

    /// Deallocate kernel memory
    pub unsafe fn zfree(
        base: Option<NonZeroUsize>,
        layout: Layout,
    ) -> Result<(), DeallocationError> {
        if let Some(base) = base {
            (base.get() as *mut u8).write_bytes(0xCC, layout.size());

            let shared = Self::shared();
            if let Some(slab) = &shared.slab {
                if slab.free(base, layout).is_ok() {
                    return Ok(());
                }
            }
            Self::pg_dealloc(base, layout);
            Ok(())
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
        let free = shared.pairs()[..shared.n_free.load(Ordering::Relaxed)]
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
                write!(sb, " {:4}: {:4}/{:4}", item.0, item.1, item.2,).unwrap();
            }
            writeln!(sb, "").unwrap();
        }
    }
}

struct AsyncMmapRequest {
    request: MemoryMapRequest,
    result: AtomicUsize,
    sem: Semaphore,
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
struct MemFreePair(u64);

#[allow(dead_code)]
impl MemFreePair {
    const PAGE_SIZE: usize = 0x1000;

    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[inline]
    pub const fn new(base: usize, size: usize) -> Self {
        let base = (base / Self::PAGE_SIZE) as u64;
        let size = (size / Self::PAGE_SIZE) as u64;
        Self(base | (size << 32))
    }

    #[inline]
    fn inner(&self) -> &AtomicU64 {
        unsafe { transmute(&self.0) }
    }

    #[inline]
    pub fn raw(&self) -> u64 {
        self.inner().load(Ordering::SeqCst)
    }

    #[inline]
    fn split(data: u64) -> (usize, usize) {
        ((data & 0xFFFF_FFFF) as usize, (data >> 32) as usize)
    }

    #[inline]
    pub fn base(&self) -> usize {
        Self::split(self.raw()).0 * Self::PAGE_SIZE
    }

    #[inline]
    pub fn size(&self) -> usize {
        Self::split(self.raw()).1 * Self::PAGE_SIZE
    }

    #[inline]
    pub fn alloc(&self, size: usize) -> Result<usize, ()> {
        let size = (size + Self::PAGE_SIZE - 1) / Self::PAGE_SIZE;

        let p = self.inner();

        p.fetch_update(Ordering::SeqCst, Ordering::Relaxed, |data| {
            let (base, limit) = Self::split(data);
            if limit < size {
                return None;
            }
            let new_size = limit - size;
            let new_data = ((base + size) as u64) | ((new_size as u64) << 32);
            Some(new_data)
        })
        .map(|data| Self::split(data).0 * Self::PAGE_SIZE)
        .map_err(|_| ())
        // Ok(_) => return Ok((base + new_size) * Self::PAGE_SIZE),
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
