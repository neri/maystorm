use super::*;
use crate::{drivers::pci::PciBar, *};
use core::{
    marker::PhantomData,
    mem::{size_of, transmute},
    num::NonZeroUsize,
    ops::{Deref, DerefMut},
    slice,
    sync::atomic::*,
};

#[repr(transparent)]
pub struct Mmio<T> {
    base: usize,
    _phantom: PhantomData<T>,
}

impl<T> Mmio<T> {
    #[inline]
    pub unsafe fn from_phys(base: PhysicalAddress) -> Option<Self> {
        MemoryManager::mmap(MemoryMapRequest::Mmio(base, size_of::<T>())).map(|va| Self {
            base: va.get(),
            _phantom: PhantomData,
        })
    }

    #[inline]
    pub unsafe fn from_bar(bar: PciBar) -> Option<Self> {
        if bar.is_mmio() && size_of::<T>() <= bar.size() {
            Self::from_phys(bar.base())
        } else {
            None
        }
    }

    #[inline]
    pub unsafe fn from_virt(base: NonZeroUsize) -> Self {
        Self {
            base: base.get(),
            _phantom: PhantomData,
        }
    }
}

// impl<T> Drop for Mmio<T> {
//     fn drop(&mut self) {
//         // TODO:
//     }
// }

impl<T> Deref for Mmio<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.base as *const _) }
    }
}

impl<T> DerefMut for Mmio<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *(self.base as *mut _) }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct MmioSlice {
    base: usize,
    size: usize,
}

impl MmioSlice {
    #[inline]
    pub unsafe fn from_phys(base: PhysicalAddress, size: usize) -> Option<Self> {
        MemoryManager::mmap(MemoryMapRequest::Mmio(base, size)).map(|va| Self {
            base: va.get(),
            size,
        })
    }

    #[inline]
    pub unsafe fn from_bar(bar: &PciBar) -> Option<Self> {
        if bar.is_mmio() {
            Self::from_phys(bar.base(), bar.size())
        } else {
            None
        }
    }

    #[inline]
    pub unsafe fn from_virt(base: NonZeroUsize, size: usize) -> Self {
        Self {
            base: base.get(),
            size,
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
    #[cfg(target_has_atomic_load_store = "8")]
    pub fn read_u8(&self, offset: usize) -> u8 {
        let slice = unsafe { slice::from_raw_parts(self.base as *const AtomicU8, self.size) };
        slice[offset].load(Ordering::SeqCst)
    }

    #[inline]
    #[track_caller]
    #[cfg(target_has_atomic_load_store = "8")]
    pub fn write_u8(&self, offset: usize, value: u8) {
        let slice = unsafe { slice::from_raw_parts(self.base as *const AtomicU8, self.size) };
        slice[offset].store(value, Ordering::SeqCst);
    }

    #[inline]
    #[track_caller]
    #[cfg(target_has_atomic_load_store = "16")]
    pub fn read_u16(&self, offset: usize) -> u16 {
        let mut result = 0;
        self.check_limit(offset, &result);
        let ptr: &AtomicU16 = unsafe { transmute(self.base + offset) };
        result = ptr.load(Ordering::SeqCst);
        result
    }

    #[inline]
    #[track_caller]
    #[cfg(target_has_atomic_load_store = "32")]
    pub fn read_u32(&self, offset: usize) -> u32 {
        let mut result = 0;
        self.check_limit(offset, &result);
        let ptr: &AtomicU32 = unsafe { transmute(self.base + offset) };
        result = ptr.load(Ordering::SeqCst);
        result
    }

    #[inline]
    #[track_caller]
    #[cfg(target_has_atomic_load_store = "32")]
    pub fn write_u32(&self, offset: usize, value: u32) {
        self.check_limit(offset, &value);
        let ptr: &AtomicU32 = unsafe { transmute(self.base + offset) };
        ptr.store(value, Ordering::SeqCst);
    }

    #[inline]
    #[track_caller]
    #[cfg(target_has_atomic_load_store = "64")]
    pub fn read_u64(&self, offset: usize) -> u64 {
        let mut result = 0;
        self.check_limit(offset, &result);
        let ptr: &AtomicU64 = unsafe { transmute(self.base + offset) };
        result = ptr.load(Ordering::SeqCst);
        result
    }

    #[inline]
    #[track_caller]
    #[cfg(target_has_atomic_load_store = "64")]
    pub fn write_u64(&self, offset: usize, value: u64) {
        self.check_limit(offset, &value);
        let ptr: &AtomicU64 = unsafe { transmute(self.base + offset) };
        ptr.store(value, Ordering::SeqCst);
    }

    #[inline]
    #[track_caller]
    pub unsafe fn transmute<T>(&self, offset: usize) -> &'static T
    where
        T: Sized,
    {
        let result = transmute((self.base as *const u8).add(offset));
        self.check_limit(offset, &result);
        result
    }
}
