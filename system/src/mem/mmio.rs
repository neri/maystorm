use super::*;
use crate::drivers::pci::PciBar;
use crate::*;
use core::mem::{size_of, transmute};
use core::num::NonZeroUsize;

macro_rules! mmio_reg_declare {
    (
        $(
            $(#[$outer:meta])*
            $vis:vis struct $class:ident: $ty:ty;
        )*
    ) => {
        $(
            $(#[$outer])*
            #[repr(transparent)]
            $vis struct $class(core::cell::UnsafeCell<$ty>);

            impl $class {
                #[allow(dead_code)]
                #[inline]
                $vis fn read_volatile(&self) -> $ty {
                    unsafe {
                        self.0.get().read_volatile()
                    }
                }

                #[allow(dead_code)]
                #[inline]
                $vis fn write_volatile(&self, val: $ty) {
                    unsafe {
                        self.0.get().write_volatile(val);
                    }
                }
            }
        )*
    };
}

mmio_reg_declare! {

    pub struct MmioRegU8: u8;

    pub struct MmioRegU16: u16;

    pub struct MmioRegU32: u32;

    pub struct MmioRegU64: u64;
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
    pub fn read_u8(&self, offset: usize) -> u8 {
        let mut result = 0;
        self.check_limit(offset, &result);
        unsafe {
            let ptr: &MmioRegU8 = transmute(self.base + offset);
            result = ptr.read_volatile();
        };
        result
    }

    #[inline]
    #[track_caller]
    pub fn write_u8(&self, offset: usize, value: u8) {
        self.check_limit(offset, &value);
        unsafe {
            let ptr: &MmioRegU8 = transmute(self.base + offset);
            ptr.write_volatile(value);
        };
    }

    #[inline]
    #[track_caller]
    pub fn read_u16(&self, offset: usize) -> u16 {
        let mut result = 0;
        self.check_limit(offset, &result);
        unsafe {
            let ptr: &MmioRegU16 = transmute(self.base + offset);
            result = ptr.read_volatile();
        };
        result
    }

    #[inline]
    #[track_caller]
    pub fn read_u32(&self, offset: usize) -> u32 {
        let mut result = 0;
        self.check_limit(offset, &result);
        unsafe {
            let ptr: &MmioRegU32 = transmute(self.base + offset);
            result = ptr.read_volatile();
        };
        result
    }

    #[inline]
    #[track_caller]
    pub fn write_u32(&self, offset: usize, value: u32) {
        self.check_limit(offset, &value);
        unsafe {
            let ptr: &MmioRegU32 = transmute(self.base + offset);
            ptr.write_volatile(value);
        };
    }

    #[inline]
    #[track_caller]
    pub fn read_u64(&self, offset: usize) -> u64 {
        let mut result = 0;
        self.check_limit(offset, &result);
        unsafe {
            let ptr: &MmioRegU64 = transmute(self.base + offset);
            result = ptr.read_volatile();
        };
        result
    }

    #[inline]
    #[track_caller]
    pub fn write_u64(&self, offset: usize, value: u64) {
        self.check_limit(offset, &value);
        unsafe {
            let ptr: &MmioRegU64 = transmute(self.base + offset);
            ptr.write_volatile(value);
        };
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
