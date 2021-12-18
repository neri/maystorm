// Window Hello World for megos + wasm
#![no_main]
#![no_std]

use megoslib::{window::*, *};
use megstd::drawing::*;

#[no_mangle]
fn _start() {
    let window = Window::new("Hello", Size::new(200, 50));
    window.draw(|ctx| ctx.draw_string("Hello, World!", Point::new(10, 10), WindowColor::BLACK));
    let _ = window.wait_char();
}
