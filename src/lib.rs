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
#![no_std]

use alloc::boxed::Box;
use core::ffi::c_void;
use core::fmt::Write;
use core::panic::PanicInfo;
use core::ptr::NonNull;
use kernel::arch::cpu::Cpu;
use kernel::io::console::GraphicalConsole;
use kernel::io::graphics::*;
use kernel::sync::spinlock::Spinlock;

pub mod boot;
pub mod kernel;

extern crate alloc;

#[macro_use()]
extern crate bitflags;

static mut USE_EMCONSOLE: bool = true;
static mut PANIC_GLOBAL_LOCK: Spinlock = Spinlock::new();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe {
        PANIC_GLOBAL_LOCK.lock();
    }
    set_em_console(true);
    let stdout = stdout();
    stdout.set_cursor_enabled(false);
    stdout.set_attribute(0x17);
    println!("{}", info);
    unsafe {
        PANIC_GLOBAL_LOCK.unlock();
    }
    unsafe {
        Cpu::stop();
    }
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}

static mut BOOT_SCREEN: Option<Box<Bitmap>> = None;

static mut EMCONSOLE: Option<Box<GraphicalConsole>> = None;

static mut STDOUT: Option<Box<GraphicalConsole>> = None;

pub fn boot_screen() -> &'static Box<Bitmap> {
    unsafe { BOOT_SCREEN.as_ref().unwrap() }
}

pub fn stdout<'a>() -> &'static mut GraphicalConsole<'a> {
    unsafe {
        if USE_EMCONSOLE {
            EMCONSOLE.as_mut().unwrap()
        } else {
            STDOUT.as_mut().unwrap()
        }
    }
}

pub(crate) fn set_em_console(value: bool) {
    unsafe {
        USE_EMCONSOLE = value;
    }
}

pub fn set_stdout(console: Box<GraphicalConsole<'static>>) {
    unsafe {
        STDOUT = Some(console);
        set_em_console(false);
    }
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
