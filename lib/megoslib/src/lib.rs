// MEG-OS Library
#![no_std]
#![feature(alloc_error_handler)]
#![feature(asm)]

pub mod bitmap;
pub mod game;
pub mod os_alloc;
pub mod syscall;
pub mod window;

use core::fmt::*;
pub use megstd;
pub use syscall::*;
extern crate alloc;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("{}", info);
    os_exit();
}

pub struct OsPrint {}

impl OsPrint {
    pub const fn new() -> Self {
        Self {}
    }
}

impl Write for OsPrint {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        os_print(s);
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        { let _ = write!(OsPrint::new(), $($arg)*); }
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
