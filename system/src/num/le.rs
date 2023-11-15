use core::sync::atomic::Ordering;

#[cfg(target_has_atomic = "16")]
use core::sync::atomic::AtomicU16;
#[cfg(target_has_atomic = "32")]
use core::sync::atomic::AtomicU32;
#[cfg(target_has_atomic = "64")]
use core::sync::atomic::AtomicU64;

macro_rules! le_type {
    (
        $vis:vis, $class_name:ident, $raw_type:ty
    ) => {
        #[repr(transparent)]
        #[derive(Clone, Copy, PartialEq, Eq)]
        $vis struct $class_name($raw_type);

        impl $class_name {
            #[inline]
            $vis const fn new(value: $raw_type) -> Self {
                Self(value.to_le())
            }

            #[inline]
            $vis const fn value(&self) -> $raw_type {
                <$raw_type>::from_le(self.0)
            }

            #[inline]
            $vis const fn store(&mut self, value: $raw_type) {
                self.0 = value.to_le()
            }
        }

        impl From<$raw_type> for $class_name {
            #[inline]
            fn from(value: $raw_type) -> $class_name {
                <$class_name>::new(value)
            }
        }

        impl From<$class_name> for $ raw_type{
            #[inline]
            fn from(value: $class_name) -> $raw_type {
                value.value()
            }
        }

        impl Default for $class_name {
            #[inline]
            fn default() -> Self {
                Self(0)
            }
        }
    };
    () => {}
}

macro_rules! atomic_le {
    (
        $vis:vis, $class_name:ident, $outer_type:ty, $atomic_type:ty
    ) => {
        #[repr(transparent)]
        $vis struct $class_name($atomic_type);

        impl $class_name {
            #[inline]
            $vis const fn new(value: $outer_type) -> Self {
                Self(<$atomic_type>::new(value.to_le()))
            }

            #[inline]
            $vis fn value(&self) -> $outer_type {
                <$outer_type>::from_le(self.0.load(Ordering::SeqCst))
            }

            #[inline]
            $vis fn store(&self, value: $outer_type) {
                self.0.store(value.to_le(), Ordering::SeqCst);
            }

            #[inline]
            $vis fn swap(&self, value: $outer_type) -> $outer_type {
                <$outer_type>::from_le(self.0.swap(value.to_le(), Ordering::SeqCst))
            }

            #[inline]
            $vis fn fetch_update<F>(&self, mut f: F) -> Result<$outer_type, $outer_type>
                where F:FnMut($outer_type) -> Option<$outer_type>
            {
                self.0.fetch_update(Ordering::SeqCst, Ordering::Relaxed, |v| {
                    f(<$outer_type>::from_le(v)).map(|v| v.to_le())
                })
                    .map(|v| <$outer_type>::from_le(v))
                    .map_err(|e| <$outer_type>::from_le(e))
            }

            #[inline]
            $vis fn fetch_add(&self, value: $outer_type) -> $outer_type {
                match self.fetch_update(|v| Some(v.wrapping_add(value))) {
                    Ok(v) => v,
                    Err(v) => v,
                }
            }
        }

        impl Default for $class_name {
            #[inline]
            fn default() -> Self {
                Self(<$atomic_type>::new(0))
            }
        }
    };
    () => {}
}

le_type!(pub, Le16, u16);
le_type!(pub, Le32, u32);
le_type!(pub, Le64, u64);

#[cfg(target_has_atomic = "16")]
atomic_le!(pub, AtomicLe16, u16, AtomicU16);
#[cfg(target_has_atomic = "32")]
atomic_le!(pub, AtomicLe32, u32, AtomicU32);
#[cfg(target_has_atomic = "64")]
atomic_le!(pub, AtomicLe64, u64, AtomicU64);
