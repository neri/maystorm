// Secure Random

use crate::arch::cpu::*;
use megstd::rand::*;

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
