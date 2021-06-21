//! A reader-writer lock like std::sync::RwLock

use super::signal::SignallingObject;
use super::*;
use alloc::boxed::Box;
use core::{
    cell::UnsafeCell,
    mem,
    ops::{Deref, DerefMut},
    ptr,
    sync::atomic::*,
};

/// A reader-writer lock like std::sync::RwLock
pub struct RwLock<T: ?Sized> {
    inner: Box<RwLockInner>,
    data: UnsafeCell<T>,
}

impl<T> RwLock<T> {
    #[inline]
    pub fn new(t: T) -> Self {
        Self {
            inner: Box::new(RwLockInner::new()),
            data: UnsafeCell::new(t),
        }
    }
}

unsafe impl<T: ?Sized + Send> Send for RwLock<T> {}

unsafe impl<T: ?Sized + Send + Sync> Sync for RwLock<T> {}

impl<T: ?Sized> RwLock<T> {
    #[inline]
    pub fn read(&self) -> LockResult<RwLockReadGuard<'_, T>> {
        unsafe {
            self.inner.read();
            RwLockReadGuard::new(self)
        }
    }

    #[inline]
    pub fn try_read(&self) -> TryLockResult<RwLockReadGuard<'_, T>> {
        unsafe {
            if self.inner.try_read() {
                Ok(RwLockReadGuard::new(self)?)
            } else {
                Err(TryLockError::WouldBlock)
            }
        }
    }

    #[inline]
    pub fn write(&self) -> LockResult<RwLockWriteGuard<'_, T>> {
        unsafe {
            self.inner.write();
            RwLockWriteGuard::new(self)
        }
    }

    #[inline]
    pub fn try_write(&self) -> TryLockResult<RwLockWriteGuard<'_, T>> {
        unsafe {
            if self.inner.try_write() {
                Ok(RwLockWriteGuard::new(self)?)
            } else {
                Err(TryLockError::WouldBlock)
            }
        }
    }

    #[inline]
    pub fn is_poisoned(&self) -> bool {
        // TODO: NOT YET IMPLEMENTED
        false
    }

    #[inline]
    pub fn into_inner(self) -> LockResult<T>
    where
        T: Sized,
    {
        // TODO: poison
        unsafe {
            let (inner, data) = {
                let RwLock {
                    ref inner,
                    ref data,
                } = self;
                (ptr::read(inner), ptr::read(data))
            };
            mem::forget(self);
            inner.destroy();
            drop(inner);

            Ok(data.into_inner())
        }
    }

    #[inline]
    pub fn get_mut(&mut self) -> LockResult<&mut T> {
        // TODO: poison
        Ok(self.data.get_mut())
    }
}

impl<T: Default> Default for RwLock<T> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: ?Sized> Drop for RwLock<T> {
    fn drop(&mut self) {
        todo!()
    }
}

impl<T> From<T> for RwLock<T> {
    #[inline]
    fn from(t: T) -> Self {
        Self::new(t)
    }
}

#[must_use = "if unused the RwLock will immediately unlock"]
pub struct RwLockReadGuard<'a, T: ?Sized + 'a> {
    lock: &'a RwLock<T>,
}

impl<T: ?Sized> !Send for RwLockReadGuard<'_, T> {}

unsafe impl<T: ?Sized + Sync> Sync for RwLockReadGuard<'_, T> {}

impl<'rwlock, T: ?Sized> RwLockReadGuard<'rwlock, T> {
    unsafe fn new(lock: &'rwlock RwLock<T>) -> LockResult<RwLockReadGuard<'rwlock, T>> {
        Ok(Self { lock })
    }
}

impl<T: ?Sized> Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> Drop for RwLockReadGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            self.lock.inner.read_unlock();
        }
    }
}

#[must_use = "if unused the RwLock will immediately unlock"]
pub struct RwLockWriteGuard<'a, T: ?Sized + 'a> {
    lock: &'a RwLock<T>,
    // poison: poison::Guard,
}

impl<T: ?Sized> !Send for RwLockWriteGuard<'_, T> {}

unsafe impl<T: ?Sized + Sync> Sync for RwLockWriteGuard<'_, T> {}

impl<'rwlock, T: ?Sized> RwLockWriteGuard<'rwlock, T> {
    unsafe fn new(lock: &'rwlock RwLock<T>) -> LockResult<RwLockWriteGuard<'rwlock, T>> {
        Ok(Self { lock })
    }
}

impl<T: ?Sized> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> DerefMut for RwLockWriteGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T: ?Sized> Drop for RwLockWriteGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            self.lock.inner.write_unlock();
        }
    }
}

struct RwLockInner {
    data: AtomicUsize,
    signal: SignallingObject,
}

impl RwLockInner {
    const LOCK_WRITE: usize = 1;
    const LOCK_COUNT: usize = 2;

    #[inline]
    const fn new() -> Self {
        Self {
            data: AtomicUsize::new(0),
            signal: SignallingObject::new(None),
        }
    }

    #[inline]
    fn destroy(&self) {
        // TODO: ?
    }

    #[inline]
    unsafe fn read(&self) {
        self.signal.wait_for(|| self.try_read());
    }

    #[inline]
    unsafe fn write(&self) {
        self.signal.wait_for(|| self.try_write());
    }

    #[inline]
    fn try_read(&self) -> bool {
        self.data
            .fetch_update(Ordering::SeqCst, Ordering::Relaxed, |v| {
                if (v & Self::LOCK_WRITE) == 0 {
                    Some(v + Self::LOCK_COUNT)
                } else {
                    None
                }
            })
            .is_ok()
    }

    #[inline]
    fn try_write(&self) -> bool {
        self.data
            .fetch_update(Ordering::SeqCst, Ordering::Relaxed, |v| {
                if v == 0 {
                    Some(Self::LOCK_WRITE)
                } else {
                    None
                }
            })
            .is_ok()
    }

    #[inline]
    #[track_caller]
    unsafe fn read_unlock(&self) {
        let _ = self
            .data
            .fetch_update(Ordering::SeqCst, Ordering::Relaxed, |v| {
                if v >= Self::LOCK_COUNT {
                    Some(v - Self::LOCK_COUNT)
                } else {
                    None
                }
            });
        if self.data.load(Ordering::Relaxed) == 0 {
            self.signal.signal()
        }
    }

    #[inline]
    #[track_caller]
    unsafe fn write_unlock(&self) {
        let _ = self
            .data
            .fetch_update(Ordering::SeqCst, Ordering::Relaxed, |v| {
                if v == Self::LOCK_WRITE {
                    Some(0)
                } else {
                    None
                }
            });
        if self.data.load(Ordering::Relaxed) == 0 {
            self.signal.signal()
        }
    }
}
