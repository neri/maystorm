#![no_main]
#![no_std]

use megstd::sys::syscall::*;

#[no_mangle]
fn _start() {
    os_print("hello, world\n");
}
