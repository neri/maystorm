//! MEG-OS Standard Graphics Drawing Library
#![no_std]
#![feature(const_fn_floating_point_arithmetic)]

extern crate alloc;

mod bitmap;
mod color;
mod coords;
mod drawable;
pub use bitmap::*;
pub use color::*;
pub use coords::*;
pub use drawable::*;

#[cfg(test)]
pub mod tests;
