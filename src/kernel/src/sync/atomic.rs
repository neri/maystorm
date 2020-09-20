// Atomic NonZero Pointer

use core::marker::PhantomData;
use core::sync::atomic::*;

#[derive(Debug, Default)]
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
    pub const NULL: AtomicObject<T> = Self::new(0);

    #[inline]
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
    #[track_caller]
    pub fn unwrap(&self) -> T {
        self.load().unwrap()
    }

    #[inline]
    pub fn map<U, F: FnOnce(T) -> U>(&self, f: F) -> Option<U> {
        match self.load() {
            Some(t) => Some(f(t)),
            None => None,
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
            None => 0,
            Some(t) => t.into(),
        }
    }

    #[inline]
    fn into_t(value: usize) -> Option<T> {
        match value {
            0 => None,
            _ => Some(value.into()),
        }
    }
}
