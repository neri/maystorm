// Atomic Bit Flags

use crate::arch::cpu::Cpu;
use core::{marker::PhantomData, sync::atomic::*};

pub struct AtomicBitflags<T> {
    bits: AtomicUsize,
    _phantom: PhantomData<T>,
}

impl<T: Into<usize>> AtomicBitflags<T> {
    pub const EMPTY: Self = Self::empty();

    #[inline]
    pub const fn empty() -> AtomicBitflags<T> {
        Self {
            bits: AtomicUsize::new(0),
            _phantom: PhantomData,
        }
    }

    #[inline]
    pub const unsafe fn from_bits_unchecked(bits: usize) -> AtomicBitflags<T> {
        Self {
            bits: AtomicUsize::new(bits),
            _phantom: PhantomData,
        }
    }

    #[inline]
    pub fn new(value: T) -> AtomicBitflags<T> {
        unsafe { Self::from_bits_unchecked(value.into()) }
    }

    #[inline]
    pub fn bits(&self) -> usize {
        self.bits.load(Ordering::Acquire)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.bits.load(Ordering::Acquire) == 0
    }

    #[inline]
    pub fn contains(&self, other: T) -> bool {
        let other = other.into();
        (self.bits.load(Ordering::Acquire) & other) == other
    }

    #[inline]
    pub fn insert(&self, other: T) {
        let other = other.into();
        self.bits.fetch_or(other, Ordering::Release);
    }

    #[inline]
    pub fn remove(&self, other: T) {
        let other = other.into();
        self.bits.fetch_and(!other, Ordering::Release);
    }

    #[inline]
    pub fn toggle(&self, other: T) {
        let other = other.into();
        self.bits.fetch_xor(other, Ordering::AcqRel);
    }

    #[inline]
    pub fn set(&self, other: T, value: bool) {
        if value {
            self.insert(other);
        } else {
            self.remove(other);
        }
    }

    #[inline]
    pub fn test_and_set(&self, other: T) -> bool {
        Cpu::interlocked_test_and_set(&self.bits, other.into().trailing_zeros() as usize)
    }

    #[inline]
    pub fn test_and_clear(&self, other: T) -> bool {
        Cpu::interlocked_test_and_clear(&self.bits, other.into().trailing_zeros() as usize)
    }
}

impl<T: Into<usize> + From<usize>> AtomicBitflags<T> {
    #[inline]
    pub fn value(&self) -> T {
        T::from(self.bits())
    }

    #[inline]
    pub fn fetch_update<F>(&self, mut f: F) -> Result<T, T>
    where
        F: FnMut(T) -> Option<T>,
    {
        self.bits
            .fetch_update(Ordering::SeqCst, Ordering::Relaxed, |v| {
                f(v.into()).map(|v| v.into())
            })
            .map(|v| v.into())
            .map_err(|v| v.into())
    }
}

impl<T: Into<usize> + Default> Default for AtomicBitflags<T> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}
