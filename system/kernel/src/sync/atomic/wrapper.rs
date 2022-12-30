use core::{marker::PhantomData, sync::atomic::*};

pub struct AtomicWrapper<T> {
    bits: AtomicUsize,
    _phantom: PhantomData<T>,
}

unsafe impl<T: Send> Send for AtomicWrapper<T> {}

unsafe impl<T: Send> Sync for AtomicWrapper<T> {}

impl<T: Into<usize>> AtomicWrapper<T> {
    #[inline]
    pub const fn new(value: T) -> Self
    where
        T: ~const Into<usize>,
    {
        Self {
            bits: AtomicUsize::new(value.into()),
            _phantom: PhantomData,
        }
    }

    #[inline]
    fn _bits(&self) -> usize {
        self.bits.load(Ordering::Acquire)
    }

    #[inline]
    pub fn store(&self, value: T) {
        self.bits.store(value.into(), Ordering::SeqCst);
    }
}

impl<T: Into<usize> + From<usize>> AtomicWrapper<T> {
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

impl<T: ~const Into<usize> + ~const Default> const Default for AtomicWrapper<T> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}
