// Spinlock

use crate::arch::cpu::*;
use core::sync::atomic::*;

#[derive(Default)]
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
            let mut spin_loop = SpinLoopWait::new();
            while self.value.load(Ordering::Relaxed) {
                spin_loop.wait();
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

#[derive(Debug, Default)]
pub struct SpinLoopWait(usize);

impl SpinLoopWait {
    pub const fn new() -> Self {
        Self(0)
    }

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
