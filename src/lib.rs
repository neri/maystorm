// My UEFI-Rust Lib
#![feature(panic_info_message)]
#![feature(abi_efiapi)]
#![feature(lang_items)]
#![feature(alloc_error_handler)]
#![no_std]

use core::fmt::Write;
use core::panic::PanicInfo;
use core::ptr::NonNull;
use uefi::prelude::*;

extern crate alloc;
// use alloc::boxed::Box;

use crate::console::GraphicalConsole;
use crate::graphics::FrameBuffer;

pub mod console;
pub mod font;
pub mod graphics;
pub mod num;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("Panic: {}", info);
    loop {}
}

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}

static mut STDOUT: Option<NonNull<GraphicalConsole>> = None;

pub fn stdout<'a>() -> &'static mut GraphicalConsole<'a> {
    unsafe { &mut *STDOUT.unwrap().as_ptr() }
}

pub fn startup<F>(handle: Handle, st: SystemTable<Boot>, custom_main: F) -> Status
where
    F: Fn(Handle, SystemTable<Boot>) -> Status,
{
    unsafe {
        uefi::alloc::init(&st.boot_services());
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
    } else {
        write!(st.stdout(), "Error: Not supported GOP\n").unwrap();
        return Status::UNSUPPORTED;
    }
    custom_main(handle, st)
}

#[macro_export]
macro_rules! uefi_pg_entry {
    ($path:path) => {
        #[entry]
        fn efi_main(handle: Handle, st: SystemTable<Boot>) -> Status {
            let f: fn(Handle, SystemTable<Boot>) -> Status = $path;
            startup(handle, st, f)
        }
    };
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
