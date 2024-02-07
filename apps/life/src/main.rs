#![no_main]
#![no_std]

use megstd::drawing::Monochrome;
use megstd::sys::syscall::*;
use megstd::window::*;

const BG_COLOR: WindowColor = WindowColor::BLACK;
const FG_COLOR: WindowColor = WindowColor::YELLOW;
const DRAW_SCALE: isize = 2;
const BITMAP_WIDTH: isize = 64;
const BITMAP_HEIGHT: isize = 64;
const SIZE_BITMAP: usize = (BITMAP_HEIGHT * BITMAP_WIDTH / 8) as usize;

#[no_mangle]
fn _start() {
    os_srand(os_monotonic());

    let window = WindowBuilder::new()
        .size(Size::new(
            BITMAP_WIDTH * DRAW_SCALE,
            BITMAP_HEIGHT * DRAW_SCALE,
        ))
        .bg_color(BG_COLOR)
        .build("LIFE");
    window.set_max_fps(10);

    let mut curr_data = [0u8; SIZE_BITMAP];
    let mut next_data = [0u8; SIZE_BITMAP];
    for i in 1..(SIZE_BITMAP - 1) {
        curr_data[i] = os_rand() as u8;
    }

    let mut current =
        BitmapRefMut1::from_bytes(&mut curr_data, Size::new(BITMAP_WIDTH, BITMAP_HEIGHT));
    let mut next =
        BitmapRefMut1::from_bytes(&mut next_data, Size::new(BITMAP_WIDTH, BITMAP_HEIGHT));

    loop {
        window.draw(|ctx| {
            ctx.fill_rect(
                Rect::new(0, 0, BITMAP_WIDTH * DRAW_SCALE, BITMAP_HEIGHT * DRAW_SCALE),
                BG_COLOR,
            );
            ctx.blt1(&current, Point::new(0, 0), FG_COLOR, DRAW_SCALE as usize);
        });

        for y in 1..(BITMAP_HEIGHT - 1) {
            for x in 1..(BITMAP_WIDTH - 1) {
                let center = Point::new(x, y);
                let life = unsafe { current.get_pixel_unchecked(center) };

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
                    acc + usize::from(unsafe {
                        current.get_pixel_unchecked(Point::new(
                            center.x + coords.0,
                            center.y + coords.1,
                        ))
                    })
                });

                let next_life = if life.into_bool() {
                    if count <= 1 || count >= 4 {
                        Monochrome::Zero
                    } else {
                        life
                    }
                } else {
                    if count == 3 {
                        Monochrome::One
                    } else {
                        life
                    }
                };
                unsafe {
                    next.set_pixel_unchecked(center, next_life);
                }
            }
        }

        current.copy_from(&next);

        if window.read_char().is_some() {
            break;
        }
    }
}
