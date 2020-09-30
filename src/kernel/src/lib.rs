// My OS

#![feature(abi_efiapi)]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(asm)]
#![feature(const_fn)]
#![feature(core_intrinsics)]
#![feature(lang_items)]
#![feature(new_uninit)]
#![feature(panic_info_message)]
#![feature(option_result_contains)]
#![feature(duration_zero)]
#![no_std]

pub mod arch;
pub mod bus;
// pub mod expr;
pub mod dev;
pub mod io;
pub mod mem;
pub mod num;
pub mod sync;
pub mod system;
pub mod task;
pub mod window;

use crate::arch::cpu::Cpu;
use crate::io::graphics::GraphicalConsole;
use crate::io::graphics::*;
use crate::io::tty::*;
use crate::mem::memory::*;
use crate::sync::spinlock::Spinlock;
use crate::task::scheduler::*;
use alloc::boxed::Box;
use bootprot::*;
use core::ffi::c_void;
// use core::fmt::Write;
use core::panic::PanicInfo;

extern crate alloc;

#[macro_use()]
extern crate bitflags;

pub fn stdout<'a>() -> &'a mut Box<dyn Tty> {
    system::System::stdout()
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        write!(stdout(), $($arg)*).unwrap()
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
macro_rules! myos_entry {
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

static mut PANIC_GLOBAL_LOCK: Spinlock = Spinlock::new();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe {
        PANIC_GLOBAL_LOCK.lock();
    }
    // set_em_console(true);
    let stdout = stdout();
    stdout.set_cursor_enabled(false);
    stdout.set_attribute(0x17);
    println!("{}", info);
    unsafe {
        let _ = MyScheduler::freeze(true);
        PANIC_GLOBAL_LOCK.unlock();
        Cpu::stop();
    }
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}
