//! Spinlock

use crate::arch::cpu::*;
use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::*,
};

#[derive(Default)]
pub struct Spinlock {
    value: AtomicBool,
}

impl Spinlock {
    const LOCKED_VALUE: bool = true;
    const UNLOCKED_VALUE: bool = false;

    #[inline]
    pub const fn new() -> Self {
        Self {
            value: AtomicBool::new(Self::UNLOCKED_VALUE),
        }
    }

    #[inline]
    pub fn try_lock(&self) -> Result<(), ()> {
        match self.value.compare_exchange(
            Self::UNLOCKED_VALUE,
            Self::LOCKED_VALUE,
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        }
    }

    pub fn lock(&self) {
        while self
            .value
            .compare_exchange(
                Self::UNLOCKED_VALUE,
                Self::LOCKED_VALUE,
                Ordering::Acquire,
                Ordering::Relaxed,
            )
            .is_err()
        {
            let mut spin_loop = SpinLoopWait::new();
            while self.value.load(Ordering::Relaxed) {
                spin_loop.wait();
            }
        }
    }

    #[inline]
    pub unsafe fn force_unlock(&self) {
        self.value.store(Self::UNLOCKED_VALUE, Ordering::Release);
    }

    #[inline]
    pub fn synchronized<F, R>(&self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        self.lock();
        let result = f();
        unsafe {
            self.force_unlock();
        }
        result
    }
}

#[derive(Debug, Default)]
pub struct SpinLoopWait(usize);

impl SpinLoopWait {
    #[inline]
    pub const fn new() -> Self {
        Self(0)
    }

    #[inline]
    pub fn reset(&mut self) {
        self.0 = 0;
    }

    pub fn wait(&mut self) {
        let count = self.0;
        for _ in 0..(1 << count) {
            Cpu::spin_loop_hint();
        }
        if count < 6 {
            self.0 += 1;
        }
    }
}

/// Mutual exclusion primitives like std::sync::Mutex implemented in Spinlock
pub struct SpinMutex<T: ?Sized> {
    lock: Spinlock,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Sync for SpinMutex<T> {}

unsafe impl<T: ?Sized + Send> Send for SpinMutex<T> {}

impl<T> SpinMutex<T> {
    #[inline]
    pub const fn new(data: T) -> Self {
        Self {
            lock: Spinlock::new(),
            data: UnsafeCell::new(data),
        }
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

impl<T: ?Sized> SpinMutex<T> {
    #[inline]
    pub fn try_lock(&self) -> Option<SpinMutexGuard<T>> {
        let interrupt_guard = unsafe { Cpu::interrupt_guard() };
        self.lock
            .try_lock()
            .map(|_| SpinMutexGuard::new(self, interrupt_guard))
            .ok()
    }

    #[inline]
    pub fn lock<'a>(&'a self) -> SpinMutexGuard<'a, T> {
        let interrupt_guard = unsafe { Cpu::interrupt_guard() };
        self.lock.lock();
        SpinMutexGuard::new(self, interrupt_guard)
    }

    #[inline]
    pub unsafe fn force_unlock(&self) {
        self.lock.force_unlock();
    }
}

impl<T> From<T> for SpinMutex<T> {
    #[inline]
    fn from(t: T) -> Self {
        Self::new(t)
    }
}

impl<T: ?Sized + Default> Default for SpinMutex<T> {
    #[inline]
    fn default() -> Self {
        Self::new(Default::default())
    }
}

#[must_use = "if unused the Mutex will immediately unlock"]
pub struct SpinMutexGuard<'a, T: ?Sized + 'a> {
    mutex: &'a SpinMutex<T>,
    #[allow(dead_code)]
    interrupt_guard: InterruptGuard,
}

impl<T: ?Sized> !Send for SpinMutexGuard<'_, T> {}

impl<T: ?Sized> !Sync for SpinMutexGuard<'_, T> {}
// unsafe impl<T: ?Sized + Sync> Sync for SpinMutexGuard<'_, T> {}

impl<'a, T: ?Sized> SpinMutexGuard<'a, T> {
    #[inline]
    fn new(mutex: &'a SpinMutex<T>, interrupt_guard: InterruptGuard) -> SpinMutexGuard<'a, T> {
        Self {
            mutex,
            interrupt_guard,
        }
    }
}

impl<T: ?Sized> Drop for SpinMutexGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            self.mutex.force_unlock();
        }
    }
}

impl<T: ?Sized> Deref for SpinMutexGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<T: ?Sized> DerefMut for SpinMutexGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mutex.data.get() }
    }
}
