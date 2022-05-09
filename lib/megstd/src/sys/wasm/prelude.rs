use crate::sys::syscall::*;
pub use core::fmt::*;

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        { let _ = write!(OsPrint(), $($arg)*); }
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

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("{}", info);
    os_exit();
}

pub struct OsPrint();

impl Write for OsPrint {
    #[inline]
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        os_print(s);
        Ok(())
    }
}
