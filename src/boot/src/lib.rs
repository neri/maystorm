#![no_std]
#![feature(core_intrinsics)]
#![feature(asm)]

use core::fmt::Write;
use core::panic::PanicInfo;
use uefi::prelude::*;
use uefi::proto::media::file::*;
use uefi::proto::media::fs::*;
use uefi::table::boot::MemoryType;

pub mod blob;
pub mod invocation;
pub mod loader;
pub mod page;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

pub struct Uart {}

impl Uart {
    pub const fn new() -> Self {
        Self {}
    }
}

impl Write for Uart {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        unsafe {
            for c in s.chars() {
                asm!("out dx, al", in("edx") 0x3F8, in("al") c as u8);
            }
        }
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        write!(Uart::new(), $($arg)*).unwrap()
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

pub fn get_file(
    handle: Handle,
    bs: &BootServices,
    path: &str,
) -> Result<&'static mut [u8], Status> {
    let li = unsafe {
        match bs.handle_protocol::<uefi::proto::loaded_image::LoadedImage>(handle) {
            Ok(val) => val.unwrap().get().as_ref().unwrap(),
            Err(_) => return Err(Status::LOAD_ERROR),
        }
    };
    let fs = unsafe {
        match bs.handle_protocol::<SimpleFileSystem>(li.device()) {
            Ok(val) => val.unwrap().get().as_mut().unwrap(),
            Err(_) => return Err(Status::LOAD_ERROR),
        }
    };
    let mut root = match fs.open_volume() {
        Ok(val) => val.unwrap(),
        Err(err) => return Err(err.status()),
    };

    let mut file = unsafe {
        RegularFile::new(
            match root
                .handle()
                .open(path, FileMode::Read, FileAttribute::empty())
            {
                Ok(handle) => handle.unwrap(),
                Err(err) => return Err(err.status()),
            },
        )
    };

    match file.set_position(u64::MAX) {
        Ok(_) => (),
        Err(err) => return Err(err.status()),
    };
    let file_size = match file.get_position() {
        Ok(val) => val.unwrap(),
        Err(err) => return Err(err.status()),
    } as usize;
    match file.set_position(0) {
        Ok(_) => (),
        Err(err) => return Err(err.status()),
    };

    let pool = match bs.allocate_pool(MemoryType::LOADER_DATA, file_size) {
        Ok(val) => val.unwrap(),
        Err(err) => return Err(err.status()),
    };
    let buffer = unsafe { core::slice::from_raw_parts_mut(pool, file_size) };

    if let Err(err) = file.read(buffer) {
        return Err(err.status());
    }

    Ok(buffer)
}
