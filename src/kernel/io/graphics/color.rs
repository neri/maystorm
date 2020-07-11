// Graphics Colors

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Color {
    rgb: u32,
}

impl Color {
    pub const TRANPARENT: Self = Self::zero();

    pub const fn zero() -> Self {
        Color { rgb: 0 }
    }

    pub const fn from_rgb(rgb: u32) -> Self {
        Color {
            rgb: rgb | 0xFF000000,
        }
    }

    pub const fn from_argb(argb: u32) -> Self {
        Color { rgb: argb }
    }

    pub fn components(self) -> ColorComponents {
        self.into()
    }

    pub const fn rgb(&self) -> u32 {
        self.rgb
    }

    pub fn alpha(&self) -> u8 {
        self.components().a
    }

    pub fn set_opacity(&mut self, alpha: u8) -> Self {
        self.rgb = (self.rgb & 0x00FFFFFF) | ((alpha as u32) << 24);
        *self
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
    pub fn blend_each<F>(&self, rhs: Self, f: F) -> Self
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
}

impl From<Color> for ColorComponents {
    fn from(color: Color) -> Self {
        unsafe { core::mem::transmute(color) }
    }
}

impl From<ColorComponents> for Color {
    fn from(components: ColorComponents) -> Self {
        unsafe { core::mem::transmute(components) }
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
    pub fn as_rgb(&self) -> u32 {
        unsafe { SYSTEM_COLOR_PALETTE[*self as usize] }
    }

    pub fn as_color(&self) -> Color {
        Color::from_rgb(self.as_rgb())
    }
}

impl From<IndexedColor> for Color {
    fn from(index: IndexedColor) -> Self {
        Color::from_rgb(index.as_rgb())
    }
}
