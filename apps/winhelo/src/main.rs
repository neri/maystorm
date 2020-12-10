// Window Hello World for myos + wasm
#![no_main]
#![no_std]

use myoslib::graphics::Size;
use myoslib::window::Window;
use myoslib::*;

#[no_mangle]
fn _start() {
    let _window = Window::new("Hello Window", Size::new(200, 100));
}
