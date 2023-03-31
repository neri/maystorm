//! WebAssembly Runtime Library

#![cfg_attr(not(test), no_std)]
#![deny(unsafe_op_in_unsafe_fn)]
#![feature(const_convert)]
#![feature(const_mut_refs)]
#![feature(const_option_ext)]
#![feature(const_trait_impl)]
#![feature(slice_split_at_unchecked)]

mod wasm;
pub use crate::wasm::*;

pub mod intcode;
pub mod intr;
pub mod opcode;
pub mod stack;

#[cfg(test)]
mod tests;

extern crate alloc;
