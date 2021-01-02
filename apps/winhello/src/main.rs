// Window Hello World for myos + wasm
#![no_main]
#![no_std]

use myoslib::graphics::*;
use myoslib::window::Window;

#[no_mangle]
fn _start() {
    let window = Window::new("Hello", Size::new(240, 50));
    window.draw_text("Hello, World!", Point::new(10, 10), Color::BLACK);
    let _ = window.wait_char();
}
