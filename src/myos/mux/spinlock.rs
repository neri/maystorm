// Spinlock

use core::sync::atomic::*;

pub struct Spinlock {
    value: AtomicUsize,
}

impl Spinlock {
    const RELEASED: usize = 0;
    const LOCKED: usize = 1;
    pub const fn new() -> Self {
        Self {
            value: AtomicUsize::new(Self::RELEASED),
        }
    }

    pub fn lock(&mut self) {
        loop {
            match self.value.compare_exchange(
                Self::RELEASED,
                Self::LOCKED,
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                Ok(_) => return,
                Err(_) => (),
            }
            loop {
                let mut count = 1;
                match self.value.load(Ordering::Acquire) {
                    Self::RELEASED => break,
                    _ => {
                        for _ in 0..count {
                            spin_loop_hint();
                        }
                        count = core::cmp::min(count << 1, 64);
                    }
                }
            }
        }
    }

    pub fn unlock(&mut self) {
        self.value.store(Self::RELEASED, Ordering::Release);
    }
}
