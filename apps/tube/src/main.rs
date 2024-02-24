/*
Ported
Original: UNKNOWN

*/

#![no_main]
#![no_std]

extern crate libm;
use core::cell::UnsafeCell;
use core::f64::consts::PI;
use libm::{atan2, cos, sin, sqrt};
use megstd::sys::syscall::*;
use megstd::window::*;

#[no_mangle]
fn _start() {
    App::new().run();
}

const BITMAP_WIDTH: u32 = 240;
const BITMAP_HEIGHT: u32 = 240;
const BITMAP_SIZE: usize = BITMAP_WIDTH as usize * BITMAP_HEIGHT as usize;
static mut DATA: UnsafeCell<[u32; BITMAP_SIZE]> = UnsafeCell::new([0; BITMAP_SIZE]);
static mut SCREEN: UnsafeCell<[i8; BITMAP_SIZE]> = UnsafeCell::new([0; BITMAP_SIZE]);
static mut TEXTURE: UnsafeCell<[u8; 65536]> = UnsafeCell::new([0; 65536]);

struct App<'a> {
    window: Window,
    bitmap: BitmapRefMut32<'a>,
    palettes: [TrueColor; 256],
    screen: &'a mut [i8; BITMAP_SIZE],
    step: isize,
    angle: f64,
}

impl<'a> App<'a> {
    #[inline]
    fn new() -> Self {
        os_srand(os_monotonic());
        let window = WindowBuilder::new()
            .size(Size::new(BITMAP_WIDTH, BITMAP_HEIGHT))
            .opaque()
            .bitmap_argb32()
            .build("tube");
        let bitmap = BitmapRefMut32::from_bytes(
            unsafe { DATA.get_mut() },
            Size::new(BITMAP_WIDTH, BITMAP_HEIGHT),
        );
        let palettes = Self::make_palettes();
        Self::make_texture();
        let screen = unsafe { SCREEN.get_mut() };
        Self {
            window,
            bitmap,
            palettes,
            screen,
            step: 0,
            angle: 0.0,
        }
    }
}

impl App<'_> {
    fn run(&mut self) {
        while self.window.read_char().is_none() {
            self.update();
            self.draw();
            self.window
                .draw(|ctx| ctx.blt32(&self.bitmap, Point::default()));
        }
    }

    fn make_palettes() -> [TrueColor; 256] {
        let mut palettes = [TrueColor::TRANSPARENT; 256];

        for (i, p) in palettes[0..128].iter_mut().enumerate() {
            let r = (i << (8 - 7)) as u8;
            let g = ((i * i) >> (7 * 2 - 8)) as u8;
            let b = 0;
            *p = ColorComponents::from_rgb(r, g, b).into();
        }
        for (p, i) in palettes[128..].iter_mut().zip((0..=127).rev()) {
            let r = 0;
            let g = (i << (8 - 7)) as u8;
            let b = i as u8;
            *p = ColorComponents::from_rgb(r, g, b).into();
        }

        palettes
    }

    fn make_texture() {
        let texture = unsafe { TEXTURE.get_mut() };

        for (i, p) in texture.iter_mut().enumerate() {
            *p = i as u8;
        }
        let mut r: isize = 0xC9;
        for i in (1..=65536).rev() {
            r += (os_rand() as isize & 3) - 1;
            r += texture[(i + 255) & 0xffff] as isize;
            r = (r >> 1) & 0x7f;
            texture[i & 0xffff] = r as u8;
            texture[(i ^ 0xff00) & 0xffff] = r as u8;
        }
    }

    fn update(&mut self) {
        let s = sin(self.angle);
        let c = cos(self.angle);
        let step = self.step;
        let texture = unsafe { TEXTURE.get_mut() };

        let pi_min = (-(BITMAP_WIDTH as isize) / 2) as f64;
        let pi_max = (BITMAP_WIDTH / 2) as f64;

        let mut pi = pi_min;
        let mut pj = (-(BITMAP_HEIGHT as isize) / 2) as f64;
        for p in self.screen.iter_mut() {
            // 回転～(x-y)
            let x = (pj * c) - (pi * s);
            let mut y = (pj * s) + (pi * c);
            let z: f64;

            if true {
                // 回転～(z-y)
                z = (y * c) - (240.0 * s);
                y = (y * s) + (240.0 * c);
            } else {
                z = 160.0;
            }

            let palx = atan2(y, x) * 128.0 / PI;
            let paly = (z / sqrt(x * x + y * y)) * 128.0 / PI;
            let mut tx = palx as isize;
            let mut ty = (paly as isize) + step;
            let base = if ((tx + ty) & (256 >> 2)) == 0 {
                8
            } else {
                tx = (palx as isize) * 4;
                ty = (paly as isize) * 4 + step;
                if ((tx - ty) & (256 >> 1)) == 0 {
                    16
                } else {
                    tx = (palx as isize) * 8;
                    ty = (paly as isize) * 8 + step;
                    48
                }
            };

            let col = (texture[((ty as usize & 0xff) << 8) + (tx as usize & 0xff)] as isize) - base
                + (*p as isize / 4);
            *p = col as i8;

            pi += 1.0;
            if pi >= pi_max {
                pi = pi_min;
                pj += 1.0;
            }
        }

        self.angle += (7.9 / 360.0) * PI;
        self.step = self.step.wrapping_add(8);
    }

    fn draw(&mut self) {
        let screen = unsafe { SCREEN.get_mut() };
        let p = unsafe { DATA.get_mut() };
        for (s, p) in screen.iter().zip(p.iter_mut()) {
            let argb = self.palettes[*s as usize & 0xFF];
            *p = argb.argb();
        }
    }
}
