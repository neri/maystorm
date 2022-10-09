#![no_main]
#![no_std]

use megstd::game::v1::prelude::*;

#[no_mangle]
fn _start() {
    let presenter = GameWindow::new("Hello world", Size::new(128, 64));

    let chars = b"Hello, world!";
    for (index, char) in chars.iter().enumerate() {
        presenter.screen().set_sprite(index, *char, 0);
    }

    let mut phase = 0;
    loop {
        presenter.sync();

        for index in 0..chars.len() {
            let position = ((phase - index as isize) & 31) - 15;
            let value = position * position / 8;
            presenter.move_sprite(
                index as v1::SpriteIndex,
                Point::new(8 + index as isize * 9, 16 + value),
            );
        }

        phase += 1;
    }
}
