#![no_std]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(asm_const)]
#![feature(const_mut_refs)]
#![feature(error_in_core)]
#![feature(iter_advance_by)]
#![feature(maybe_uninit_uninit_array)]
#![feature(naked_functions)]
#![feature(negative_impls)]
#![feature(step_trait)]
// #![feature(let_chains)]
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

use crate::system::System;
use bootprot::*;
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

static PANIC_GLOBAL_LOCK: Spinlock = Spinlock::new();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe {
        Hal::cpu().disable_interrupt();
        task::scheduler::Scheduler::freeze(true);
        PANIC_GLOBAL_LOCK.synchronized(|| {
            let stdout = System::log();
            stdout.set_attribute(0x4F);
            if let Some(thread) = task::scheduler::Scheduler::current_thread() {
                if let Some(name) = thread.name() {
                    let _ = write!(stdout, "thread '{}' ", name);
                } else {
                    let _ = write!(stdout, "thread {} ", thread.as_usize());
                }
            }
            let _ = writeln!(stdout, "{}", info);
        });
        Hal::cpu().stop();
    }
}
