// My UEFI-Rust Lib
#![feature(panic_info_message)]
#![feature(abi_efiapi)]
#![feature(lang_items)]
#![feature(alloc_error_handler)]
#![feature(llvm_asm)]
#![no_std]

use core::ffi::c_void;
use core::fmt::Write;
use core::panic::PanicInfo;
use core::ptr::NonNull;
use myos::io::console::GraphicalConsole;
use myos::io::graphics::FrameBuffer;
use myos::*;
use uefi::prelude::*;

pub mod myos;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    stdout().set_cursor_enabled(false);
    stdout().set_attribute(0x17);
    println!("Panic: {}", info);
    loop {
        unsafe {
            arch::cpu::Cpu::disable();
            arch::cpu::Cpu::halt();
        }
    }
}

// #[alloc_error_handler]
// fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
//     panic!("allocation error: {:?}", layout)
// }

// #[lang = "eh_personality"]
// extern "C" fn eh_personality() {}

static mut STDOUT: Option<NonNull<GraphicalConsole>> = None;

pub fn stdout<'a>() -> &'static mut GraphicalConsole<'a> {
    unsafe { &mut *STDOUT.unwrap().as_ptr() }
}

pub fn startup<F>(handle: Handle, st: SystemTable<Boot>, custom_main: F) -> Status
where
    F: Fn(Handle, SystemTable<Boot>) -> Status,
{
    let bs = st.boot_services();
    if let Ok(gop) = bs.locate_protocol::<uefi::proto::console::gop::GraphicsOutput>() {
        let gop = gop.unwrap();
        let gop = unsafe { &mut *gop.get() };
        let fb = FrameBuffer::from(gop);
        let stdout = GraphicalConsole::new(fb);
        unsafe {
            STDOUT = NonNull::new(&stdout as *const _ as *mut _);
        }
    } else {
        write!(st.stdout(), "Error: GOP Not Found\n").unwrap();
        return Status::UNSUPPORTED;
    }
    custom_main(handle, st)
}

pub fn exit_boot_services<'a>(
    st: SystemTable<Boot>,
    image: Handle,
) -> (
    SystemTable<uefi::table::Runtime>,
    uefi::table::boot::MemoryMapIter<'a>,
) {
    // because some UEFI implementations require an additional buffer during exit_boot_services
    let buf_size = st.boot_services().memory_map_size() * 2;
    let buf_ptr = st
        .boot_services()
        .allocate_pool(uefi::table::boot::MemoryType::LOADER_DATA, buf_size)
        .unwrap()
        .unwrap();
    let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr, buf_size) };
    st.exit_boot_services(image, buf).unwrap().unwrap()
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

pub trait MyUefiLib {
    fn find_config_table(&self, _: uefi::Guid) -> Option<*const c_void>;
}

impl MyUefiLib for SystemTable<uefi::table::Boot> {
    fn find_config_table(&self, expected: uefi::Guid) -> Option<*const c_void> {
        for entry in self.config_table() {
            if entry.guid == expected {
                return Some(entry.address);
            }
        }
        None
    }
}

impl MyUefiLib for SystemTable<uefi::table::Runtime> {
    fn find_config_table(&self, expected: uefi::Guid) -> Option<*const c_void> {
        for entry in self.config_table() {
            if entry.guid == expected {
                return Some(entry.address);
            }
        }
        None
    }
}
