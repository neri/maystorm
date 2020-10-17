// Random Number Generator

use core::sync::atomic::*;

pub trait RandomNumberGenerator {
    fn next(&mut self) -> u64;
}

pub struct XorShift {
    seed: AtomicU64,
}

impl XorShift {
    pub const fn new(seed: u64) -> Self {
        Self {
            seed: AtomicU64::new(seed),
        }
    }
}

impl Default for XorShift {
    fn default() -> Self {
        Self::new(88172645463325252)
    }
}

impl RandomNumberGenerator for XorShift {
    fn next(&mut self) -> u64 {
        self.seed
            .fetch_update(Ordering::SeqCst, Ordering::Relaxed, |x| {
                let mut x = x;
                x = x ^ (x << 7);
                x = x ^ (x >> 9);
                Some(x)
            })
            .unwrap()
    }
}
