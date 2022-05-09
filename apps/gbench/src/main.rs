#![no_main]
#![no_std]

use megstd::{game::v1::prelude::*, sys::syscall::*};

#[no_mangle]
fn _start() {
    App::new().run();
}

pub struct App {
    presenter: GamePresenterImpl,
}

impl App {
    const WINDOW_WIDTH: isize = 256;
    const WINDOW_HEIGHT: isize = 240;

    #[inline]
    fn new() -> Self {
        let presenter = GameWindow::with_options(
            "GAME BENCH",
            Size::new(Self::WINDOW_WIDTH, Self::WINDOW_HEIGHT),
            ScaleMode::DotByDot,
            500,
        );
        Self { presenter }
    }

    #[inline]
    fn screen(&mut self) -> &mut v1::Screen {
        self.presenter.screen()
    }

    #[rustfmt::skip]
    fn initialize(&mut self) {

        let screen = self.screen();

        screen.set_palette(0x04, PackedColor::from_safe_rgb(0xCCCCFF));

        screen.set_palette(0x25, PackedColor::WHITE);
        screen.set_palette(0x26, PackedColor::LIGHT_BLUE);
        screen.set_palette(0x27, PackedColor::BLUE);
        screen.set_palette(0x29, PackedColor::WHITE);
        screen.set_palette(0x2A, PackedColor::LIGHT_RED);
        screen.set_palette(0x2B, PackedColor::RED);
        screen.set_palette(0x2D, PackedColor::WHITE);
        screen.set_palette(0x2E, PackedColor::YELLOW);
        screen.set_palette(0x2F, PackedColor::BROWN);
        screen.set_palette(0x31, PackedColor::WHITE);
        screen.set_palette(0x32, PackedColor::LIGHT_GREEN);
        screen.set_palette(0x33, PackedColor::GREEN);


        // basic background patterns
        screen.set_tile_data(0x01, &[
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ]);

        // sprites
        screen.set_tile_data(0x80, &[
            0x03, 0x0C, 0x10, 0x20, 0x4C, 0x4C, 0x80, 0x80, 0x03, 0x0F, 0x1F, 0x3F, 0x73, 0x73, 0xFF, 0xFF,
        ]);
        screen.set_tile_data(0x81, &[
            0xC0, 0x30, 0x08, 0x04, 0x02, 0x02, 0x01, 0x01, 0xC0, 0xF0, 0xF8, 0xFC, 0xFE, 0xFE, 0xFF, 0xFF,
        ]);
        screen.set_tile_data(0x90, &[
            0x80, 0x80, 0x40, 0x40, 0x20, 0x10, 0x0C, 0x03, 0xFF, 0xFF, 0x7F, 0x7F, 0x3F, 0x1F, 0x0F, 0x03,
        ]);
        screen.set_tile_data(0x91, &[
            0x01, 0x01, 0x02, 0x02, 0x04, 0x08, 0x30, 0xC0, 0xFF, 0xFF, 0xFE, 0xFE, 0xFC, 0xF8, 0xF0, 0xC0,
        ]);
    }

    fn run(&mut self) {
        self.initialize();
        let screen = self.screen();

        for y in 0..v1::MAX_VHEIGHT as isize / v1::TILE_SIZE {
            for x in 0..v1::MAX_VWIDTH as isize / v1::TILE_SIZE {
                if ((x ^ y) & 1) != 0 {
                    screen.set_name(x, y, v1::NameTableEntry::new(0, v1::PALETTE_1));
                }
            }
        }

        const MAX_SPRITES: usize = 64;
        screen.control_mut().sprite_max = MAX_SPRITES.wrapping_sub(1) as u8;
        let mut marbles = [Marble::empty(); MAX_SPRITES];
        for (index, item) in marbles.iter_mut().enumerate() {
            *item = Marble::new(
                (os_rand() % (Self::WINDOW_WIDTH as u32 - 48)) as isize + 16,
                (os_rand() % (Self::WINDOW_HEIGHT as u32 - 48)) as isize + 16,
                if (os_rand() & 1) == 0 { 1 } else { -1 },
                if (os_rand() & 1) == 0 { 1 } else { -1 },
                1 + (os_rand() & 1) as isize,
            );
            screen.set_sprite(
                index,
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

            // self.presenter.screen().control_mut().scroll_y += 1;
            // self.presenter.set_needs_display();

            fps += 1;
            let now = os_monotonic();
            if (now - time) >= 1000_000 {
                let fps2 = (0x30 + fps % 10) as u8;
                let fps1 = (0x30 + (fps / 10) % 10) as u8;
                let mut fps0 = (fps / 100) as u8;
                if fps0 > 0 {
                    fps0 += 0x30;
                }
                let str = [fps0, fps1, fps2, b'F', b'P', b'S'];
                self.screen().draw_string(Point::new(0, 0), 0, &str);
                self.presenter.set_needs_display();
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
            if self.x + self.speed < App::WINDOW_WIDTH - 16 {
                self.x += self.speed;
            } else {
                self.dir_x = -1;
                // turned = true;
            }
        } else if self.dir_x < 0 {
            if self.x - self.speed > 0 {
                self.x -= self.speed;
            } else {
                self.dir_x = 1;
                // turned = true;
            }
        }
        if self.dir_y > 0 {
            if self.y + self.speed < App::WINDOW_HEIGHT - 16 {
                self.y += self.speed;
            } else {
                self.dir_y = -1;
                // turned = true;
            }
        } else if self.dir_y < 0 {
            if self.y - self.speed > 0 {
                self.y -= self.speed;
            } else {
                self.dir_y = 1;
                // turned = true;
            }
        }
    }
}
