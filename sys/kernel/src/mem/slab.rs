// Slab Allocator

use super::memory::*;
use crate::*;
use alloc::vec::*;
use core::alloc::Layout;
use core::num::*;
use core::sync::atomic::*;
// use core::ops::Drop;

type UsizeSmall = u16;
type AtomicUsizeSmall = AtomicU16;
const MAX_BITMAP_SIZE: usize = 16;

pub struct SlabAllocator {
    vec: Vec<SlabCache>,
}

impl SlabAllocator {
    pub unsafe fn new() -> Self {
        let sizes = [32, 64, 128, 256, 512, 1024];

        let mut vec: Vec<SlabCache> = Vec::with_capacity(sizes.len());
        for item_size in &sizes {
            vec.push(SlabCache::new(*item_size));
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

    pub fn statistics(&self) -> Vec<(usize, usize, usize)> {
        let mut vec = Vec::with_capacity(self.vec.len());
        for item in &self.vec {
            vec.push((
                item.block_size as usize,
                item.first_chunk.free.load(Ordering::Relaxed) as usize,
                item.first_chunk.count as usize,
            ));
        }
        vec
    }
}

#[derive(Debug)]
struct SlabCache {
    block_size: UsizeSmall,
    chunk_size_shift: UsizeSmall,
    items_per_chunk: UsizeSmall,
    first_chunk: SlabChunkHeader,
}

impl SlabCache {
    fn new(block_size: UsizeSmall) -> Self {
        let min_bitmap_size = 16;
        let mut chunk_size_shift = 12;
        let mut items_per_chunk: UsizeSmall;
        let mut bitmap_size: UsizeSmall;
        loop {
            let chunk_size = 1 << chunk_size_shift;
            items_per_chunk = (chunk_size / block_size as usize) as UsizeSmall;
            bitmap_size = (items_per_chunk + 7) / 8;
            if bitmap_size >= min_bitmap_size {
                break;
            }
            chunk_size_shift += 1;
        }
        let chunk_size = 1 << chunk_size_shift;

        Self {
            chunk_size_shift,
            block_size,
            items_per_chunk,
            first_chunk: SlabChunkHeader::new(items_per_chunk, chunk_size),
        }
    }

    fn chunk_size(&self) -> usize {
        1 << self.chunk_size_shift
    }

    fn alloc(&self) -> Result<NonZeroUsize, AllocationError> {
        let chunk = &self.first_chunk;

        match chunk.alloc() {
            Ok(index) => {
                return NonZeroUsize::new(chunk.entity + index * self.block_size as usize)
                    .ok_or(AllocationError::Unexpected)
            }
            Err(err) => Err(err),
        }
    }

    fn free(&self, base: NonZeroUsize) -> Result<(), DeallocationError> {
        let base = base.get();
        let chunk = &self.first_chunk;

        if base >= chunk.entity && base < chunk.entity + self.chunk_size() {
            let index = (base - chunk.entity) / self.block_size as usize;
            chunk.free(index);
            return Ok(());
        }

        Err(DeallocationError::InvalidArgument)
    }
}

#[derive(Debug)]
struct SlabChunkHeader {
    link: AtomicUsize,
    free: AtomicUsizeSmall,
    count: UsizeSmall,
    entity: usize,
    bitmap: [u8; MAX_BITMAP_SIZE],
}

impl SlabChunkHeader {
    fn new(items_per_chunk: UsizeSmall, chunk_size: usize) -> Self {
        let entity = unsafe {
            let blob = MemoryManager::zalloc(chunk_size).unwrap().get() as *mut u8;
            blob.write_bytes(0, chunk_size);
            blob as usize
        };

        Self {
            link: AtomicUsize::new(0),
            free: AtomicUsizeSmall::new(items_per_chunk),
            count: items_per_chunk,
            entity,
            bitmap: [0; MAX_BITMAP_SIZE],
        }
    }

    fn alloc(&self) -> Result<usize, AllocationError> {
        if self
            .free
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                if v > 0 {
                    Some(v - 1)
                } else {
                    None
                }
            })
            .is_ok()
        {
            for i in 0..self.count as usize {
                unsafe {
                    let mut result: usize;
                    asm!("
                        lock bts [{0}], {1}
                        sbb {2}, {2}
                        ", in(reg) &self.bitmap[0], in(reg) i, lateout(reg) result);
                    if result == 0 {
                        return Ok(i as usize);
                    }
                }
            }
            Err(AllocationError::Unexpected)
        } else {
            Err(AllocationError::OutOfMemory)
        }
    }

    fn free(&self, index: usize) {
        unsafe {
            asm!("lock btr [{0}], {1}", in(reg) &self.bitmap[0], in(reg) index);
        }
        self.free.fetch_add(1, Ordering::Relaxed);
    }
}
