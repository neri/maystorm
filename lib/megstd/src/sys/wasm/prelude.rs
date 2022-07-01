use crate::sys::syscall::*;
pub use core::fmt;

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        {
            use core::fmt::Write;
            let _ = write!(OsPrint(), $($arg)*);
        }
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

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // println!("{}", info);
    os_exit();
}

pub struct OsPrint();

impl fmt::Write for OsPrint {
    #[inline]
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        os_print(s);
        Ok(())
    }
}
