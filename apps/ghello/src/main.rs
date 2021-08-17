//! Hello world for Game API
#![no_main]
#![no_std]

use megoslib::game::v1::prelude::*;

#[no_mangle]
fn _start() {
    let presenter = GameWindow::new("hello", Size::new(128, 64));
    presenter
        .screen()
        .draw_string(Point::new(0, 3), b"Hello, world!");
    loop {
        presenter.sync();

        presenter.screen().control_mut().scroll_x -= 1;
        presenter.set_needs_display();
    }
}
