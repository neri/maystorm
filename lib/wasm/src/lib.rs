//! WebAssembly Runtime Library

#![no_std]
#![feature(const_mut_refs)]

mod wasm;
pub use crate::wasm::*;

pub mod intcode;
pub mod intr;
pub mod opcode;
pub mod stack;

#[cfg(test)]
mod tests;

extern crate alloc;
