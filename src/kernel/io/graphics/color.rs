// Graphics Colors

use core::mem::transmute;
use core::ops::*;

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Color {
    argb: u32,
}

impl Color {
    pub const TRANSPARENT: Self = Self::zero();
    pub const BLACK: Self = Self::from_rgb(0x000000);
    pub const WHITE: Self = Self::from_rgb(0xFFFFFF);

    pub const fn zero() -> Self {
        Color { argb: 0 }
    }

    pub const fn from_rgb(rgb: u32) -> Self {
        Color {
            argb: rgb | 0xFF000000,
        }
    }

    pub const fn from_argb(argb: u32) -> Self {
        Color { argb: argb }
    }

    pub fn components(self) -> ColorComponents {
        self.into()
    }

    pub const fn rgb(self) -> u32 {
        self.argb & 0x00FFFFFF
    }

    pub const fn argb(self) -> u32 {
        self.argb
    }

    pub fn opacity(self) -> u8 {
        self.components().a
    }

    pub fn blend_each<F>(self, rhs: Self, f: F) -> Self
    where
        F: Fn(u8, u8) -> u8,
    {
        self.components().blend_each(rhs.into(), f).into()
    }

    pub fn blend_color<F>(self, rhs: Self, f: F) -> Self
    where
        F: Fn(u8, u8) -> u8,
    {
        self.components().blend_color(rhs.into(), f).into()
    }
}

impl Add for Color {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        self.blend_each(rhs, |a, b| a.saturating_add(b))
    }
}

impl Sub for Color {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        self.blend_each(rhs, |a, b| a.saturating_sub(b))
    }
}

impl Mul<f64> for Color {
    type Output = Self;
    #[inline]
    fn mul(self, opacity: f64) -> Self {
        let alpha = (opacity * 255.0) as u8;
        let mut components = self.components();
        components.a = alpha;
        components.into()
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

    pub fn blend_color<F>(self, rhs: Self, f: F) -> Self
    where
        F: Fn(u8, u8) -> u8,
    {
        Self {
            a: self.a,
            r: f(self.r, rhs.r),
            g: f(self.g, rhs.g),
            b: f(self.b, rhs.b),
        }
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

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum IndexedColor {
    Black = 0,
    Blue,
    Green,
    Cyan,
    Red,
    Magenta,
    Brown,
    LightGray,
    DarkGray,
    LightBlue,
    LightGreen,
    LightCyan,
    LightRed,
    LightMagenta,
    Yellow,
    White,
}

impl From<u8> for IndexedColor {
    fn from(value: u8) -> Self {
        // FIXME: BAD CODE
        match value {
            0 => Self::Black,
            1 => Self::Blue,
            2 => Self::Green,
            3 => Self::Cyan,
            4 => Self::Red,
            5 => Self::Magenta,
            6 => Self::Brown,
            7 => Self::LightGray,
            8 => Self::DarkGray,
            9 => Self::LightBlue,
            10 => Self::LightGreen,
            11 => Self::LightCyan,
            12 => Self::LightRed,
            13 => Self::LightMagenta,
            14 => Self::Yellow,
            _ => Self::White,
        }
    }
}

static mut SYSTEM_COLOR_PALETTE: [u32; 16] = [
    0x000000, 0x0D47A1, 0x1B5E20, 0x006064, 0xb71c1c, 0x4A148C, 0x795548, 0x9E9E9E, 0x616161,
    0x2196F3, 0x4CAF50, 0x00BCD4, 0xf44336, 0x9C27B0, 0xFFEB3B, 0xFFFFFF,
];

impl IndexedColor {
    pub fn as_rgb(self) -> u32 {
        unsafe { SYSTEM_COLOR_PALETTE[self as usize] }
    }

    pub fn as_color(self) -> Color {
        Color::from_rgb(self.as_rgb())
    }
}

impl From<IndexedColor> for Color {
    fn from(index: IndexedColor) -> Self {
        Color::from_rgb(index.as_rgb())
    }
}
