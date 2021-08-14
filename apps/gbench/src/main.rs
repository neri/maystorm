//! Game Bench
#![no_main]
#![no_std]

use core::cell::UnsafeCell;
// use core::fmt::*;
use megoslib::game::v1::GamePresenterImpl;
use megoslib::{window::*, *};
use megstd::drawing::*;
use megstd::game::v1;
use megstd::game::v1::GamePresenter;

#[no_mangle]
fn _start() {
    let mut app = App::new();
    app.initialize();
    app.run();
}
pub struct App<'a> {
    presenter: GamePresenterImpl<'a>,
}

impl App<'_> {
    const WINDOW_WIDTH: isize = 256;
    const WINDOW_HEIGHT: isize = 240;
}

static mut SCREEN: UnsafeCell<v1::Screen> = UnsafeCell::new(v1::Screen::new());

impl<'a> App<'a> {
    fn new() -> Self {
        let window = WindowBuilder::new()
            .size(Size::new(Self::WINDOW_WIDTH, Self::WINDOW_HEIGHT))
            // .bitmap_argb32()
            .build("GAME BENCH");
        let presenter =
            window.init_game_presenter(unsafe { &SCREEN }, v1::ScaleMode::DotByDot, 240);
        Self { presenter }
    }
}

impl App<'_> {
    #[inline]
    fn screen(&mut self) -> &mut v1::Screen {
        self.presenter.screen()
    }

    #[rustfmt::skip]
    fn initialize(&mut self) {
        let screen = self.screen();

        screen.set_palette(0x05, PackedColor::WHITE);
        screen.set_palette(0x06, PackedColor::LIGHT_BLUE);
        screen.set_palette(0x07, PackedColor::BLUE);
        screen.set_palette(0x09, PackedColor::WHITE);
        screen.set_palette(0x0A, PackedColor::LIGHT_RED);
        screen.set_palette(0x0B, PackedColor::RED);
        screen.set_palette(0x0D, PackedColor::WHITE);
        screen.set_palette(0x0E, PackedColor::YELLOW);
        screen.set_palette(0x0F, PackedColor::BROWN);
        screen.set_palette(0x11, PackedColor::WHITE);
        screen.set_palette(0x12, PackedColor::LIGHT_GREEN);
        screen.set_palette(0x13, PackedColor::GREEN);

        // basic background patterns
        screen.set_char_data(0x01, &[
            0x00, 0x55, 0x00, 0xAA, 0x00, 0x55, 0x00, 0xAA, 0, 0, 0, 0, 0, 0, 0, 0,
        ]);
        screen.set_char_data(0x02, &[
            0x00, 0x00, 0x00, 0x1C, 0x1C, 0x1C, 0x00, 0x00, 0x00, 0x7F, 0x7F, 0x63, 0x63, 0x63, 0x7F, 0x7F,
        ]);
        screen.set_char_data(0x03, &[
            0x00, 0x00, 0x00, 0xF7, 0xC6, 0xE7, 0xC6, 0xC6, 0x00, 0x00, 0x00, 0xF7, 0xC6, 0xE7, 0xC6, 0xC6,
        ]);
        screen.set_char_data(0x04, &[
            0x00, 0x00, 0x00, 0x8F, 0xDC, 0x8E, 0x07, 0x1E, 0x00, 0x00, 0x00, 0x8F, 0xDC, 0x8E, 0x07, 0x1E,
        ]);

        // number fonts
        screen.set_char_data(0x30, &[
            0x3c, 0x66, 0x66, 0x6e, 0x76, 0x66, 0x66, 0x3c, 0x3c, 0x66, 0x66, 0x6e, 0x76, 0x66, 0x66, 0x3c,
        ]);
        screen.set_char_data(0x31, &[
            0x18, 0x38, 0x78, 0x18, 0x18, 0x18, 0x18, 0x7e, 0x18, 0x38, 0x78, 0x18, 0x18, 0x18, 0x18, 0x7e,
        ]);
        screen.set_char_data(0x32, &[
            0x3c, 0x66, 0x66, 0x06, 0x1c, 0x30, 0x62, 0x7e, 0x3c, 0x66, 0x66, 0x06, 0x1c, 0x30, 0x62, 0x7e,
        ]);
        screen.set_char_data(0x33, &[
            0x7e, 0x0c, 0x18, 0x3c, 0x06, 0x06, 0x66, 0x3c, 0x7e, 0x0c, 0x18, 0x3c, 0x06, 0x06, 0x66, 0x3c,
        ]);
        screen.set_char_data(0x34, &[
            0x0c, 0x1c, 0x3c, 0x3c, 0x6c, 0x6c, 0x7e, 0x0c, 0x0c, 0x1c, 0x3c, 0x3c, 0x6c, 0x6c, 0x7e, 0x0c,
        ]);
        screen.set_char_data(0x35, &[
            0x7e, 0x60, 0x60, 0x7c, 0x06, 0x06, 0x66, 0x3c, 0x7e, 0x60, 0x60, 0x7c, 0x06, 0x06, 0x66, 0x3c,
        ]);
        screen.set_char_data(0x36, &[
            0x3c, 0x62, 0x60, 0x7c, 0x66, 0x66, 0x66, 0x3c, 0x3c, 0x62, 0x60, 0x7c, 0x66, 0x66, 0x66, 0x3c,
        ]);
        screen.set_char_data(0x37, &[
            0x7e, 0x66, 0x06, 0x0c, 0x18, 0x18, 0x18, 0x18, 0x7e, 0x66, 0x06, 0x0c, 0x18, 0x18, 0x18, 0x18,
        ]);
        screen.set_char_data(0x38, &[
            0x3c, 0x66, 0x66, 0x3c, 0x66, 0x66, 0x66, 0x3c, 0x3c, 0x66, 0x66, 0x3c, 0x66, 0x66, 0x66, 0x3c,
        ]);
        screen.set_char_data(0x39, &[
            0x3c, 0x66, 0x66, 0x66, 0x3e, 0x06, 0x0c, 0x38, 0x3c, 0x66, 0x66, 0x66, 0x3e, 0x06, 0x0c, 0x38,
        ]);

        // sprites
        screen.set_char_data(0x80, &[
            0x03, 0x0C, 0x10, 0x20, 0x4C, 0x4C, 0x80, 0x80, 0x03, 0x0F, 0x1F, 0x3F, 0x73, 0x73, 0xFF, 0xFF,
        ]);
        screen.set_char_data(0x81, &[
            0xC0, 0x30, 0x08, 0x04, 0x02, 0x02, 0x01, 0x01, 0xC0, 0xF0, 0xF8, 0xFC, 0xFE, 0xFE, 0xFF, 0xFF,
        ]);
        screen.set_char_data(0x90, &[
            0x80, 0x80, 0x40, 0x40, 0x20, 0x10, 0x0C, 0x03, 0xFF, 0xFF, 0x7F, 0x7F, 0x3F, 0x1F, 0x0F, 0x03,
        ]);
        screen.set_char_data(0x91, &[
            0x01, 0x01, 0x02, 0x02, 0x04, 0x08, 0x30, 0xC0, 0xFF, 0xFF, 0xFE, 0xFE, 0xFC, 0xF8, 0xF0, 0xC0,
        ]);
    }

    fn run(&mut self) {
        let screen = self.screen();

        screen.fill_names(
            Rect::new(
                0,
                0,
                Self::WINDOW_WIDTH / v1::CHAR_SIZE,
                Self::WINDOW_HEIGHT / v1::CHAR_SIZE,
            ),
            2,
        );
        screen.fill_names(
            Rect::new(
                2,
                2,
                Self::WINDOW_WIDTH / v1::CHAR_SIZE - 4,
                Self::WINDOW_HEIGHT / v1::CHAR_SIZE - 4,
            ),
            1,
        );

        unsafe {
            screen.set_name(Self::WINDOW_WIDTH / v1::CHAR_SIZE - 2, 0, 3);
            screen.set_name(Self::WINDOW_WIDTH / v1::CHAR_SIZE - 1, 0, 4);
        }

        let mut marbles = [Marble::empty(); 64];
        for (index, item) in marbles.iter_mut().enumerate() {
            *item = Marble::new(
                (os_rand() % (Self::WINDOW_WIDTH as u32 - 48)) as isize + 16,
                (os_rand() % (Self::WINDOW_HEIGHT as u32 - 48)) as isize + 16,
                if (os_rand() & 1) == 0 { 1 } else { -1 },
                if (os_rand() & 1) == 0 { 1 } else { -1 },
                1 + (os_rand() & 1) as isize,
            );
            *screen.get_sprite_mut(index) = v1::Sprite::new(
                item.origin(),
                0x80,
                v1::OAM_ATTR_W16 | v1::OAM_ATTR_H16 | (1 + (3 & index as u8)),
            );
        }

        self.presenter.set_needs_display();

        let mut fps = 0;
        let mut time = os_monotonic();
        loop {
            self.presenter.sync();

            for (index, item) in marbles.iter_mut().enumerate() {
                item.step();
                self.presenter.move_sprite(index as u8, item.origin());
            }

            fps += 1;
            let now = os_monotonic();
            if (now - time) >= 1000_000 {
                let screen = self.screen();
                let fps0 = 0x30 + fps % 10;
                let fps1 = 0x30 + (fps / 10) % 10;
                let mut fps2 = fps / 100;
                if fps2 > 0 {
                    fps2 += 0x30;
                }
                unsafe {
                    screen.set_name(Self::WINDOW_WIDTH / v1::CHAR_SIZE - 5, 0, fps2);
                    screen.set_name(Self::WINDOW_WIDTH / v1::CHAR_SIZE - 4, 0, fps1);
                    screen.set_name(Self::WINDOW_WIDTH / v1::CHAR_SIZE - 3, 0, fps0);
                }
                self.presenter.invalidate_rect(Rect::new(
                    Self::WINDOW_WIDTH - v1::CHAR_SIZE * 5,
                    0,
                    24,
                    8,
                ));
                fps = 0;
                time = now;
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Marble {
    x: isize,
    y: isize,
    dir_x: isize,
    dir_y: isize,
    speed: isize,
}

impl Marble {
    #[inline]
    fn new(x: isize, y: isize, dir_x: isize, dir_y: isize, speed: isize) -> Self {
        Self {
            x,
            y,
            dir_x,
            dir_y,
            speed,
        }
    }

    #[inline]
    const fn origin(&self) -> Point {
        Point::new(self.x as isize, self.y as isize)
    }

    #[inline]
    const fn empty() -> Self {
        Self {
            x: 0,
            y: 0,
            dir_x: 0,
            dir_y: 0,
            speed: 0,
        }
    }

    fn step(&mut self) {
        // let mut turned = false;
        if self.dir_x > 0 {
            if self.x + self.speed < App::WINDOW_WIDTH - 32 {
                self.x += self.speed;
            } else {
                self.dir_x = -1;
                // turned = true;
            }
        } else if self.dir_x < 0 {
            if self.x - self.speed > 16 {
                self.x -= self.speed;
            } else {
                self.dir_x = 1;
                // turned = true;
            }
        }
        if self.dir_y > 0 {
            if self.y + self.speed < App::WINDOW_HEIGHT - 32 {
                self.y += self.speed;
            } else {
                self.dir_y = -1;
                // turned = true;
            }
        } else if self.dir_y < 0 {
            if self.y - self.speed > 16 {
                self.y -= self.speed;
            } else {
                self.dir_y = 1;
                // turned = true;
            }
        }
    }
}
