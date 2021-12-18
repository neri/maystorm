#![no_std]
#![feature(core_intrinsics)]

// use crate::debug::console::DebugConsole;
// use core::fmt::Write;
use uefi::prelude::*;

pub mod blob;
pub mod invocation;
pub mod loader;
pub mod page;
