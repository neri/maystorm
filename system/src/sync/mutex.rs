//! A mutual exclusion primitive like std::sync::Mutex

use super::semaphore::BinarySemaphore;
use super::*;
use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
};

/// A mutual exclusion primitive like std::sync::Mutex
pub struct Mutex<T: ?Sized> {
    inner: BinarySemaphore,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}

unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}

impl<T> Mutex<T> {
    #[inline]
    pub const fn new(data: T) -> Self {
        Self {
            inner: BinarySemaphore::new(),
            data: UnsafeCell::new(data),
        }
    }
}

impl<T: ?Sized> Mutex<T> {
    #[inline]
    pub fn lock(&self) -> LockResult<MutexGuard<'_, T>> {
        self.inner.lock();
        MutexGuard::new(self)
    }

    #[inline]
    pub fn try_lock(&self) -> TryLockResult<MutexGuard<'_, T>> {
        if self.inner.try_lock() {
            Ok(MutexGuard::new(self)?)
        } else {
            Err(TryLockError::WouldBlock)
        }
    }

    #[inline]
    pub fn into_inner(self) -> LockResult<T>
    where
        T: Sized,
    {
        // TODO: poison
        Ok(self.data.into_inner())
    }

    #[inline]
    pub fn get_mut(&mut self) -> LockResult<&mut T> {
        // TODO: poison
        Ok(self.data.get_mut())
    }
}

impl<T> From<T> for Mutex<T> {
    #[inline]
    fn from(t: T) -> Self {
        Self::new(t)
    }
}

impl<T: ?Sized + Default> Default for Mutex<T> {
    #[inline]
    fn default() -> Self {
        Self::new(Default::default())
    }
}

#[must_use = "if unused the Mutex will immediately unlock"]
pub struct MutexGuard<'a, T: ?Sized + 'a> {
    mutex: &'a Mutex<T>,
}

impl<T: ?Sized> !Send for MutexGuard<'_, T> {}

unsafe impl<T: ?Sized + Sync> Sync for MutexGuard<'_, T> {}

impl<'a, T: ?Sized> MutexGuard<'a, T> {
    #[inline]
    fn new(mutex: &'a Mutex<T>) -> LockResult<MutexGuard<'a, T>> {
        Ok(Self { mutex })
    }
}

impl<T: ?Sized> Drop for MutexGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            self.mutex.inner.force_unlock();
        }
    }
}

impl<T: ?Sized> Deref for MutexGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<T: ?Sized> DerefMut for MutexGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mutex.data.get() }
    }
}
