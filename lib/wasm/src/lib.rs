// Wasm-O
#![no_std]
#![feature(try_reserve)]

mod wasm;
pub use crate::wasm::*;
pub mod intcode;
pub mod opcode;
pub mod stack;
pub mod wasmintr;

extern crate alloc;
