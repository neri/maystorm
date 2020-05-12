// My UEFI-Rust Lib
#![feature(panic_info_message)]
#![feature(abi_efiapi)]
#![feature(lang_items)]
#![feature(alloc_error_handler)]
#![no_std]

use core::panic::PanicInfo;
use core::ptr::NonNull;
use uefi::prelude::*;
// use core::fmt::Write;

extern crate alloc;
// use alloc::boxed::Box;

use crate::graphics::FrameBuffer;
use crate::console::GraphicalConsole;

pub mod console;
pub mod font;
pub mod graphics;
pub mod num;

static mut LOGGER: Option<uefi::logger::Logger> = None;

static mut STDOUT: Option<NonNull<GraphicalConsole>> = None;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    log::error!("{}", info);
    loop {}
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}

pub fn init<F>(handle: Handle, st: SystemTable<Boot>, main: F) -> Status
    where F: Fn(Handle, SystemTable<Boot>) -> Status {
    unsafe {
        uefi::alloc::init(&st.boot_services());

        let logger = {
            LOGGER = Some(uefi::logger::Logger::new(st.stdout()));
            LOGGER.as_ref().unwrap()
        };
        log::set_logger(logger).unwrap();
        log::set_max_level(log::LevelFilter::Info);
    }
    let bs = st.boot_services();
    if let Ok(gop) = bs.locate_protocol::<uefi::proto::console::gop::GraphicsOutput>() {
        let gop = gop.unwrap();
        let gop = unsafe { &mut *gop.get() };
       unsafe {
            let fb = FrameBuffer::from(gop);
            let stdout = GraphicalConsole::new(fb);
            STDOUT = NonNull::new(&stdout as *const _ as *mut _);
       }
    }
    main(handle, st)
}

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}

pub fn stdout<'a>() -> &'static mut GraphicalConsole<'a> {
    unsafe { &mut *STDOUT.unwrap().as_ptr() }
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
