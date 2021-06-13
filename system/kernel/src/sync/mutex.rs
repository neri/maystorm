//! A mutual exclusion primitive like std::sync::Mutex

use super::semaphore::Semaphore;
use super::*;
use core::{
    cell::UnsafeCell,
    mem,
    ops::{Deref, DerefMut},
    ptr,
};

/// A mutual exclusion primitive like std::sync::Mutex
pub struct Mutex<T: ?Sized> {
    inner: Semaphore,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}

unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}

impl<T> Mutex<T> {
    #[inline]
    pub const fn new(data: T) -> Self {
        Self {
            inner: Semaphore::new(1),
            data: UnsafeCell::new(data),
        }
    }

    #[inline]
    pub fn unlock(guard: MutexGuard<'_, T>) {
        drop(guard);
    }
}

impl<T: ?Sized> Mutex<T> {
    #[inline]
    pub fn lock(&self) -> LockResult<MutexGuard<'_, T>> {
        self.inner.lock();
        unsafe { MutexGuard::new(self) }
    }

    #[inline]
    pub fn try_lock(&self) -> TryLockResult<MutexGuard<'_, T>> {
        if self.inner.try_lock() {
            unsafe { Ok(MutexGuard::new(self)?) }
        } else {
            Err(TryLockError::WouldBlock)
        }
    }

    #[inline]
    pub fn into_inner(self) -> LockResult<T>
    where
        T: Sized,
    {
        unsafe {
            let (inner, data) = {
                let Mutex {
                    ref inner,
                    ref data,
                } = self;
                (ptr::read(inner), ptr::read(data))
            };
            mem::forget(self);
            drop(inner);
            Ok(data.into_inner())
        }
    }
}

impl<T> From<T> for Mutex<T> {
    #[inline]
    fn from(t: T) -> Self {
        Mutex::new(t)
    }
}

impl<T: ?Sized + Default> Default for Mutex<T> {
    #[inline]
    fn default() -> Mutex<T> {
        Mutex::new(Default::default())
    }
}

#[must_use = "if unused the Mutex will immediately unlock"]
pub struct MutexGuard<'a, T: ?Sized + 'a> {
    lock: &'a Mutex<T>,
}

impl<T: ?Sized> !Send for MutexGuard<'_, T> {}

unsafe impl<T: ?Sized + Sync> Sync for MutexGuard<'_, T> {}

impl<'a, T: ?Sized> MutexGuard<'a, T> {
    #[inline]
    unsafe fn new(lock: &'a Mutex<T>) -> LockResult<MutexGuard<'a, T>> {
        Ok(Self { lock })
    }
}

impl<T: ?Sized> Drop for MutexGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        self.lock.inner.unlock();
    }
}

impl<T: ?Sized> Deref for MutexGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> DerefMut for MutexGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}
