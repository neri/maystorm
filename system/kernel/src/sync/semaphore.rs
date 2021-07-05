// Semaphore

use super::signal::SignallingObject;
use crate::arch::cpu::Cpu;
use core::sync::atomic::*;

/// counting semaphore
pub struct Semaphore {
    value: AtomicUsize,
    signal: SignallingObject,
}

impl Semaphore {
    #[inline]
    pub const fn new(value: usize) -> Self {
        Self {
            value: AtomicUsize::new(value),
            signal: SignallingObject::new(None),
        }
    }

    #[inline]
    pub fn try_lock(&self) -> bool {
        Cpu::interlocked_fetch_update(&self.value, |v| if v >= 1 { Some(v - 1) } else { None })
            .is_ok()
    }

    #[inline]
    pub fn lock(&self) {
        self.wait()
    }

    #[inline]
    pub fn unlock(&self) {
        self.signal()
    }

    #[inline]
    pub fn wait(&self) {
        self.signal.wait_for(|| self.try_lock());
    }

    #[inline]
    pub fn signal(&self) {
        let _ = Cpu::interlocked_increment(&self.value);
        let _ = self.signal.signal();
    }

    #[inline]
    pub fn synchronized<F, R>(&self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        self.wait();
        let result = f();
        self.signal();
        result
    }
}

/// binary semaphore
pub struct BinarySemaphore {
    value: AtomicBool,
    signal: SignallingObject,
}

impl BinarySemaphore {
    #[inline]
    pub const fn new(value: bool) -> Self {
        Self {
            value: AtomicBool::new(value),
            signal: SignallingObject::new(None),
        }
    }

    #[inline]
    pub fn try_lock(&self) -> bool {
        self.value
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
    }

    #[inline]
    pub fn lock(&self) {
        self.signal.wait_for(|| self.try_lock())
    }

    #[inline]
    pub fn unlock(&self) {
        self.value.store(false, Ordering::SeqCst);
        let _ = self.signal.signal();
    }

    #[inline]
    pub fn synchronized<F, R>(&self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        self.lock();
        let result = f();
        self.unlock();
        result
    }
}

impl Default for BinarySemaphore {
    #[inline]
    fn default() -> Self {
        Self::new(false)
    }
}
