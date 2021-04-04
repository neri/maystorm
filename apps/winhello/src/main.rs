// Window Hello World for myos + wasm
#![no_main]
#![no_std]

use megstd::drawing::*;
use myoslib::window::*;
use myoslib::*;

#[no_mangle]
fn _start() {
    let window = Window::new("Hello", Size::new(240, 50));
    window.draw_string("Hello, World!", Point::new(10, 10), WindowColor::BLACK);
    let _ = window.wait_char();
}
