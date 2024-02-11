//! MEG-OS Standard Graphics Drawing Library
#![no_std]
// #![feature(const_fn_floating_point_arithmetic)]

extern crate alloc;

/// Preferred Signed Integer
pub type GlSInt = i32;
/// Preferred Unsigned Integer
pub type GlUInt = u32;
/// Preferred Floating Point Number
pub type GlFloat = f64;

mod bitmap;
mod color;
mod coords;
pub use bitmap::*;
pub use color::*;
pub use coords::*;

pub mod rotation;
pub mod vec;

#[cfg(test)]
pub mod tests;
