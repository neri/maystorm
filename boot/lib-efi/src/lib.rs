#![no_std]
#![feature(core_intrinsics)]
#![feature(asm)]

// use crate::debug::console::DebugConsole;
use core::fmt::Write;
use core::panic::PanicInfo;
use uefi::prelude::*;
use uefi::proto::media::file::*;
use uefi::proto::media::fs::*;
use uefi::table::boot::MemoryType;

pub mod debug;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
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

    let path_len = path.len();
    let path_pool = match bs.allocate_pool(MemoryType::LOADER_DATA, path_len) {
        Ok(val) => val.unwrap(),
        Err(err) => return Err(err.status()),
    };
    for (index, c) in path.chars().enumerate() {
        unsafe {
            let c = match c {
                '/' => '\\',
                _ => c,
            };
            path_pool.add(index).write(c as u8);
        }
    }
    let path =
        unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(path_pool, path_len)) };
    let handle = match root
        .handle()
        .open(path, FileMode::Read, FileAttribute::empty())
    {
        Ok(handle) => handle.unwrap(),
        Err(err) => {
            bs.free_pool(path_pool).unwrap().unwrap();
            return Err(err.status());
        }
    };
    bs.free_pool(path_pool).unwrap().unwrap();

    let mut file = match handle.into_type().unwrap().unwrap() {
        FileType::Regular(file) => file,
        FileType::Dir(_) => return Err(Status::ACCESS_DENIED),
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
        bs.free_pool(pool).unwrap().unwrap();
        return Err(err.status());
    }

    Ok(buffer)
}
