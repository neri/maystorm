// Atomic Enum

use core::{marker::PhantomData, sync::atomic::*};

pub struct AtomicEnum<T> {
    bits: AtomicUsize,
    _phantom: PhantomData<T>,
}

impl<T: Into<usize>> AtomicEnum<T> {
    #[inline]
    pub fn new(value: T) -> Self {
        Self {
            bits: AtomicUsize::new(value.into()),
            _phantom: PhantomData,
        }
    }

    #[inline]
    pub const unsafe fn from_raw_unchecked(raw: usize) -> Self {
        Self {
            bits: AtomicUsize::new(raw),
            _phantom: PhantomData,
        }
    }

    #[inline]
    fn _bits(&self) -> usize {
        self.bits.load(Ordering::Acquire)
    }

    #[inline]
    pub fn set(&self, val: T) {
        self.bits.store(val.into(), Ordering::SeqCst);
    }
}

impl<T: Into<usize> + From<usize>> AtomicEnum<T> {
    #[inline]
    pub fn value(&self) -> T {
        T::from(self._bits())
    }

    #[inline]
    pub fn swap(&self, other: T) -> T {
        T::from(self.bits.swap(other.into(), Ordering::SeqCst))
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

impl<T: Into<usize> + Default> Default for AtomicEnum<T> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}
