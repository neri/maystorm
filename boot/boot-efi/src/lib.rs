#![no_std]
#![feature(const_mut_refs)]
#![feature(const_trait_impl)]
#![feature(core_intrinsics)]
#![feature(generic_arg_infer)]

// use crate::debug::console::DebugConsole;
// use core::fmt::Write;
use uefi::prelude::*;

pub mod invocation;
pub mod loader;
pub mod page;
