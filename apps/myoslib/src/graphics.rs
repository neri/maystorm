// myos Graphics Library

use core::mem::transmute;

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Color {
    argb: u32,
}

impl Color {
    pub const TRANSPARENT: Self = Self::zero();
    pub const WHITE: Self = Self::from_rgb(0xFFFFFF);

    #[inline]
    pub const fn zero() -> Self {
        Color { argb: 0 }
    }

    #[inline]
    pub const fn from_rgb(rgb: u32) -> Self {
        Color {
            argb: rgb | 0xFF000000,
        }
    }

    #[inline]
    pub const fn from_argb(argb: u32) -> Self {
        Color { argb }
    }

    #[inline]
    pub const fn gray(white: u8, alpha: u8) -> Self {
        Self {
            argb: white as u32 * 0x00_01_01_01 + alpha as u32 * 0x01_00_00_00,
        }
    }

    #[inline]
    pub fn components(self) -> ColorComponents {
        self.into()
    }

    #[inline]
    pub const fn rgb(self) -> u32 {
        self.argb & 0x00FFFFFF
    }

    #[inline]
    pub const fn argb(self) -> u32 {
        self.argb
    }

    #[inline]
    pub fn brightness(self) -> u8 {
        let cc = self.components();
        ((cc.r as usize * 19589 + cc.g as usize * 38444 + cc.b as usize * 7502 + 32767) >> 16) as u8
    }

    #[inline]
    pub const fn opacity(self) -> u8 {
        (self.argb >> 24) as u8
    }

    #[inline]
    pub fn set_opacity(mut self, alpha: u8) -> Self {
        let mut components = self.components();
        components.a = alpha;
        self.argb = components.into();
        self
    }

    #[inline]
    pub const fn is_opaque(self) -> bool {
        self.opacity() == 0xFF
    }

    #[inline]
    pub const fn is_transparent(self) -> bool {
        self.opacity() == 0
    }

    #[inline]
    pub fn blend_each<F>(self, rhs: Self, f: F) -> Self
    where
        F: Fn(u8, u8) -> u8,
    {
        self.components().blend_each(rhs.into(), f).into()
    }

    #[inline]
    pub fn blend_color<F1, F2>(self, rhs: Self, f_rgb: F1, f_a: F2) -> Self
    where
        F1: Fn(u8, u8) -> u8,
        F2: Fn(u8, u8) -> u8,
    {
        self.components().blend_color(rhs.into(), f_rgb, f_a).into()
    }

    #[inline]
    pub fn blend(self, other: Self) -> Self {
        let c = other.components();
        let alpha_l = c.a as usize;
        let alpha_r = 255 - alpha_l;
        c.blend_each(self.components(), |a, b| {
            ((a as usize * alpha_l + b as usize * alpha_r) / 255) as u8
        })
        .into()
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::TRANSPARENT
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
#[cfg(target_endian = "little")]
pub struct ColorComponents {
    pub b: u8,
    pub g: u8,
    pub r: u8,
    pub a: u8,
}

impl ColorComponents {
    #[inline]
    pub fn blend_each<F>(self, rhs: Self, f: F) -> Self
    where
        F: Fn(u8, u8) -> u8,
    {
        Self {
            a: f(self.a, rhs.a),
            r: f(self.r, rhs.r),
            g: f(self.g, rhs.g),
            b: f(self.b, rhs.b),
        }
    }

    #[inline]
    pub fn blend_color<F1, F2>(self, rhs: Self, f_rgb: F1, f_a: F2) -> Self
    where
        F1: Fn(u8, u8) -> u8,
        F2: Fn(u8, u8) -> u8,
    {
        Self {
            a: f_a(self.a, rhs.a),
            r: f_rgb(self.r, rhs.r),
            g: f_rgb(self.g, rhs.g),
            b: f_rgb(self.b, rhs.b),
        }
    }

    #[inline]
    pub const fn is_opaque(self) -> bool {
        self.a == 255
    }

    #[inline]
    pub const fn is_transparent(self) -> bool {
        self.a == 0
    }
}

impl From<Color> for ColorComponents {
    fn from(color: Color) -> Self {
        unsafe { transmute(color) }
    }
}

impl From<ColorComponents> for Color {
    fn from(components: ColorComponents) -> Self {
        unsafe { transmute(components) }
    }
}

impl Into<u32> for ColorComponents {
    fn into(self) -> u32 {
        unsafe { transmute(self) }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Point {
    pub x: isize,
    pub y: isize,
}

impl Point {
    #[inline]
    pub const fn new(x: isize, y: isize) -> Self {
        Self { x, y }
    }

    #[inline]
    pub const fn x(&self) -> isize {
        self.x
    }

    #[inline]
    pub const fn y(&self) -> isize {
        self.y
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Size {
    pub width: isize,
    pub height: isize,
}

impl Size {
    #[inline]
    pub const fn new(width: isize, height: isize) -> Self {
        Self { width, height }
    }

    #[inline]
    pub const fn width(&self) -> isize {
        self.width
    }

    #[inline]
    pub const fn height(&self) -> isize {
        self.height
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    #[inline]
    pub const fn new(x: isize, y: isize, width: isize, height: isize) -> Self {
        Self {
            origin: Point { x, y },
            size: Size { width, height },
        }
    }

    #[inline]
    pub const fn origin(&self) -> Point {
        self.origin
    }

    #[inline]
    pub const fn x(&self) -> isize {
        self.origin.x
    }

    #[inline]
    pub const fn y(&self) -> isize {
        self.origin.y
    }

    #[inline]
    pub const fn size(&self) -> Size {
        self.size
    }

    #[inline]
    pub const fn width(&self) -> isize {
        self.size.width
    }

    #[inline]
    pub const fn height(&self) -> isize {
        self.size.height
    }
}
