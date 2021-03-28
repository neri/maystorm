// MEG-OS standard library
#![no_std]
#![feature(const_fn_transmute)]

mod osstr;
pub use osstr::*;
pub mod drawing;
pub mod error;
pub mod fs;
pub mod io;
pub mod path;
pub mod sys;

extern crate alloc;
