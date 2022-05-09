//! MEG-OS standard library like std
#![no_std]
#![feature(const_mut_refs)]
#![feature(alloc_error_handler)]
#![feature(asm_experimental_arch)]

#[macro_use]
pub mod sys;

pub mod drawing;
pub mod error;
pub mod fs;
pub mod game;
pub mod io;
pub mod mem;
pub mod path;
pub mod rand;
pub mod string;
pub mod time;
pub mod uuid;

pub use osstr::*;
mod osstr;

#[cfg(feature = "window")]
pub mod window {
    pub use crate::sys::window::*;
}

extern crate alloc;

pub use prelude::*;
mod prelude {
    pub use crate::sys::prelude::*;
}
