// MyOS Library
#![no_std]

pub mod graphics;
pub mod syscall;
pub mod window;

use core::fmt::*;
pub use syscall::*;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

pub struct OsPrint {
    _phantom: (),
}

impl OsPrint {
    pub const fn new() -> Self {
        Self { _phantom: () }
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
