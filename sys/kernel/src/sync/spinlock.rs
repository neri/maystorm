// Spinlock

use crate::arch::cpu::*;
use core::sync::atomic::*;

#[derive(Default)]
pub struct Spinlock {
    value: AtomicBool,
}

impl Spinlock {
    #[inline]
    pub const fn new() -> Self {
        Self {
            value: AtomicBool::new(false),
        }
    }

    #[inline]
    pub fn try_lock(&self) -> Result<(), ()> {
        match self
            .value
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        }
    }

    pub fn lock(&self) {
        while self
            .value
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
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
