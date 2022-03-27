#![no_std]
#![feature(core_intrinsics)]
#![feature(alloc_error_handler)]

use alloc::vec::Vec;
use core::{alloc::Layout, fmt::Write, panic::PanicInfo};
use uefi::CStr16;
use uefi::{
    prelude::*,
    proto::{
        loaded_image::LoadedImage,
        media::{file::*, fs::*},
    },
    table::boot::{MemoryType, OpenProtocolParams},
};
extern crate alloc;

pub mod debug;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        write!(debug::Console::shared(), $($arg)*).unwrap()
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
    let li: &LoadedImage = match bs.open_protocol(
        OpenProtocolParams {
            handle,
            agent: handle,
            controller: None,
        },
        uefi::table::boot::OpenProtocolAttributes::GetProtocol,
    ) {
        Ok(val) => unsafe { &*val.interface.get() },
        Err(_) => return Err(Status::LOAD_ERROR),
    };
    let fs: &mut SimpleFileSystem = match bs.open_protocol(
        OpenProtocolParams {
            handle: li.device(),
            agent: handle,
            controller: None,
        },
        uefi::table::boot::OpenProtocolAttributes::GetProtocol,
    ) {
        Ok(val) => unsafe { &mut *val.interface.get() },
        Err(_) => return Err(Status::LOAD_ERROR),
    };
    let mut root = match fs.open_volume() {
        Ok(val) => val,
        Err(err) => return Err(err.status()),
    };

    let mut path = path
        .chars()
        .map(|c| match c {
            '/' => '\\',
            _ => c,
        })
        .map(|c| c as u16)
        .collect::<Vec<u16>>();
    path.push(0);
    let path = CStr16::from_u16_with_nul(&path).unwrap();

    let handle = match root
        .handle()
        .open(path, FileMode::Read, FileAttribute::empty())
    {
        Ok(handle) => handle,
        Err(err) => {
            return Err(err.status());
        }
    };

    let mut file = match handle.into_type().unwrap() {
        FileType::Regular(file) => file,
        FileType::Dir(_) => return Err(Status::UNSUPPORTED),
    };

    match file.set_position(RegularFile::END_OF_FILE) {
        Ok(_) => (),
        Err(err) => return Err(err.status()),
    };
    let file_size = match file.get_position() {
        Ok(val) => val,
        Err(err) => return Err(err.status()),
    } as usize;
    match file.set_position(0) {
        Ok(_) => (),
        Err(err) => return Err(err.status()),
    };

    let pool = match bs.allocate_pool(MemoryType::LOADER_DATA, file_size) {
        Ok(val) => val,
        Err(err) => return Err(err.status()),
    };
    let buffer = unsafe { core::slice::from_raw_parts_mut(pool, file_size) };

    if let Err(err) = file.read(buffer) {
        bs.free_pool(pool).unwrap();
        return Err(err.status());
    }

    Ok(buffer)
}
