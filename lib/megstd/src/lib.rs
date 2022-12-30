//! MEG-OS standard library like `std`
#![no_std]
#![feature(alloc_error_handler)]
#![feature(asm_experimental_arch)]
#![feature(const_mut_refs)]
#![feature(const_swap)]
#![feature(const_trait_impl)]

#[macro_use]
pub mod sys;

pub use meggl as drawing;
pub mod error;
pub mod fs;
pub mod game;
pub mod io;
pub mod mem;
pub mod osstr;
pub mod path;
pub mod rand;
pub mod string;
pub mod time;
pub mod uuid;

#[cfg(feature = "window")]
pub mod window {
    pub use crate::sys::window::*;
}

extern crate alloc;

pub use prelude::*;
mod prelude {
    pub use crate::{osstr::*, sys::prelude::*};
    pub use alloc::{
        borrow::ToOwned, boxed::Box, collections::btree_map::BTreeMap, format, rc::Rc,
        string::String, string::ToString, sync::Arc, vec::Vec,
    };
}
