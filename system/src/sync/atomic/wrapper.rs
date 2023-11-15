use core::{
    marker::PhantomData,
    ops::{Add, BitAnd, BitOr, BitXor, Sub},
    sync::atomic::*,
};

macro_rules! atomic_wrapper {
    ( $class_name:ident, $ty:ty, $atomic_type:ty ) => {
        pub struct $class_name<T> {
            bits: $atomic_type,
            _phantom: PhantomData<T>,
        }

        unsafe impl<T: Send> Send for $class_name<T> {}

        unsafe impl<T: Send> Sync for $class_name<T> {}

        impl<T> $class_name<T> {
            #[inline]
            fn _bits(&self) -> $ty {
                self.bits.load(Ordering::Acquire)
            }

            #[inline]
            pub const fn empty() -> Self {
                Self {
                    bits: <$atomic_type>::new(0),
                    _phantom: PhantomData,
                }
            }
        }

        impl<T: From<$ty>> $class_name<T> {
            #[inline]
            pub fn value(&self) -> T {
                T::from(self._bits())
            }

            #[inline]
            pub fn into_inner(self) -> T {
                self.bits.into_inner().into()
            }
        }

        impl<T: Into<$ty>> $class_name<T> {
            #[inline]
            pub fn new(value: T) -> Self {
                Self {
                    bits: <$atomic_type>::new(value.into()),
                    _phantom: PhantomData,
                }
            }

            #[inline]
            pub fn store(&self, value: T) {
                self.bits.store(value.into(), Ordering::SeqCst);
            }
        }

        impl<T: Into<$ty> + From<$ty>> $class_name<T> {
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

        impl<T: Into<$ty> + Default> Default for $class_name<T> {
            #[inline]
            fn default() -> Self {
                Self::new(T::default())
            }
        }

        impl<T: Copy + Into<$ty> + From<$ty> + Add<Output = T>> $class_name<T> {
            #[inline]
            pub fn fetch_add(&self, other: T) -> T {
                match self.fetch_update(|v| Some(v.add(other))) {
                    Ok(v) => v,
                    Err(v) => v,
                }
            }
        }

        impl<T: Copy + Into<$ty> + From<$ty> + Sub<Output = T>> $class_name<T> {
            #[inline]
            pub fn fetch_sub(&self, other: T) -> T {
                match self.fetch_update(|v| Some(v.sub(other))) {
                    Ok(v) => v,
                    Err(v) => v,
                }
            }
        }

        impl<T: Copy + Into<$ty> + From<$ty> + BitOr<Output = T>> $class_name<T> {
            #[inline]
            pub fn fetch_or(&self, other: T) -> T {
                match self.fetch_update(|v| Some(v.bitor(other))) {
                    Ok(v) => v,
                    Err(v) => v,
                }
            }
        }

        impl<T: Copy + Into<$ty> + From<$ty> + BitAnd<Output = T>> $class_name<T> {
            #[inline]
            pub fn fetch_and(&self, other: T) -> T {
                match self.fetch_update(|v| Some(v.bitand(other))) {
                    Ok(v) => v,
                    Err(v) => v,
                }
            }
        }

        impl<T: Copy + Into<$ty> + From<$ty> + BitXor<Output = T>> $class_name<T> {
            #[inline]
            pub fn fetch_xor(&self, other: T) -> T {
                match self.fetch_update(|v| Some(v.bitxor(other))) {
                    Ok(v) => v,
                    Err(v) => v,
                }
            }
        }
    };
}

#[cfg(target_has_atomic = "ptr")]
atomic_wrapper!(AtomicWrapper, usize, AtomicUsize);
#[cfg(target_has_atomic = "ptr")]
atomic_wrapper!(AtomicFlags, usize, AtomicUsize);
#[cfg(target_has_atomic = "8")]
atomic_wrapper!(AtomicWrapperU8, u8, AtomicU8);
#[cfg(target_has_atomic = "16")]
atomic_wrapper!(AtomicWrapperU16, u16, AtomicU16);
#[cfg(target_has_atomic = "32")]
atomic_wrapper!(AtomicWrapperU32, u32, AtomicU32);
#[cfg(target_has_atomic = "64")]
atomic_wrapper!(AtomicWrapperU64, u64, AtomicU64);

impl<T: Into<usize>> AtomicFlags<T> {
    pub const EMPTY: Self = Self {
        bits: AtomicUsize::new(0),
        _phantom: PhantomData,
    };

    #[inline]
    pub fn bits(&self) -> usize {
        self._bits()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self._bits() == 0
    }

    #[inline]
    pub fn contains(&self, other: T) -> bool {
        let other = other.into();
        (self._bits() & other) == other
    }

    #[inline]
    pub fn insert(&self, other: T) {
        let other = other.into();
        self.bits.fetch_or(other, Ordering::AcqRel);
    }

    #[inline]
    pub fn remove(&self, other: T) {
        let other = other.into();
        self.bits.fetch_and(!other, Ordering::AcqRel);
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
}

impl<T: Into<usize>> AtomicFlags<T> {
    #[inline]
    pub fn fetch_set(&self, other: T) -> bool {
        use crate::*;
        Hal::sync().fetch_set(&self.bits, other.into().trailing_zeros() as usize)
    }

    #[inline]
    pub fn fetch_reset(&self, other: T) -> bool {
        use crate::*;
        Hal::sync().fetch_reset(&self.bits, other.into().trailing_zeros() as usize)
    }
}
