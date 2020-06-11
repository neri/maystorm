// Spinlock

// pub use super::Synchronized;
use crate::myos::arch::cpu::Cpu;
use core::sync::atomic::*;

pub struct Spinlock {
    value: AtomicBool,
}

unsafe impl Sync for Spinlock {}

impl Spinlock {
    pub const fn new() -> Self {
        Self {
            value: AtomicBool::new(false),
        }
    }

    pub fn try_to_lock(&self) -> Result<(), ()> {
        if self.value.compare_and_swap(false, true, Ordering::Acquire) {
            Err(())
        } else {
            Ok(())
        }
    }

    pub fn lock(&self) {
        while self.value.compare_and_swap(false, true, Ordering::Acquire) {
            let mut count = 1;
            while self.value.load(Ordering::Relaxed) {
                for _ in 0..count {
                    Cpu::relax();
                }
                count = core::cmp::min(count << 1, 64);
            }
        }
    }

    #[inline]
    pub fn unlock(&self) {
        self.value.store(false, Ordering::Release);
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
