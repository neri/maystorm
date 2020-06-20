// Atomic NonZero Pointer

use core::marker::PhantomData;
use core::sync::atomic::*;

#[derive(Debug)]
pub struct AtomicObject<T>
where
    T: Into<usize> + From<usize>,
{
    repr: AtomicUsize,
    phantom: PhantomData<T>,
}

impl<T> AtomicObject<T>
where
    T: Into<usize> + From<usize>,
{
    pub const NULL: AtomicObject<T> = Self {
        repr: AtomicUsize::new(0),
        phantom: PhantomData,
    };

    pub const fn new(value: usize) -> Self {
        Self {
            repr: AtomicUsize::new(value),
            phantom: PhantomData,
        }
    }

    #[inline]
    pub fn as_usize(&self) -> usize {
        self.repr.load(Ordering::SeqCst)
    }

    #[inline]
    pub fn load(&self) -> Option<T> {
        Self::into_t(self.repr.load(Ordering::Acquire))
    }

    #[inline]
    pub fn unwrap(&self) -> T {
        self.load().unwrap()
    }

    #[inline]
    pub fn map<U, F: FnOnce(T) -> U>(&self, f: F) -> Option<U> {
        if let Some(t) = self.load() {
            Some(f(t))
        } else {
            None
        }
    }

    #[inline]
    pub fn store(&self, value: Option<T>) {
        let value = Self::from_t(value);
        self.repr.store(value, Ordering::Release)
    }

    #[inline]
    pub fn swap(&self, value: Option<T>) -> Option<T> {
        let value = Self::from_t(value);
        Self::into_t(self.repr.swap(value, Ordering::AcqRel))
    }

    #[inline]
    pub fn cas(&self, expect: Option<T>, desired: Option<T>) -> Result<Option<T>, Option<T>> {
        let expect = Self::from_t(expect);
        let desired = Self::from_t(desired);

        match self.repr.compare_exchange(
            expect.into(),
            desired.into(),
            Ordering::SeqCst,
            Ordering::Relaxed,
        ) {
            Ok(v) => Ok(Self::into_t(v)),
            Err(v) => Err(Self::into_t(v)),
        }
    }

    #[inline]
    fn from_t(value: Option<T>) -> usize {
        match value {
            Some(t) => t.into(),
            None => 0,
        }
    }

    #[inline]
    fn into_t(value: usize) -> Option<T> {
        if value != 0 {
            Some(value.into())
        } else {
            None
        }
    }
}
