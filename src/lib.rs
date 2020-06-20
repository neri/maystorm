// My UEFI-Rust Lib
#![feature(abi_efiapi)]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(asm)]
#![feature(const_fn)]
#![feature(core_intrinsics)]
#![feature(lang_items)]
#![feature(new_uninit)]
#![feature(panic_info_message)]
#![no_std]

use alloc::boxed::Box;
use core::ffi::c_void;
use core::fmt::Write;
use core::panic::PanicInfo;
use core::ptr::NonNull;
use myos::io::console::GraphicalConsole;
use myos::io::graphics::*;
use myos::sync::spinlock::Spinlock;
use myos::*;

pub mod boot;
pub mod myos;

extern crate alloc;

#[macro_use()]
extern crate bitflags;

static mut PANIC_GLOBAL_LOCK: Spinlock = Spinlock::new();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe {
        PANIC_GLOBAL_LOCK.lock();
    }
    stdout().set_cursor_enabled(false);
    stdout().set_attribute(0x17);
    println!("{}", info);
    unsafe {
        PANIC_GLOBAL_LOCK.unlock();
    }
    unsafe {
        arch::cpu::Cpu::stop();
    }
}

static mut STDOUT: Option<Box<GraphicalConsole>> = None;

pub fn stdout<'a>() -> &'static mut GraphicalConsole<'a> {
    unsafe { STDOUT.as_mut().unwrap() }
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
