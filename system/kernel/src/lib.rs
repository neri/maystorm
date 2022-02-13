#![no_std]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(async_closure)]
#![feature(box_into_inner)]
#![feature(const_btree_new)]
#![feature(const_fn_trait_bound)]
#![feature(const_mut_refs)]
#![feature(control_flow_enum)]
#![feature(core_intrinsics)]
#![feature(lang_items)]
#![feature(maybe_uninit_uninit_array)]
#![feature(negative_impls)]
#![feature(new_uninit)]
#![feature(option_result_contains)]
#![feature(panic_info_message)]

#[macro_use]
pub mod arch;
pub mod dev;
pub mod drivers;
pub mod fs;
pub mod fw;
pub mod io;
pub mod log;
pub mod mem;
pub mod r;
pub mod res;
pub mod rt;
pub mod sync;
pub mod system;
pub mod task;
pub mod ui;
pub mod user;

use crate::arch::cpu::Cpu;
use crate::system::System;
use alloc::boxed::Box;
use bootprot::*;
use core::fmt::Write;
use core::panic::PanicInfo;

extern crate alloc;
extern crate bitflags;

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        write!(system::System::stdout(), $($arg)*).unwrap()
    };
}

#[macro_export]
macro_rules! println {
    ($fmt:expr) => {
        print!(concat!($fmt, "\r\n"))
    };
    ($fmt:expr, $($arg:tt)*) => {
        print!(concat!($fmt, "\r\n"), $($arg)*)
    };
}

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        let _ = writeln!(log::Log::new(), $($arg)*).unwrap();
    };
}

static mut PANIC_GLOBAL_LOCK: sync::spinlock::Spinlock = sync::spinlock::Spinlock::new();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    use io::tty::*;
    unsafe {
        Cpu::disable_interrupt();
        let _ = task::scheduler::Scheduler::freeze(true);
        PANIC_GLOBAL_LOCK.synchronized(|| {
            let stdout = System::em_console();
            stdout.set_attribute(0x4F);
            let _ = writeln!(stdout, " = Guru Meditation = ");
            if let Some(thread) = task::scheduler::Scheduler::current_thread() {
                if let Some(name) = thread.name() {
                    let _ = write!(stdout, "thread '{}' ", name);
                } else {
                    let _ = write!(stdout, "thread {} ", thread.as_usize());
                }
            }
            let _ = writeln!(stdout, "{}", info);
        });
        Cpu::stop();
    }
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}

#[repr(transparent)]
pub struct HexDump<'a>(pub &'a [u8]);

impl core::fmt::Debug for HexDump<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for c in self.0.iter() {
            let _ = write!(f, " {:02x}", *c);
        }
        writeln!(f, "")
    }
}
