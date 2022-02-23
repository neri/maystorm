use core::mem::transmute;

/// Common color trait
pub trait ColorTrait: Sized + Copy + Clone + PartialEq + Eq + Default {}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IndexedColor(pub u8);

impl ColorTrait for IndexedColor {}

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
        0xFF212121, 0xFF0D47A1, 0xFF1B5E20, 0xFF006064, 0xFFB71C1C, 0xFF4A148C, 0xFF795548,
        0xFFBDBDBD, 0xFF616161, 0xFF2196F3, 0xFF4CAF50, 0xFF00BCD4, 0xFFF44336, 0xFF9C27B0,
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

    #[inline]
    pub const fn brightness(self) -> Option<u8> {
        self.as_true_color().brightness()
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

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct TrueColor(u32);

impl ColorTrait for TrueColor {}

impl TrueColor {
    pub const TRANSPARENT: Self = Self::from_argb(0);
    pub const WHITE: Self = Self::from_rgb(0xFFFFFF);

    #[inline]
    pub const fn from_rgb(rgb: u32) -> Self {
        Self(rgb | 0xFF000000)
    }

    #[inline]
    pub const fn from_argb(argb: u32) -> Self {
        Self(argb)
    }

    #[inline]
    pub const fn from_gray(white: u8, alpha: u8) -> Self {
        Self(white as u32 * 0x00_01_01_01 + alpha as u32 * 0x01_00_00_00)
    }

    #[inline]
    pub const fn components(&self) -> ColorComponents {
        ColorComponents::from_true_color(*self)
    }

    #[inline]
    pub const fn rgb(&self) -> u32 {
        self.0 & 0x00FFFFFF
    }

    #[inline]
    pub const fn argb(&self) -> u32 {
        self.0
    }

    #[inline]
    pub const fn brightness(&self) -> Option<u8> {
        let cc = self.components();
        match cc.a {
            0 => None,
            _ => Some(
                ((cc.r as usize * 19589 + cc.g as usize * 38444 + cc.b as usize * 7502 + 32767)
                    >> 16) as u8,
            ),
        }
    }

    #[inline]
    pub const fn opacity(&self) -> u8 {
        (self.0 >> 24) as u8
    }

    #[inline]
    pub const fn with_opacity(&self, alpha: u8) -> Self {
        let mut components = self.components();
        components.a = alpha;
        components.into_true_color()
    }

    #[inline]
    pub const fn is_opaque(&self) -> bool {
        self.opacity() == 0xFF
    }

    #[inline]
    pub const fn is_transparent(&self) -> bool {
        self.opacity() == 0
    }

    #[inline]
    pub fn blend_each<F>(&self, rhs: Self, f: F) -> Self
    where
        F: Fn(u8, u8) -> u8,
    {
        self.components().blend_each(rhs.into(), f).into()
    }

    #[inline]
    pub fn blend_color<F1, F2>(&self, rhs: Self, f_rgb: F1, f_a: F2) -> Self
    where
        F1: Fn(u8, u8) -> u8,
        F2: Fn(u8, u8) -> u8,
    {
        self.components().blend_color(rhs.into(), f_rgb, f_a).into()
    }

    #[inline]
    pub fn blend(&self, rhs: Self) -> Self {
        let c = rhs.components();
        let alpha_l = c.a as usize;
        let alpha_r = 255 - alpha_l;
        c.blend_each(self.components(), |l, r| {
            ((l as usize * alpha_l + r as usize * alpha_r) / 255) as u8
        })
        .into()
    }

    #[inline]
    pub fn blend_draw(&self, rhs: Self) -> Self {
        let r = rhs.components();
        let l = self.components();
        let alpha_r = r.a as usize;
        let alpha_l = l.a as usize * (256 - alpha_r) / 256;
        let out_a = alpha_r + alpha_l;
        if out_a > 0 {
            l.blend_color(
                r,
                |l, r| ((l as usize * alpha_l + r as usize * alpha_r) / out_a) as u8,
                |_, _| out_a as u8,
            )
            .into()
        } else {
            Self::TRANSPARENT
        }
    }
}

impl From<u32> for TrueColor {
    fn from(argb: u32) -> Self {
        Self::from_argb(argb)
    }
}

impl From<TrueColor> for IndexedColor {
    fn from(color: TrueColor) -> Self {
        Self::from_rgb(color.rgb())
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
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            r,
            g,
            b,
            a: u8::MAX,
        }
    }

    #[inline]
    pub const fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    #[inline]
    pub const fn from_true_color(val: TrueColor) -> Self {
        unsafe { transmute(val) }
    }

    #[inline]
    pub const fn into_true_color(self) -> TrueColor {
        unsafe { transmute(self) }
    }

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

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DeepColor30 {
    rgb: u32,
}

impl ColorTrait for DeepColor30 {}

impl DeepColor30 {
    #[inline]
    pub const fn from_rgb(rgb: u32) -> Self {
        Self { rgb }
    }

    #[inline]
    pub const fn from_true_color(val: TrueColor) -> Self {
        let rgb32 = val.argb();
        let components = (
            (rgb32 & 0xFF),
            ((rgb32 >> 8) & 0xFF),
            ((rgb32 >> 16) & 0xFF),
        );
        Self {
            rgb: Self::c8c10(components.0)
                | (Self::c8c10(components.1) << 10)
                | (Self::c8c10(components.2) << 20),
        }
    }

    #[inline]
    pub const fn components(&self) -> (u32, u32, u32) {
        let rgb = self.rgb();
        ((rgb & 0x3FF), ((rgb >> 10) & 0x3FF), ((rgb >> 20) & 0x3FF))
    }

    #[inline]
    pub const fn into_true_color(&self) -> TrueColor {
        let components = self.components();
        TrueColor::from_rgb(
            (components.0 >> 2) | ((components.1 >> 2) << 8) | ((components.2 >> 2) << 16),
        )
    }

    /// Convert 8bit color component to 10bit color component
    const fn c8c10(c8: u32) -> u32 {
        (c8 * 0x0101) >> 6
    }

    #[inline]
    pub const fn rgb(&self) -> u32 {
        self.rgb
    }
}

impl From<TrueColor> for DeepColor30 {
    fn from(val: TrueColor) -> Self {
        Self::from_true_color(val)
    }
}

impl From<DeepColor30> for TrueColor {
    fn from(val: DeepColor30) -> Self {
        val.into_true_color()
    }
}

/// A type that represents a generic color.
///
/// The [Color] type is convertible to the [PackedColor] type and each other, with some exceptions.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Color {
    Transparent,
    Indexed(IndexedColor),
    Argb32(TrueColor),
}

impl ColorTrait for Color {}

impl Color {
    pub const TRANSPARENT: Self = Self::Transparent;

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
    pub const DEFAULT_KEY: Self = Self::Indexed(IndexedColor::DEFAULT_KEY);

    #[inline]
    pub const fn from_rgb(rgb: u32) -> Self {
        Self::Argb32(TrueColor::from_rgb(rgb))
    }

    #[inline]
    pub const fn from_argb(argb: u32) -> Self {
        Self::Argb32(TrueColor::from_argb(argb))
    }

    #[inline]
    pub const fn into_indexed(&self) -> IndexedColor {
        match self {
            Color::Transparent => IndexedColor::DEFAULT_KEY,
            Color::Indexed(v) => *v,
            Color::Argb32(v) => IndexedColor::from_rgb(v.rgb()),
        }
    }

    #[inline]
    pub const fn into_true_color(&self) -> TrueColor {
        match self {
            Color::Transparent => TrueColor::TRANSPARENT,
            Color::Indexed(v) => v.as_true_color(),
            Color::Argb32(v) => *v,
        }
    }

    #[inline]
    pub const fn brightness(&self) -> Option<u8> {
        match self {
            Color::Transparent => None,
            Color::Indexed(c) => c.brightness(),
            Color::Argb32(c) => c.brightness(),
        }
    }

    #[inline]
    pub const fn is_transparent(&self) -> bool {
        match self {
            Color::Transparent => true,
            Color::Indexed(c) => match *c {
                IndexedColor::DEFAULT_KEY => true,
                _ => false,
            },
            Color::Argb32(c) => c.is_transparent(),
        }
    }
}

impl Default for Color {
    #[inline]
    fn default() -> Self {
        Self::TRANSPARENT
    }
}

impl Into<IndexedColor> for Color {
    #[inline]
    fn into(self) -> IndexedColor {
        self.into_indexed()
    }
}

impl Into<TrueColor> for Color {
    #[inline]
    fn into(self) -> TrueColor {
        self.into_true_color()
    }
}

impl From<IndexedColor> for Color {
    #[inline]
    fn from(val: IndexedColor) -> Self {
        Self::Indexed(val)
    }
}

impl From<TrueColor> for Color {
    #[inline]
    fn from(val: TrueColor) -> Self {
        Self::Argb32(val)
    }
}

/// A color type that packed into 32 bits
///
/// The [PackedColor] type is convertible to the [Color] type and each other, with some exceptions.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PackedColor(pub u32);

impl ColorTrait for PackedColor {}

impl PackedColor {
    pub const TRANSPARENT: Self = Self(0x100);
    const INDEX_COLOR_MIN: u32 = 0;
    const INDEX_COLOR_MAX: u32 = Self::INDEX_COLOR_MIN + 0xFF;

    pub const BLACK: Self = Self::from_indexed(IndexedColor::BLACK);
    pub const BLUE: Self = Self::from_indexed(IndexedColor::BLUE);
    pub const GREEN: Self = Self::from_indexed(IndexedColor::GREEN);
    pub const CYAN: Self = Self::from_indexed(IndexedColor::CYAN);
    pub const RED: Self = Self::from_indexed(IndexedColor::RED);
    pub const MAGENTA: Self = Self::from_indexed(IndexedColor::MAGENTA);
    pub const BROWN: Self = Self::from_indexed(IndexedColor::BROWN);
    pub const LIGHT_GRAY: Self = Self::from_indexed(IndexedColor::LIGHT_GRAY);
    pub const DARK_GRAY: Self = Self::from_indexed(IndexedColor::DARK_GRAY);
    pub const LIGHT_BLUE: Self = Self::from_indexed(IndexedColor::LIGHT_BLUE);
    pub const LIGHT_GREEN: Self = Self::from_indexed(IndexedColor::LIGHT_GREEN);
    pub const LIGHT_CYAN: Self = Self::from_indexed(IndexedColor::LIGHT_CYAN);
    pub const LIGHT_RED: Self = Self::from_indexed(IndexedColor::LIGHT_RED);
    pub const LIGHT_MAGENTA: Self = Self::from_indexed(IndexedColor::LIGHT_MAGENTA);
    pub const YELLOW: Self = Self::from_indexed(IndexedColor::YELLOW);
    pub const WHITE: Self = Self::from_indexed(IndexedColor::WHITE);

    #[inline]
    pub const fn from_argb(argb: u32) -> Self {
        Self::from_true_color(TrueColor::from_argb(argb))
    }

    #[inline]
    pub const fn from_safe_rgb(rgb: u32) -> Self {
        Self::from_indexed(IndexedColor::from_rgb(rgb))
    }

    #[inline]
    pub const fn from_true_color(argb: TrueColor) -> Self {
        match argb.is_transparent() {
            true => Self::TRANSPARENT,
            false => Self(argb.argb()),
        }
    }

    #[inline]
    pub const fn from_indexed(index: IndexedColor) -> Self {
        match index {
            IndexedColor::DEFAULT_KEY => Self::TRANSPARENT,
            _ => Self(Self::INDEX_COLOR_MIN + index.0 as u32),
        }
    }

    #[inline]
    pub const fn from_color(color: Color) -> Self {
        match color {
            Color::Transparent => Self::TRANSPARENT,
            Color::Indexed(index) => Self::from_indexed(index),
            Color::Argb32(argb) => Self::from_true_color(argb),
        }
    }

    #[inline]
    pub const fn as_color(&self) -> Color {
        if self.0 == Self::TRANSPARENT.0 {
            Color::Transparent
        } else if self.0 >= Self::INDEX_COLOR_MIN && self.0 <= Self::INDEX_COLOR_MAX {
            Color::Indexed(IndexedColor((self.0 - Self::INDEX_COLOR_MIN) as u8))
        } else {
            Color::from_argb(self.0)
        }
    }

    #[inline]
    pub const fn into_true_color(&self) -> TrueColor {
        self.as_color().into_true_color()
    }

    #[inline]
    pub const fn into_indexed(&self) -> IndexedColor {
        self.as_color().into_indexed()
    }
}

impl From<TrueColor> for PackedColor {
    #[inline]
    fn from(color: TrueColor) -> Self {
        Self::from_true_color(color)
    }
}

impl From<IndexedColor> for PackedColor {
    #[inline]
    fn from(index: IndexedColor) -> Self {
        Self::from_indexed(index)
    }
}

impl From<Color> for PackedColor {
    #[inline]
    fn from(color: Color) -> Self {
        Self::from_color(color)
    }
}

impl From<PackedColor> for Color {
    #[inline]
    fn from(color: PackedColor) -> Self {
        color.as_color()
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct RGB555(pub u16);

impl ColorTrait for RGB555 {}

impl RGB555 {
    #[inline]
    pub const fn components(&self) -> (u8, u8, u8) {
        let b = (self.0 & 0x1F) as u8;
        let g = ((self.0 >> 5) & 0x1F) as u8;
        let r = ((self.0 >> 10) & 0x1F) as u8;
        (r, g, b)
    }

    #[inline]
    pub const fn from_components(r: u8, g: u8, b: u8) -> Self {
        Self(((r as u16) << 10) | ((g as u16) << 5) | (b as u16))
    }

    #[inline]
    pub const fn as_true_color(&self) -> TrueColor {
        let components = self.components();
        let components = ColorComponents {
            a: u8::MAX,
            r: Self::c5c8(components.2),
            g: Self::c5c8(components.1),
            b: Self::c5c8(components.0),
        };
        components.into_true_color()
    }

    const fn c5c8(c: u8) -> u8 {
        (c << 3) | (c >> 2)
    }

    #[inline]
    pub const fn from_true_color(color: TrueColor) -> Self {
        let components = color.components();
        Self(
            ((components.b >> 3) as u16)
                | (((components.g >> 3) as u16) << 5)
                | (((components.r >> 3) as u16) << 10),
        )
    }
}

impl From<TrueColor> for RGB555 {
    #[inline]
    fn from(color: TrueColor) -> Self {
        Self::from_true_color(color)
    }
}

impl From<RGB555> for TrueColor {
    #[inline]
    fn from(color: RGB555) -> Self {
        color.as_true_color()
    }
}

impl From<Color> for RGB555 {
    #[inline]
    fn from(color: Color) -> Self {
        Self::from_true_color(color.into_true_color())
    }
}

impl From<RGB555> for Color {
    #[inline]
    fn from(color: RGB555) -> Self {
        Color::Argb32(color.as_true_color())
    }
}
