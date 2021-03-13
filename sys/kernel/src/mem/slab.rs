// Slab Allocator

use super::*;
use crate::arch::cpu::Cpu;
use ::alloc::vec::Vec;
use core::sync::atomic::*;
use core::{alloc::Layout, mem::transmute};
use core::{mem::size_of, num::*};

type UsizeSmall = u16;

pub(crate) struct SlabAllocator {
    vec: Vec<SlabCache>,
}

impl SlabAllocator {
    pub unsafe fn new() -> Self {
        let sizes = [16, 32, 64, 128, 256, 512, 1024, 2048];

        let mut vec: Vec<SlabCache> = Vec::with_capacity(sizes.len());
        for block_size in &sizes {
            vec.push(SlabCache::new(*block_size));
        }

        Self { vec }
    }

    pub fn alloc(&self, layout: Layout) -> Result<NonZeroUsize, AllocationError> {
        if layout.size() > UsizeSmall::MAX as usize || layout.align() > UsizeSmall::MAX as usize {
            return Err(AllocationError::Unsupported);
        }
        let size = layout.size() as UsizeSmall;
        let align = layout.align() as UsizeSmall;
        for slab in &self.vec {
            if size <= slab.block_size && align <= slab.block_size {
                return slab.alloc();
            }
        }
        return Err(AllocationError::Unsupported);
    }

    pub fn free(&self, base: NonZeroUsize, layout: Layout) -> Result<(), DeallocationError> {
        if layout.size() > UsizeSmall::MAX as usize || layout.align() > UsizeSmall::MAX as usize {
            return Err(DeallocationError::Unsupported);
        }
        let size = layout.size() as UsizeSmall;
        let align = layout.align() as UsizeSmall;
        for slab in &self.vec {
            if size <= slab.block_size && align <= slab.block_size {
                return slab.free(base);
            }
        }
        Err(DeallocationError::Unsupported)
    }

    #[allow(dead_code)]
    pub(super) fn free_memory_size(&self) -> usize {
        self.vec.iter().fold(0, |v, i| v + i.free_memory_size())
    }

    #[allow(dead_code)]
    pub(super) fn statistics(&self) -> Vec<(usize, usize, usize)> {
        let mut vec = Vec::with_capacity(self.vec.len());
        for item in &self.vec {
            vec.push((
                item.block_size(),
                item.chunks[..item.count as usize].iter().fold(0, |v, i| {
                    v + i.bitmap.load(Ordering::Relaxed).count_ones() as usize
                }),
                item.items_per_chunk() * item.count(),
            ));
        }
        vec
    }
}

struct SlabCache {
    block_size: UsizeSmall,
    count: UsizeSmall,
    chunks: [SlabChunkHeader; Self::N_CHUNKS],
}

impl SlabCache {
    const N_CHUNKS: usize = 63;
    const ITEMS_PER_CHUNK: usize = 8 * size_of::<usize>();

    fn new(block_size: usize) -> Self {
        let mut chunks: [SlabChunkHeader; Self::N_CHUNKS] =
            unsafe { transmute([0u8; size_of::<SlabChunkHeader>() * Self::N_CHUNKS]) };

        let mut count = 0;
        let items_per_chunk = Self::ITEMS_PER_CHUNK;
        let preferred_page_size = items_per_chunk * block_size;
        let atomic_page_size = if MemoryManager::PAGE_SIZE_MIN < preferred_page_size {
            1
        } else {
            MemoryManager::PAGE_SIZE_MIN / preferred_page_size
        };

        unsafe {
            let pages = usize::min(
                (items_per_chunk / atomic_page_size) * atomic_page_size,
                usize::max(atomic_page_size, 0x1000 / preferred_page_size),
            );
            let alloc_size = preferred_page_size * pages;
            let blob = MemoryManager::zalloc(Layout::from_size_align_unchecked(
                alloc_size,
                MemoryManager::PAGE_SIZE_MIN,
            ))
            .unwrap();

            // println!("CHUNK: {} {} {} ", block_size, preferred_page_size, pages);

            for i in 0..pages {
                let ptr = blob.get() + preferred_page_size * i;
                let chunk = SlabChunkHeader::new(ptr, usize::MAX);
                chunks[count as usize] = chunk;
                count += 1;
            }
        }

        Self {
            block_size: block_size as UsizeSmall,
            count,
            chunks,
        }
    }

    #[inline]
    const fn block_size(&self) -> usize {
        self.block_size as usize
    }

    #[inline]
    const fn count(&self) -> usize {
        self.count as usize
    }

    #[inline]
    const fn items_per_chunk(&self) -> usize {
        Self::ITEMS_PER_CHUNK
    }

    fn alloc(&self) -> Result<NonZeroUsize, AllocationError> {
        for chunk in self.chunks[..self.count()].iter() {
            if !chunk.is_full() {
                match chunk.alloc() {
                    Some(index) => {
                        return NonZeroUsize::new(chunk.ptr() + index * self.block_size as usize)
                            .ok_or(AllocationError::Unexpected)
                    }
                    None => (),
                }
            }
        }
        Err(AllocationError::OutOfMemory)
    }

    fn free(&self, ptr: NonZeroUsize) -> Result<(), DeallocationError> {
        let ptr = ptr.get();

        for chunk in self.chunks[..self.count()].iter() {
            let base = chunk.ptr();
            if ptr >= base && ptr < base + self.items_per_chunk() * self.block_size() {
                let index = (ptr - base) / self.block_size();
                chunk.free(index);
                return Ok(());
            }
        }
        Err(DeallocationError::InvalidArgument)
    }

    fn free_memory_size(&self) -> usize {
        self.chunks[..self.count()].iter().fold(0, |v, i| {
            v + self.block_size() * i.bitmap.load(Ordering::Relaxed).count_ones() as usize
        })
    }
}

struct SlabChunkHeader {
    ptr: AtomicUsize,
    bitmap: AtomicUsize,
}

impl SlabChunkHeader {
    #[inline]
    fn new(ptr: usize, bitmap: usize) -> Self {
        Self {
            ptr: AtomicUsize::new(ptr),
            bitmap: AtomicUsize::new(bitmap),
        }
    }

    #[inline]
    fn is_full(&self) -> bool {
        self.bitmap.load(Ordering::Relaxed) == 0
    }

    #[inline]
    fn ptr(&self) -> usize {
        self.ptr.load(Ordering::Relaxed)
    }

    #[inline]
    fn alloc(&self) -> Option<usize> {
        let limit = 8 * size_of::<usize>();
        for i in 0..limit {
            if Cpu::interlocked_test_and_clear(&self.bitmap, i) {
                return Some(i);
            }
        }
        None
    }

    #[inline]
    fn free(&self, position: usize) {
        Cpu::interlocked_test_and_set(&self.bitmap, position);
    }
}
