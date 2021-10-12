//! MEG-OS standard library like std

#![no_std]
#![feature(const_mut_refs)]
#![feature(const_raw_ptr_deref)]

mod osstr;
pub use osstr::*;
pub mod drawing;
pub mod error;
pub mod fs;
pub mod game;
pub mod io;
pub mod mem;
pub mod path;
pub mod rand;
pub mod string;
pub mod sys;
pub mod time;
pub mod uuid;

extern crate alloc;

pub use prelude::*;
mod prelude {
    //
}
