#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]
#![feature(generic_arg_infer)]

// use crate::debug::console::DebugConsole;
// use core::fmt::Write;
use uefi::prelude::*;

pub mod invocation;
pub mod loader;
pub mod page;
