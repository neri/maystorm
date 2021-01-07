// Game of Life sample
#![no_main]
#![no_std]

use myoslib::*;
use myoslib::{bitmap::*, graphics::*, window::Window};

const DRAW_SCALE: isize = 2;
const BITMAP_WIDTH: isize = 64;
const BITMAP_HEIGHT: isize = 64;
const SIZE_BITMAP: usize = (BITMAP_HEIGHT * BITMAP_WIDTH / 8) as usize;

#[no_mangle]
fn _start() {
    os_srand(os_monotonic());

    let window = Window::new(
        "Game of Life",
        Size::new(BITMAP_WIDTH * DRAW_SCALE, BITMAP_HEIGHT * DRAW_SCALE),
    );

    let mut curr_data = [0u8; SIZE_BITMAP];
    let mut next_data = [0u8; SIZE_BITMAP];
    for i in 1..(SIZE_BITMAP - 1) {
        curr_data[i] = os_rand() as u8;
    }

    let mut current =
        OsMutBitmap1::from_slice(&mut curr_data, Size::new(BITMAP_WIDTH, BITMAP_HEIGHT));
    let mut next = OsMutBitmap1::from_slice(&mut next_data, Size::new(BITMAP_WIDTH, BITMAP_HEIGHT));

    loop {
        window.fill_rect(
            Rect::new(0, 0, BITMAP_WIDTH * DRAW_SCALE, BITMAP_HEIGHT * DRAW_SCALE),
            Color::WHITE,
        );
        current.blt(
            &window,
            Point::new(0, 0),
            Color::DARK_GRAY,
            DRAW_SCALE as usize,
        );
        window.flash();

        let w = BITMAP_WIDTH - 1;
        let h = BITMAP_HEIGHT - 1;
        for y in 1..h {
            for x in 1..w {
                let center = Point::new(x, y);
                let mut life = unsafe { current.get_pixel_unchecked(center) };

                let mut count = 0;
                for coords in &[
                    (-1, -1),
                    (0, -1),
                    (1, -1),
                    (-1, 0),
                    (1, 0),
                    (-1, 1),
                    (0, 1),
                    (1, 1),
                ] {
                    let x = coords.0;
                    let y = coords.1;
                    let point = Point::new(center.x + x, center.y + y);
                    if unsafe { current.get_pixel_unchecked(point) } != 0 {
                        count += 1;
                    }
                }

                if life == 0 {
                    if count == 3 {
                        life = 1;
                    }
                } else {
                    if count <= 1 || count >= 4 {
                        life = 0;
                    }
                }
                unsafe {
                    next.set_pixel_unchecked(center, life);
                }
            }
        }

        current.copy_from(&next);

        if window.read_char().is_some() {
            break;
        }
    }
}
