//! Game test
#![no_main]
#![no_std]

// use core::fmt::*;
use megoslib::game::v1::prelude::*;
use megoslib::*;

#[no_mangle]
fn _start() {
    App::new().run();
}

struct App {
    presenter: GamePresenterImpl,
}

impl App {
    const WINDOW_WIDTH: isize = 240;
    const WINDOW_HEIGHT: isize = 160;

    fn new() -> Self {
        let presenter = GameWindow::with_options(
            "GAME TEST",
            Size::new(Self::WINDOW_WIDTH, Self::WINDOW_HEIGHT),
            v1::ScaleMode::Interlace2X,
            60,
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

        screen.set_palette(0x00, PackedColor::from_safe_rgb(0x333333));
        screen.set_palette(0x01, PackedColor::from_safe_rgb(0x999999));
        screen.set_palette(0x02, PackedColor::from_safe_rgb(0xCCCCCC));
        screen.set_palette(0x03, PackedColor::from_safe_rgb(0xFFFFFF));

        screen.set_palette(0x21, PackedColor::WHITE);
        screen.set_palette(0x22, PackedColor::LIGHT_BLUE);
        screen.set_palette(0x23, PackedColor::BLUE);
        screen.set_palette(0x25, PackedColor::WHITE);
        screen.set_palette(0x26, PackedColor::LIGHT_RED);
        screen.set_palette(0x27, PackedColor::RED);

        // basic background patterns
        screen.set_tile_data(0x01, &[
            // 0x81, 0x42, 0x24, 0x18, 0x18, 0x24, 0x42, 0x81, 0, 0, 0, 0, 0, 0, 0, 0,
            0x81, 0xC3, 0xE7, 0xFF, 0xFF, 0xE7, 0xC3, 0x81, 0, 0, 0, 0, 0, 0, 0, 0,
        ]);
        screen.set_tile_data(0x02, &[
            0x00, 0x00, 0x00, 0x1C, 0x1C, 0x1C, 0x00, 0x00, 0x00, 0x7F, 0x7F, 0x63, 0x63, 0x63, 0x7F, 0x7F,
        ]);
        screen.set_tile_data(0x03, &[
            0x00, 0x00, 0x00, 0xF7, 0xC6, 0xE7, 0xC6, 0xC6, 0x00, 0x00, 0x00, 0xF7, 0xC6, 0xE7, 0xC6, 0xC6,
        ]);
        screen.set_tile_data(0x04, &[
            0x00, 0x00, 0x00, 0x8F, 0xDC, 0x8E, 0x07, 0x1E, 0x00, 0x00, 0x00, 0x8F, 0xDC, 0x8E, 0x07, 0x1E,
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

        screen.set_tile_data(0xE0, &[
            0x00, 0x00, 0x30, 0x30, 0x00, 0x00, 0x00, 0x00, 0x3C, 0x7E, 0xCF, 0xCF, 0xFF, 0xFF, 0x7E, 0x3C,
        ]);

    }

    fn run(&mut self) {
        self.initialize();

        let screen = self.screen();

        screen.fill_all(v1::NameTableEntry::from_index(1));

        screen.fill_names(
            Rect::new(
                0,
                0,
                Self::WINDOW_WIDTH / v1::TILE_SIZE,
                Self::WINDOW_HEIGHT / v1::TILE_SIZE,
            ),
            v1::NameTableEntry::from_index(2),
        );
        screen.fill_names(
            Rect::new(
                2,
                2,
                Self::WINDOW_WIDTH / v1::TILE_SIZE - 4,
                Self::WINDOW_HEIGHT / v1::TILE_SIZE - 4,
            ),
            v1::NameTableEntry::from_index(0),
        );

        screen.set_name(
            Self::WINDOW_WIDTH / v1::TILE_SIZE - 2,
            0,
            v1::NameTableEntry::from_index(3),
        );
        screen.set_name(
            Self::WINDOW_WIDTH / v1::TILE_SIZE - 1,
            0,
            v1::NameTableEntry::from_index(4),
        );

        let mut player = Player::new(
            Point::new(Self::WINDOW_WIDTH / 2, Self::WINDOW_HEIGHT / 2),
            1,
            Direction::Neutral,
        );
        let mut missile = Missile::new(Point::new(0, v1::MAX_HEIGHT), Direction::Neutral);

        screen.set_sprite(0, 0x80, v1::OAM_ATTR_W16 | v1::OAM_ATTR_H16 | v1::PALETTE_0);

        screen.set_sprite(1, 0xE0, v1::PALETTE_1);

        screen.draw_string(Point::new(3, 3), 0, b"Hello, world!");

        let mut fps = 0;
        let mut time = os_monotonic();
        loop {
            self.presenter.sync();

            self.presenter.dispatch_buttons(|pad| match pad {
                v1::JoyPad::DpadRight => {
                    player.dir = Direction::Right;
                    player.walk();
                }
                v1::JoyPad::DpadLeft => {
                    player.dir = Direction::Left;
                    player.walk();
                }
                v1::JoyPad::DpadDown => {
                    player.dir = Direction::Down;
                    player.walk();
                }
                v1::JoyPad::DpadUp => {
                    player.dir = Direction::Up;
                    player.walk();
                }
                v1::JoyPad::Fire1 => {
                    if player.dir != Direction::Neutral && !missile.is_alive() {
                        missile.point = player.point + Point::new(4, 4);
                        missile.dir = player.dir;
                    }
                }
                _ => (),
            });
            self.presenter.move_sprite(0, player.point);
            missile.step();
            self.presenter.move_sprite(1, missile.point);

            // self.presenter.screen().control_mut().scroll_x -= 1;
            // self.presenter.screen().control_mut().scroll_y -= 1;
            // self.presenter.set_needs_display();

            fps += 1;
            let now = os_monotonic();
            if (now - time) >= 1000_000 {
                let screen = self.screen();
                let fps0 = (0x30 + fps % 10) as u8;
                let fps1 = (0x30 + (fps / 10) % 10) as u8;
                let mut fps2 = (fps / 100) as u8;
                if fps2 > 0 {
                    fps2 += 0x30;
                }
                screen.set_name(
                    Self::WINDOW_WIDTH / v1::TILE_SIZE - 5,
                    0,
                    v1::NameTableEntry::from_index(fps2),
                );
                screen.set_name(
                    Self::WINDOW_WIDTH / v1::TILE_SIZE - 4,
                    0,
                    v1::NameTableEntry::from_index(fps1),
                );
                screen.set_name(
                    Self::WINDOW_WIDTH / v1::TILE_SIZE - 3,
                    0,
                    v1::NameTableEntry::from_index(fps0),
                );
                self.presenter.invalidate_rect(Rect::new(
                    Self::WINDOW_WIDTH - v1::TILE_SIZE * 5,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Direction {
    Neutral,
    Up,
    Down,
    Left,
    Right,
}

struct Player {
    point: Point,
    speed: isize,
    dir: Direction,
}

impl Player {
    #[inline]
    const fn new(point: Point, speed: isize, dir: Direction) -> Self {
        Self { point, speed, dir }
    }

    fn walk(&mut self) {
        let speed = self.speed * 2;
        match self.dir {
            Direction::Neutral => (),
            Direction::Up => {
                if self.point.y > 16 {
                    self.point.y -= speed;
                }
            }
            Direction::Down => {
                if self.point.y < App::WINDOW_HEIGHT - 32 {
                    self.point.y += speed;
                }
            }
            Direction::Left => {
                if self.point.x > 16 {
                    self.point.x -= speed;
                }
            }
            Direction::Right => {
                if self.point.x < App::WINDOW_WIDTH - 32 {
                    self.point.x += speed;
                }
            }
        }
    }
}

struct Missile {
    point: Point,
    dir: Direction,
}

impl Missile {
    #[inline]
    fn new(point: Point, dir: Direction) -> Self {
        Self { point, dir }
    }

    #[inline]
    fn is_alive(&self) -> bool {
        self.point.y < v1::MAX_HEIGHT
    }

    fn step(&mut self) {
        let speed = 4;
        if self.point.y < v1::MAX_HEIGHT {
            match self.dir {
                Direction::Neutral => (),
                Direction::Up => {
                    self.point.y -= speed;
                }
                Direction::Down => {
                    self.point.y += speed;
                }
                Direction::Left => {
                    self.point.x -= speed;
                }
                Direction::Right => {
                    self.point.x += speed;
                }
            }
            if self.point.x < 8
                || self.point.x >= App::WINDOW_WIDTH - 8
                || self.point.y < 8
                || self.point.y >= App::WINDOW_HEIGHT - 8
            {
                self.point.y = v1::MAX_HEIGHT;
            }
        }
    }
}
