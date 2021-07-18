//! WebAssembly Runtime Library

#![no_std]
#![feature(const_raw_ptr_deref)]
#![feature(try_reserve)]
#![feature(const_mut_refs)]

mod wasm;
pub use crate::wasm::*;

pub mod intcode;
pub mod opcode;
pub mod stack;
pub mod wasmintr;

#[cfg(test)]
mod tests;

extern crate alloc;
