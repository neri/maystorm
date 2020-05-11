// My UEFI-Rust Lib
#![feature(panic_info_message)]
#![feature(abi_efiapi)]
#![feature(lang_items)]
#![no_std]

use core::panic::PanicInfo;
use core::ptr::NonNull;
use uefi::prelude::*;
use uefi::proto::console::text::Output;

pub mod console;
pub mod font;
pub mod gs;
pub mod num;

static mut LOGGER: Option<uefi::logger::Logger> = None;

static mut BOOT_SERVICES: Option<NonNull<BootServices>> = None;

static mut STDOUT: Option<NonNull<Output>> = None;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    log::error!("{}", info);
    loop {}
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}

pub fn init(st: &SystemTable<Boot>) {
    unsafe {
        BOOT_SERVICES = NonNull::new(st.boot_services() as *const _ as *mut _);

        STDOUT = NonNull::new(st.stdout() as *const _ as *mut _);

        let logger = {
            LOGGER = Some(uefi::logger::Logger::new(stdout()));
            LOGGER.as_ref().unwrap()
        };
        log::set_logger(logger).unwrap();
        log::set_max_level(log::LevelFilter::Info);
    }
}

pub fn stdout() -> &'static mut Output<'static> {
    unsafe { &mut *(STDOUT.unwrap().as_ptr()) }
}

pub fn boot_services() -> &'static mut BootServices {
    unsafe { &mut *(BOOT_SERVICES.unwrap().as_ptr()) }
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
