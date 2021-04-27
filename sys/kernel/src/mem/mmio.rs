// Memory Mapped I/O Registers

use super::*;
use core::{
    mem::{size_of, transmute},
    num::NonZeroUsize,
    sync::atomic::*,
};

#[derive(Debug, Copy, Clone)]
pub struct Mmio {
    base: usize,
    size: usize,
}

impl Mmio {
    #[inline]
    pub unsafe fn from_phys(base: usize, size: NonZeroUsize) -> Result<Self, AllocationError> {
        MemoryManager::map_mmio(base, size).map(|va| Self {
            base: va.get(),
            size: size.get(),
        })
    }

    #[inline]
    pub unsafe fn from_virt(base: NonZeroUsize, size: NonZeroUsize) -> Self {
        Self {
            base: base.get(),
            size: size.get(),
        }
    }

    #[inline]
    #[track_caller]
    fn check_limit<T>(&self, offset: usize, _: &T)
    where
        T: Sized,
    {
        let delta = size_of::<T>();
        assert!(
            offset + delta <= self.size,
            "mmio: index {}..{} is out of bounds",
            offset,
            offset + delta,
        );
    }

    #[inline]
    pub const fn base(&self) -> usize {
        self.base
    }

    #[inline]
    pub const fn size(&self) -> usize {
        self.size
    }

    #[inline]
    #[track_caller]
    pub unsafe fn read_u8(&self, offset: usize) -> u8 {
        let mut result = 0;
        self.check_limit(offset, &result);
        let ptr: &AtomicU8 = transmute(self.base + offset);
        result = ptr.load(Ordering::SeqCst);
        result
    }

    #[inline]
    #[track_caller]
    pub unsafe fn read_u32(&self, offset: usize) -> u32 {
        let mut result = 0;
        self.check_limit(offset, &result);
        let ptr: &AtomicU32 = transmute(self.base + offset);
        result = ptr.load(Ordering::SeqCst);
        result
    }

    #[inline]
    #[track_caller]
    #[cfg(target_has_atomic_load_store = "64")]
    pub unsafe fn read_u64(&self, offset: usize) -> u64 {
        let mut result = 0;
        self.check_limit(offset, &result);
        let ptr: &AtomicU64 = transmute(self.base + offset);
        result = ptr.load(Ordering::SeqCst);
        result
    }

    #[inline]
    #[track_caller]
    pub unsafe fn write_u8(&self, offset: usize, value: u8) {
        self.check_limit(offset, &value);
        let ptr: &AtomicU8 = transmute(self.base + offset);
        ptr.store(value, Ordering::SeqCst);
    }

    #[inline]
    #[track_caller]
    pub unsafe fn write_u32(&self, offset: usize, value: u32) {
        self.check_limit(offset, &value);
        let ptr: &AtomicU32 = transmute(self.base + offset);
        ptr.store(value, Ordering::SeqCst);
    }

    #[inline]
    #[track_caller]
    #[cfg(target_has_atomic_load_store = "64")]
    pub unsafe fn write_u64(&self, offset: usize, value: u64) {
        self.check_limit(offset, &value);
        let ptr: &AtomicU64 = transmute(self.base + offset);
        ptr.store(value, Ordering::SeqCst);
    }

    #[inline]
    #[track_caller]
    pub unsafe fn transmute<T>(&self, offset: usize) -> &T
    where
        T: Sized,
    {
        let result = transmute((self.base as *const u8).add(offset));
        self.check_limit(offset, &result);
        result
    }

    #[inline]
    #[track_caller]
    pub unsafe fn transmute_mut<T>(&self, offset: usize) -> &mut T
    where
        T: Sized,
    {
        let result = transmute((self.base as *const u8).add(offset));
        self.check_limit(offset, &result);
        result
    }
}
