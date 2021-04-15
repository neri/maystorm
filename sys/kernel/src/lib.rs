// My OS

#![no_std]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(asm)]
#![feature(cfg_target_has_atomic)]
#![feature(const_fn)]
#![feature(const_mut_refs)]
#![feature(core_intrinsics)]
#![feature(global_asm)]
#![feature(lang_items)]
#![feature(new_uninit)]
#![feature(option_result_contains)]
#![feature(panic_info_message)]
#![feature(try_reserve)]

pub mod arch;
pub mod bus;
pub mod dev;
pub mod fonts;
pub mod fs;
pub mod io;
pub mod mem;
pub mod rt;
pub mod sync;
pub mod system;
pub mod task;
pub mod user;
pub mod util;
pub mod uuid;
pub mod window;

use crate::arch::cpu::Cpu;
use crate::system::System;
use alloc::boxed::Box;
use bootprot::*;
use core::fmt::Write;
use core::panic::PanicInfo;

extern crate alloc;

#[macro_use()]
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
macro_rules! entry {
    ($path:path) => {
        #[no_mangle]
        pub fn efi_main(info: &BootInfo, mbz: usize) -> usize {
            let f: fn() = $path;
            unsafe { kernel_entry(info, mbz, f) }
        }
    };
}

/// Entry Point of The Kernel
#[inline]
pub unsafe fn kernel_entry(info: &BootInfo, mbz: usize, f: fn() -> ()) -> usize {
    if mbz != 0 {
        // EFI Stub is no longer supported
        return !(isize::MAX as usize) + 1;
    }
    system::System::init(info, f);
}

static mut PANIC_GLOBAL_LOCK: sync::spinlock::Spinlock = sync::spinlock::Spinlock::new();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe {
        PANIC_GLOBAL_LOCK.lock();
    }
    // stdout.set_attribute(0x17);
    if let Some(thread) = task::scheduler::Scheduler::current_thread() {
        if let Some(name) = thread.name() {
            let _ = write!(System::em_console(), "thread '{}' ", name);
        } else {
            let _ = write!(System::em_console(), "thread {} ", thread.as_usize());
        }
    }
    let _ = writeln!(System::em_console(), "{}", info);
    unsafe {
        let _ = task::scheduler::Scheduler::freeze(true);
        PANIC_GLOBAL_LOCK.unlock();
        Cpu::stop();
    }
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}
