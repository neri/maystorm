//! Memory manager

pub mod alloc;
pub mod mmio;
pub mod slab;

mod mm;
pub use mm::*;
