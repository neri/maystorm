// Atomic Bit Operations

// use crate::kernel::num::*;
// pub use crate::kernel::num::*;
// use core::mem::transmute;
use core::ops::*;
use core::sync::atomic::*;

pub trait AtomicBits {
    type ValueType;
    fn contains(&self, bits: Self::ValueType) -> bool;
    fn test_and_set(&self, bits: Self::ValueType) -> bool;
    fn test_and_clear(&self, bits: Self::ValueType) -> bool;
}

impl AtomicBits for AtomicU8 {
    type ValueType = u8;

    fn contains(&self, bits: Self::ValueType) -> bool {
        (self.load(Ordering::Relaxed) & bits) == bits
    }

    fn test_and_set(&self, bits: Self::ValueType) -> bool {
        self.fetch_update(Ordering::AcqRel, Ordering::Acquire, |x: Self::ValueType| {
            if (x & bits) == bits {
                None
            } else {
                Some(x | bits)
            }
        })
        .is_err()
    }

    fn test_and_clear(&self, bits: Self::ValueType) -> bool {
        self.fetch_update(Ordering::AcqRel, Ordering::Acquire, |x: Self::ValueType| {
            if (x & bits) == bits {
                Some(x & !bits)
            } else {
                None
            }
        })
        .is_ok()
    }
}
