#![no_main]
#![no_std]

use megstd::{sys::syscall::*, window::*};

const DRAW_SCALE: isize = 2;
const BITMAP_WIDTH: isize = 64;
const BITMAP_HEIGHT: isize = 64;
const SIZE_BITMAP: usize = (BITMAP_HEIGHT * BITMAP_WIDTH / 8) as usize;

#[no_mangle]
fn _start() {
    os_srand(os_monotonic());

    let window = Window::new(
        "LIFE",
        Size::new(BITMAP_WIDTH * DRAW_SCALE, BITMAP_HEIGHT * DRAW_SCALE),
    );

    let mut curr_data = [0u8; SIZE_BITMAP];
    let mut next_data = [0u8; SIZE_BITMAP];
    for i in 1..(SIZE_BITMAP - 1) {
        curr_data[i] = os_rand() as u8;
    }

    let mut current =
        Bitmap1::from_slice(&mut curr_data, Size::new(BITMAP_WIDTH, BITMAP_HEIGHT), None);
    let mut next =
        Bitmap1::from_slice(&mut next_data, Size::new(BITMAP_WIDTH, BITMAP_HEIGHT), None);

    loop {
        window.draw(|ctx| {
            ctx.fill_rect(
                Rect::new(0, 0, BITMAP_WIDTH * DRAW_SCALE, BITMAP_HEIGHT * DRAW_SCALE),
                WindowColor::WHITE,
            );
            ctx.blt1(
                &current,
                Point::new(0, 0),
                WindowColor::DARK_GRAY,
                DRAW_SCALE as usize,
            );
        });

        let w = BITMAP_WIDTH - 1;
        let h = BITMAP_HEIGHT - 1;
        for y in 1..h {
            for x in 1..w {
                let center = Point::new(x, y);
                let mut life = unsafe { current.get_pixel_unchecked(center) };

                let count = [
                    (-1, -1),
                    (0, -1),
                    (1, -1),
                    (-1, 0),
                    (1, 0),
                    (-1, 1),
                    (0, 1),
                    (1, 1),
                ]
                .iter()
                .fold(0, |acc, coords| {
                    if unsafe {
                        current.get_pixel_unchecked(Point::new(
                            center.x + coords.0,
                            center.y + coords.1,
                        ))
                    } != 0
                    {
                        acc + 1
                    } else {
                        acc
                    }
                });

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
