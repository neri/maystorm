#![no_main]
#![no_std]

use megoslib::*;

#[no_mangle]
fn _start() {
    os_print("hello, world\n");
}
