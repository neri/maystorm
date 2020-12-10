// Hello World for myos + wasm
#![no_main]
#![no_std]

use myoslib::*;

#[no_mangle]
fn _start() {
    os_print("Hello, world!\n");
}
