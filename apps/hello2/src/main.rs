// Hello World for myos + wasm
#![no_main]
#![no_std]

use core::fmt::Write;
use myoslib::*;

#[no_mangle]
fn _start() {
    println!("Hello, world!");
}
