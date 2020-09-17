// Memory Mapped I/O Registers

use super::page::*;
use crate::system::VirtualAddress;
use core::mem::{size_of, transmute};
use core::sync::atomic::*;

#[derive(Debug, Copy, Clone)]
pub struct Mmio {
    base: usize,
    size: usize,
}

unsafe impl Send for Mmio {}

unsafe impl Sync for Mmio {}

impl Mmio {
    pub unsafe fn from_phys(base: usize, size: usize) -> Option<Self> {
        PageManager::direct_map(base, size).map(|va| Self {
            base: va.get(),
            size,
        })
    }

    pub unsafe fn from_virt(base: VirtualAddress, size: usize) -> Option<Self> {
        base.into_nonzero().map(|va| Self {
            base: va.get(),
            size,
        })
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
        result = ptr.load(Ordering::Acquire);
        result
    }

    #[inline]
    #[track_caller]
    pub unsafe fn read_u32(&self, offset: usize) -> u32 {
        let mut result = 0;
        self.check_limit(offset, &result);
        let ptr: &AtomicU32 = transmute(self.base + offset);
        result = ptr.load(Ordering::Acquire);
        result
    }

    #[inline]
    #[track_caller]
    pub unsafe fn read_u64(&self, offset: usize) -> u64 {
        let mut result = 0;
        self.check_limit(offset, &result);
        let ptr: &AtomicU64 = transmute(self.base + offset);
        result = ptr.load(Ordering::Acquire);
        result
    }

    #[inline]
    #[track_caller]
    pub unsafe fn write_u8(&self, offset: usize, value: u8) {
        self.check_limit(offset, &value);
        let ptr: &AtomicU8 = transmute(self.base + offset);
        ptr.store(value, Ordering::Release);
    }

    #[inline]
    #[track_caller]
    pub unsafe fn write_u32(&self, offset: usize, value: u32) {
        self.check_limit(offset, &value);
        let ptr: &AtomicU32 = transmute(self.base + offset);
        ptr.store(value, Ordering::Release);
    }

    #[inline]
    #[track_caller]
    pub unsafe fn write_u64(&self, offset: usize, value: u64) {
        self.check_limit(offset, &value);
        let ptr: &AtomicU64 = transmute(self.base + offset);
        ptr.store(value, Ordering::Release);
    }
}
