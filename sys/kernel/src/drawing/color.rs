// Colors

use core::mem::transmute;

pub trait ColorTrait: Sized + Copy + Clone + PartialEq + Eq {}

impl ColorTrait for IndexedColor {}
impl ColorTrait for TrueColor {}
impl ColorTrait for AmbiguousColor {}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexedColor(pub u8);

impl IndexedColor {
    pub const MIN: Self = Self(u8::MIN);
    pub const MAX: Self = Self(u8::MAX);
    pub const DEFAULT_KEY: Self = Self(u8::MAX);

    pub const BLACK: Self = Self(0);
    pub const BLUE: Self = Self(1);
    pub const GREEN: Self = Self(2);
    pub const CYAN: Self = Self(3);
    pub const RED: Self = Self(4);
    pub const MAGENTA: Self = Self(5);
    pub const BROWN: Self = Self(6);
    pub const LIGHT_GRAY: Self = Self(7);
    pub const DARK_GRAY: Self = Self(8);
    pub const LIGHT_BLUE: Self = Self(9);
    pub const LIGHT_GREEN: Self = Self(10);
    pub const LIGHT_CYAN: Self = Self(11);
    pub const LIGHT_RED: Self = Self(12);
    pub const LIGHT_MAGENTA: Self = Self(13);
    pub const YELLOW: Self = Self(14);
    pub const WHITE: Self = Self(15);

    pub const COLOR_PALETTE: [u32; 256] = [
        0xFF212121, 0xFF0D47A1, 0xFF1B5E20, 0xFF006064, 0xFFb71c1c, 0xFF4A148C, 0xFF795548,
        0xFF9E9E9E, 0xFF616161, 0xFF2196F3, 0xFF4CAF50, 0xFF00BCD4, 0xFFf44336, 0xFF9C27B0,
        0xFFFFEB3B, 0xFFFFFFFF, 0xFF000000, 0xFF330000, 0xFF660000, 0xFF990000, 0xFFCC0000,
        0xFFFF0000, 0xFF003300, 0xFF333300, 0xFF663300, 0xFF993300, 0xFFCC3300, 0xFFFF3300,
        0xFF006600, 0xFF336600, 0xFF666600, 0xFF996600, 0xFFCC6600, 0xFFFF6600, 0xFF009900,
        0xFF339900, 0xFF669900, 0xFF999900, 0xFFCC9900, 0xFFFF9900, 0xFF00CC00, 0xFF33CC00,
        0xFF66CC00, 0xFF99CC00, 0xFFCCCC00, 0xFFFFCC00, 0xFF00FF00, 0xFF33FF00, 0xFF66FF00,
        0xFF99FF00, 0xFFCCFF00, 0xFFFFFF00, 0xFF000033, 0xFF330033, 0xFF660033, 0xFF990033,
        0xFFCC0033, 0xFFFF0033, 0xFF003333, 0xFF333333, 0xFF663333, 0xFF993333, 0xFFCC3333,
        0xFFFF3333, 0xFF006633, 0xFF336633, 0xFF666633, 0xFF996633, 0xFFCC6633, 0xFFFF6633,
        0xFF009933, 0xFF339933, 0xFF669933, 0xFF999933, 0xFFCC9933, 0xFFFF9933, 0xFF00CC33,
        0xFF33CC33, 0xFF66CC33, 0xFF99CC33, 0xFFCCCC33, 0xFFFFCC33, 0xFF00FF33, 0xFF33FF33,
        0xFF66FF33, 0xFF99FF33, 0xFFCCFF33, 0xFFFFFF33, 0xFF000066, 0xFF330066, 0xFF660066,
        0xFF990066, 0xFFCC0066, 0xFFFF0066, 0xFF003366, 0xFF333366, 0xFF663366, 0xFF993366,
        0xFFCC3366, 0xFFFF3366, 0xFF006666, 0xFF336666, 0xFF666666, 0xFF996666, 0xFFCC6666,
        0xFFFF6666, 0xFF009966, 0xFF339966, 0xFF669966, 0xFF999966, 0xFFCC9966, 0xFFFF9966,
        0xFF00CC66, 0xFF33CC66, 0xFF66CC66, 0xFF99CC66, 0xFFCCCC66, 0xFFFFCC66, 0xFF00FF66,
        0xFF33FF66, 0xFF66FF66, 0xFF99FF66, 0xFFCCFF66, 0xFFFFFF66, 0xFF000099, 0xFF330099,
        0xFF660099, 0xFF990099, 0xFFCC0099, 0xFFFF0099, 0xFF003399, 0xFF333399, 0xFF663399,
        0xFF993399, 0xFFCC3399, 0xFFFF3399, 0xFF006699, 0xFF336699, 0xFF666699, 0xFF996699,
        0xFFCC6699, 0xFFFF6699, 0xFF009999, 0xFF339999, 0xFF669999, 0xFF999999, 0xFFCC9999,
        0xFFFF9999, 0xFF00CC99, 0xFF33CC99, 0xFF66CC99, 0xFF99CC99, 0xFFCCCC99, 0xFFFFCC99,
        0xFF00FF99, 0xFF33FF99, 0xFF66FF99, 0xFF99FF99, 0xFFCCFF99, 0xFFFFFF99, 0xFF0000CC,
        0xFF3300CC, 0xFF6600CC, 0xFF9900CC, 0xFFCC00CC, 0xFFFF00CC, 0xFF0033CC, 0xFF3333CC,
        0xFF6633CC, 0xFF9933CC, 0xFFCC33CC, 0xFFFF33CC, 0xFF0066CC, 0xFF3366CC, 0xFF6666CC,
        0xFF9966CC, 0xFFCC66CC, 0xFFFF66CC, 0xFF0099CC, 0xFF3399CC, 0xFF6699CC, 0xFF9999CC,
        0xFFCC99CC, 0xFFFF99CC, 0xFF00CCCC, 0xFF33CCCC, 0xFF66CCCC, 0xFF99CCCC, 0xFFCCCCCC,
        0xFFFFCCCC, 0xFF00FFCC, 0xFF33FFCC, 0xFF66FFCC, 0xFF99FFCC, 0xFFCCFFCC, 0xFFFFFFCC,
        0xFF0000FF, 0xFF3300FF, 0xFF6600FF, 0xFF9900FF, 0xFFCC00FF, 0xFFFF00FF, 0xFF0033FF,
        0xFF3333FF, 0xFF6633FF, 0xFF9933FF, 0xFFCC33FF, 0xFFFF33FF, 0xFF0066FF, 0xFF3366FF,
        0xFF6666FF, 0xFF9966FF, 0xFFCC66FF, 0xFFFF66FF, 0xFF0099FF, 0xFF3399FF, 0xFF6699FF,
        0xFF9999FF, 0xFFCC99FF, 0xFFFF99FF, 0xFF00CCFF, 0xFF33CCFF, 0xFF66CCFF, 0xFF99CCFF,
        0xFFCCCCFF, 0xFFFFCCFF, 0xFF00FFFF, 0xFF33FFFF, 0xFF66FFFF, 0xFF99FFFF, 0xFFCCFFFF,
        0xFFFFFFFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];

    #[inline]
    pub const fn from_rgb(rgb: u32) -> Self {
        let b = (((rgb & 0xFF) + 25) / 51) as u8;
        let g = ((((rgb >> 8) & 0xFF) + 25) / 51) as u8;
        let r = ((((rgb >> 16) & 0xFF) + 25) / 51) as u8;
        Self(16 + r + g * 6 + b * 36)
    }

    #[inline]
    pub const fn as_rgb(self) -> u32 {
        Self::COLOR_PALETTE[self.0 as usize] & 0xFF_FF_FF
    }

    #[inline]
    pub const fn as_argb(self) -> u32 {
        Self::COLOR_PALETTE[self.0 as usize]
    }

    #[inline]
    pub const fn as_true_color(self) -> TrueColor {
        TrueColor::from_argb(self.as_argb())
    }
}

impl From<u8> for IndexedColor {
    fn from(val: u8) -> Self {
        Self(val)
    }
}

impl From<IndexedColor> for TrueColor {
    fn from(val: IndexedColor) -> Self {
        val.as_true_color()
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TrueColor {
    argb: u32,
}

impl TrueColor {
    pub const TRANSPARENT: Self = Self::from_argb(0);
    pub const WHITE: Self = Self::from_rgb(0xFFFFFF);

    #[inline]
    pub const fn from_rgb(rgb: u32) -> Self {
        Self {
            argb: rgb | 0xFF000000,
        }
    }

    #[inline]
    pub const fn from_argb(argb: u32) -> Self {
        Self { argb }
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

impl From<u32> for TrueColor {
    fn from(val: u32) -> Self {
        Self::from_argb(val)
    }
}

impl From<TrueColor> for IndexedColor {
    fn from(val: TrueColor) -> Self {
        Self::from_rgb(val.rgb())
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

impl From<TrueColor> for ColorComponents {
    fn from(color: TrueColor) -> Self {
        unsafe { transmute(color) }
    }
}

impl From<ColorComponents> for TrueColor {
    fn from(components: ColorComponents) -> Self {
        unsafe { transmute(components) }
    }
}

impl Into<u32> for ColorComponents {
    fn into(self) -> u32 {
        unsafe { transmute(self) }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AmbiguousColor {
    Indexed(IndexedColor),
    Argb32(TrueColor),
}

impl AmbiguousColor {
    pub const TRANSPARENT: Self = Self::Argb32(TrueColor::TRANSPARENT);
    pub const BLACK: Self = Self::Indexed(IndexedColor::BLACK);
    pub const BLUE: Self = Self::Indexed(IndexedColor::BLUE);
    pub const GREEN: Self = Self::Indexed(IndexedColor::GREEN);
    pub const CYAN: Self = Self::Indexed(IndexedColor::CYAN);
    pub const RED: Self = Self::Indexed(IndexedColor::RED);
    pub const MAGENTA: Self = Self::Indexed(IndexedColor::MAGENTA);
    pub const BROWN: Self = Self::Indexed(IndexedColor::BROWN);
    pub const LIGHT_GRAY: Self = Self::Indexed(IndexedColor::LIGHT_GRAY);
    pub const DARK_GRAY: Self = Self::Indexed(IndexedColor::DARK_GRAY);
    pub const LIGHT_BLUE: Self = Self::Indexed(IndexedColor::LIGHT_BLUE);
    pub const LIGHT_GREEN: Self = Self::Indexed(IndexedColor::LIGHT_GREEN);
    pub const LIGHT_CYAN: Self = Self::Indexed(IndexedColor::LIGHT_CYAN);
    pub const LIGHT_RED: Self = Self::Indexed(IndexedColor::LIGHT_RED);
    pub const LIGHT_MAGENTA: Self = Self::Indexed(IndexedColor::LIGHT_MAGENTA);
    pub const YELLOW: Self = Self::Indexed(IndexedColor::YELLOW);
    pub const WHITE: Self = Self::Indexed(IndexedColor::WHITE);

    #[inline]
    pub const fn from_rgb(rgb: u32) -> Self {
        Self::Argb32(TrueColor::from_rgb(rgb))
    }

    #[inline]
    pub const fn from_argb(rgb: u32) -> Self {
        Self::Argb32(TrueColor::from_argb(rgb))
    }

    #[inline]
    pub const fn into_argb(&self) -> TrueColor {
        match self {
            AmbiguousColor::Indexed(v) => v.as_true_color(),
            AmbiguousColor::Argb32(v) => *v,
        }
    }
}

impl Into<IndexedColor> for AmbiguousColor {
    fn into(self) -> IndexedColor {
        match self {
            AmbiguousColor::Indexed(v) => v,
            AmbiguousColor::Argb32(v) => v.into(),
        }
    }
}

impl Into<TrueColor> for AmbiguousColor {
    fn into(self) -> TrueColor {
        match self {
            AmbiguousColor::Indexed(v) => v.into(),
            AmbiguousColor::Argb32(v) => v,
        }
    }
}

impl From<IndexedColor> for AmbiguousColor {
    fn from(val: IndexedColor) -> Self {
        Self::Indexed(val)
    }
}

impl From<TrueColor> for AmbiguousColor {
    fn from(val: TrueColor) -> Self {
        Self::Argb32(val)
    }
}
