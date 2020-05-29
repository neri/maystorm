// Spinlock

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

    pub fn lock(&self) {
        while self.value.compare_and_swap(false, true, Ordering::Relaxed) {
            let mut count = 1;
            while self.value.load(Ordering::Acquire) {
                for _ in 0..count {
                    // spin_loop_hint();
                    Cpu::relax();
                }
                count = core::cmp::min(count << 1, 64);
            }
        }
        fence(Ordering::Acquire);
    }

    pub fn unlock(&self) {
        self.value.store(false, Ordering::Relaxed);
        fence(Ordering::Release);
    }

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
