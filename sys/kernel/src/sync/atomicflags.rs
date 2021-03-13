// Atomic Bit Flags

use crate::arch::cpu::Cpu;
use core::marker::PhantomData;
use core::sync::atomic::*;

pub struct AtomicBitflags<T>
where
    T: Into<usize>,
{
    repr: AtomicUsize,
    _phantom: PhantomData<T>,
}

impl<T: Into<usize>> AtomicBitflags<T> {
    pub const EMPTY: Self = Self::empty();

    #[inline]
    pub const fn empty() -> AtomicBitflags<T> {
        Self {
            repr: AtomicUsize::new(0),
            _phantom: PhantomData,
        }
    }

    #[inline]
    pub const fn from_bits(bits: usize) -> AtomicBitflags<T> {
        Self {
            repr: AtomicUsize::new(bits),
            _phantom: PhantomData,
        }
    }

    #[inline]
    pub fn new(value: T) -> AtomicBitflags<T> {
        Self::from_bits(value.into())
    }

    #[inline]
    pub fn contains(&self, other: T) -> bool {
        let other = other.into();
        (self.repr.load(Ordering::Relaxed) & other) == other
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.repr.load(Ordering::Relaxed) == 0
    }

    #[inline]
    pub fn insert(&self, other: T) {
        let other = other.into();
        self.repr.fetch_or(other, Ordering::SeqCst);
    }

    #[inline]
    pub fn remove(&self, other: T) {
        let other = other.into();
        self.repr.fetch_and(!other, Ordering::SeqCst);
    }

    #[inline]
    pub fn toggle(&self, other: T) {
        let other = other.into();
        self.repr.fetch_xor(other, Ordering::SeqCst);
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
    pub fn test_and_set(&self, bits: T) -> bool {
        Cpu::interlocked_test_and_set(&self.repr, bits.into().trailing_zeros() as usize)
    }

    #[inline]
    pub fn test_and_clear(&self, bits: T) -> bool {
        Cpu::interlocked_test_and_clear(&self.repr, bits.into().trailing_zeros() as usize)
    }
}
