// Slab Allocator

use super::memory::*;
use crate::*;
use alloc::boxed::Box;
use alloc::vec::*;
use core::num::*;
use core::ops::Drop;
use core::sync::atomic::*;

pub struct SlabAllocator {
    vec: Vec<SlabHeader>,
}

impl SlabAllocator {
    pub fn new() -> Box<Self> {
        let sizes = [8, 12, 16, 24, 32, 48, 64, 96, 128, 192, 256, 384, 512];
        let mut vec: Vec<SlabHeader> = Vec::with_capacity(sizes.len());
        for item_size in &sizes {
            vec.push(SlabHeader::new(*item_size));
        }
        Box::new(Self { vec })
    }

    pub fn alloc(&self, size: usize) -> Result<NonZeroUsize, AllocationError> {
        for slab in &self.vec {
            if size <= slab.item_size {
                return slab.alloc();
            }
        }
        return Err(AllocationError::Unsupported);
    }

    pub fn free(&self, base: NonZeroUsize, size: usize) -> Result<(), DeallocationError> {
        for slab in &self.vec {
            if size <= slab.item_size {
                return slab.free(base);
            }
        }
        Err(DeallocationError::Unsupported)
    }
}

const MAX_BITMAP_SIZE: usize = 64;

#[derive(Debug)]
struct SlabHeader {
    item_size: usize,
    chunk_size: usize,
    items_per_chunk: usize,
    first_chunk: SlabChunk,
}

impl SlabHeader {
    fn new(item_size: usize) -> Self {
        let min_bitmap_size = 4;
        let mut chunk_size = 0x1000;
        let mut items_per_chunk: usize;
        let mut bitmap_size: usize;
        loop {
            items_per_chunk = chunk_size / item_size;
            bitmap_size = (items_per_chunk + 7) / 8;
            if bitmap_size >= min_bitmap_size {
                break;
            }
            chunk_size *= 2;
        }

        Self {
            chunk_size,
            item_size,
            items_per_chunk,
            first_chunk: SlabChunk::new(items_per_chunk, chunk_size),
        }
    }

    fn alloc(&self) -> Result<NonZeroUsize, AllocationError> {
        let chunk = &self.first_chunk;

        match chunk.alloc() {
            Ok(index) => {
                return NonZeroUsize::new(chunk.entity + index * self.item_size)
                    .ok_or(AllocationError::Unexpected)
            }
            Err(err) => Err(err),
        }
    }

    fn free(&self, base: NonZeroUsize) -> Result<(), DeallocationError> {
        let base = base.get();
        let chunk = &self.first_chunk;

        if base >= chunk.entity && base < chunk.entity + self.chunk_size {
            let index = (base - chunk.entity) / self.item_size;
            chunk.free(index);
            return Ok(());
        }

        Err(DeallocationError::InvalidArgument)
    }
}

impl Drop for SlabChunk {
    fn drop(&mut self) {
        todo!()
    }
}

#[derive(Debug)]
struct SlabChunk {
    link: AtomicUsize,
    free: AtomicUsize,
    count: usize,
    entity: usize,
    bitmap: [u8; MAX_BITMAP_SIZE],
}

impl SlabChunk {
    fn new(items_per_chunk: usize, chunk_size: usize) -> Self {
        let entity = unsafe {
            let blob = MemoryManager::zalloc(chunk_size).unwrap().get() as *mut u8;
            blob.write_bytes(0, chunk_size);
            blob as usize
        };

        Self {
            link: AtomicUsize::new(0),
            free: AtomicUsize::new(items_per_chunk),
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
            for i in 0..self.count {
                unsafe {
                    let mut result: usize;
                    asm!("
                        lock bts [{0}], {1}
                        sbb {2}, {2}
                        ", in(reg) &self.bitmap[0], in(reg) i, lateout(reg) result);
                    if result == 0 {
                        return Ok(i);
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
            asm!("
                lock btr [{0}], {1}
                ", in(reg) &self.bitmap[0], in(reg) index);
        }
        self.free.fetch_add(1, Ordering::Relaxed);
    }
}
