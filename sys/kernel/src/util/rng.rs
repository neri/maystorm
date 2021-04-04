// Random Number Generator

use crate::arch::cpu::*;
use core::num::{NonZeroU32, NonZeroU64};
use core::sync::atomic::*;

pub trait Rng {
    type Output;
    fn rand(&mut self) -> Result<Self::Output, ()>;
}

pub struct XorShift64 {
    seed: AtomicU64,
}

impl XorShift64 {
    pub const fn new(seed: NonZeroU64) -> Self {
        Self {
            seed: AtomicU64::new(seed.get()),
        }
    }

    pub fn next(&self) -> u64 {
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

impl Default for XorShift64 {
    fn default() -> Self {
        Self {
            seed: AtomicU64::new(88172645463325252),
        }
    }
}

impl Rng for XorShift64 {
    type Output = u64;
    fn rand(&mut self) -> Result<Self::Output, ()> {
        Ok(self.next())
    }
}

pub struct XorShift32 {
    seed: AtomicU32,
}

impl XorShift32 {
    pub const fn new(seed: NonZeroU32) -> Self {
        Self {
            seed: AtomicU32::new(seed.get()),
        }
    }

    pub fn next(&self) -> u32 {
        self.seed
            .fetch_update(Ordering::SeqCst, Ordering::Relaxed, |x| {
                let mut x = x;
                x = x ^ (x << 13);
                x = x ^ (x >> 17);
                x = x ^ (x << 5);
                Some(x)
            })
            .unwrap()
    }
}

impl Default for XorShift32 {
    fn default() -> Self {
        Self {
            seed: AtomicU32::new(2463534242),
        }
    }
}

impl Rng for XorShift32 {
    type Output = u32;
    fn rand(&mut self) -> Result<Self::Output, ()> {
        Ok(self.next())
    }
}

pub struct SecureRandom {
    _phantom: (),
}

impl SecureRandom {
    pub const fn new() -> Self {
        Self { _phantom: () }
    }

    pub fn next() -> Result<u64, ()> {
        Cpu::secure_rand()
    }
}

impl Rng for SecureRandom {
    type Output = u64;
    fn rand(&mut self) -> Result<Self::Output, ()> {
        Self::next()
    }
}
