#![no_std]
#![feature(abi_x86_interrupt)]
#![feature(asm_const)]
#![feature(naked_functions)]
//
#![feature(alloc_error_handler)]
#![feature(error_in_core)]
//
#![feature(const_mut_refs)]
// #![feature(const_trait_impl)]
#![feature(iter_advance_by)]
#![feature(maybe_uninit_uninit_array)]
#![feature(negative_impls)]
#![feature(step_trait)]
#![feature(trait_alias)]

#[macro_use]
pub mod arch;

#[macro_use]
pub mod hal;

pub mod drivers;
pub mod fs;
pub mod fw;
pub mod io;
pub mod mem;
pub mod r;
pub mod res;
pub mod rt;
pub mod sync;
pub mod system;
pub mod task;
pub mod ui;

#[macro_use]
pub mod utils;

#[path = "init/init.rs"]
pub mod init;

pub use crate::hal::*;

use core::{fmt::Write, panic::PanicInfo};
pub use megstd::prelude::*;

extern crate alloc;

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        let _ = write!(system::System::stdout(), $($arg)*);
    }};
}

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {{
        let _ = writeln!(system::System::stdout(), $($arg)*);
    }};
}

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let _ = writeln!(utils::Log::new(), $($arg)*);
    }};
}
