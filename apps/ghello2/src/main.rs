//! Hello world for Game API
#![no_main]
#![no_std]

use megoslib::game::v1::prelude::*;

#[no_mangle]
fn _start() {
    let presenter = GameWindow::new("hello2", Size::new(128, 64));

    let chars = b"Hello, world!";
    for (index, char) in chars.iter().enumerate() {
        presenter.screen().set_sprite(index, *char, 0);
    }

    let mut phase = 0;
    loop {
        presenter.sync();

        for index in 0..chars.len() {
            let position = ((phase - index as isize) & 31) - 15;
            let value = position * position / 8 - 16;
            presenter.move_sprite(
                index as v1::SpriteIndex,
                Point::new(index as isize * 10, 28 + value),
            );
        }

        phase += 1;
    }
}
