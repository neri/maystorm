//! MEG-OS Standard Graphics Drawing Library
#![no_std]
#![feature(const_fn_floating_point_arithmetic)]
#![feature(const_mut_refs)]
#![feature(const_swap)]
#![feature(const_trait_impl)]
#![feature(core_intrinsics)]

extern crate alloc;

mod bitmap;
mod color;
mod coords;
mod drawable;
pub use bitmap::*;
pub use color::*;
pub use coords::*;
pub use drawable::*;
pub mod vertex;

#[cfg(test)]
pub mod tests;
