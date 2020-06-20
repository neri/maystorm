// Graphics Colors

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Color {
    rgb: u32,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Color {
            rgb: ((r as u32) * 0x10000) + ((g as u32) * 0x100) + (b as u32),
        }
    }

    pub const fn rgb(&self) -> u32 {
        self.rgb
    }

    pub const fn components(&self) -> [u8; 3] {
        let r = (self.rgb >> 16) as u8;
        let g = (self.rgb >> 8) as u8;
        let b = self.rgb as u8;
        [r, g, b]
    }
}

impl From<u32> for Color {
    fn from(rgb: u32) -> Self {
        Color { rgb: rgb }
    }
}

impl From<[u8; 3]> for Color {
    fn from(components: [u8; 3]) -> Self {
        Color::new(components[0], components[1], components[2])
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
// 0x000000, 0x0000AA, 0x00AA00, 0x00AAAA, 0xAA0000, 0xAA00AA, 0xAA5500, 0xAAAAAA, 0x555555,
// 0x5555FF, 0x55FF55, 0x55FFFF, 0xFF5555, 0xFF55FF, 0xFFFF55, 0xFFFFFF,

impl IndexedColor {
    pub fn as_rgb(&self) -> u32 {
        unsafe { SYSTEM_COLOR_PALETTE[*self as usize] }
    }

    pub fn as_color(&self) -> Color {
        Color::from(self.as_rgb())
    }
}

impl From<IndexedColor> for Color {
    fn from(index: IndexedColor) -> Self {
        Color::from(index.as_rgb())
    }
}
