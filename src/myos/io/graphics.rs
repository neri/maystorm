// Graphics Services
use crate::num::*;
use core::mem::swap;
use core::ops::*;

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Point<T: Number> {
    pub x: T,
    pub y: T,
}

impl<T: Number> Point<T> {
    pub const fn new(x: T, y: T) -> Self {
        Point { x: x, y: y }
    }
}

impl<T: Number> From<(T, T)> for Point<T> {
    fn from(p: (T, T)) -> Self {
        Self::new(p.0, p.1)
    }
}

impl<T: Number> Zero for Point<T> {
    fn zero() -> Self {
        Point {
            x: T::zero(),
            y: T::zero(),
        }
    }
}

impl<T: Number> Add for Point<T> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Point {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl<T: Number> AddAssign for Point<T> {
    fn add_assign(&mut self, rhs: Self) {
        *self = Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl<T: Number> Sub for Point<T> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Point {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl<T: Number> SubAssign for Point<T> {
    fn sub_assign(&mut self, rhs: Self) {
        *self = Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Size<T: Number> {
    pub width: T,
    pub height: T,
}

impl<T: Number> Size<T> {
    pub fn new(width: T, height: T) -> Self {
        Size {
            width: width,
            height: height,
        }
    }
}

impl<T: Number> From<(T, T)> for Size<T> {
    fn from(p: (T, T)) -> Self {
        Self::new(p.0, p.1)
    }
}

impl<T: Number> Zero for Size<T> {
    fn zero() -> Self {
        Size {
            width: T::zero(),
            height: T::zero(),
        }
    }
}

impl<T: Number> Add for Size<T> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Size {
            width: self.width + rhs.width,
            height: self.height + rhs.height,
        }
    }
}

impl<T: Number> AddAssign for Size<T> {
    fn add_assign(&mut self, rhs: Self) {
        *self = Self {
            width: self.width + rhs.width,
            height: self.height + rhs.height,
        }
    }
}

impl<T: Number> Sub for Size<T> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Size {
            width: self.width - rhs.width,
            height: self.height - rhs.height,
        }
    }
}

impl<T: Number> SubAssign for Size<T> {
    fn sub_assign(&mut self, rhs: Self) {
        *self = Self {
            width: self.width - rhs.width,
            height: self.height - rhs.height,
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Rect<T: Number> {
    pub origin: Point<T>,
    pub size: Size<T>,
}

impl<T: Number> Rect<T> {
    pub fn new(x: T, y: T, width: T, height: T) -> Self {
        Rect {
            origin: Point { x: x, y: y },
            size: Size {
                width: width,
                height: height,
            },
        }
    }

    pub fn insets_by(&self, insets: EdgeInsets<T>) -> Self {
        Rect {
            origin: Point {
                x: self.origin.x + insets.left,
                y: self.origin.y + insets.top,
            },
            size: Size {
                width: self.size.width - (insets.left + insets.right),
                height: self.size.height - (insets.top + insets.bottom),
            },
        }
    }
}

impl<T: Number> From<(T, T, T, T)> for Rect<T> {
    fn from(p: (T, T, T, T)) -> Self {
        Self::new(p.0, p.1, p.2, p.3)
    }
}

impl<T: Number> Zero for Rect<T> {
    fn zero() -> Self {
        Rect {
            origin: Point::zero(),
            size: Size::zero(),
        }
    }
}

impl<T: Number> From<Size<T>> for Rect<T> {
    fn from(size: Size<T>) -> Self {
        Rect {
            origin: Point::zero(),
            size: size,
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct EdgeInsets<T: Number> {
    pub top: T,
    pub left: T,
    pub bottom: T,
    pub right: T,
}

impl<T: Number> EdgeInsets<T> {
    pub fn new(top: T, left: T, bottom: T, right: T) -> Self {
        EdgeInsets {
            top: top,
            left: left,
            bottom: bottom,
            right: right,
        }
    }
}

impl<T: Number> From<(T, T, T, T)> for EdgeInsets<T> {
    fn from(p: (T, T, T, T)) -> Self {
        Self::new(p.0, p.1, p.2, p.3)
    }
}

impl<T: Number> Zero for EdgeInsets<T> {
    fn zero() -> Self {
        EdgeInsets {
            top: T::zero(),
            left: T::zero(),
            bottom: T::zero(),
            right: T::zero(),
        }
    }
}

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

#[repr(C)]
pub struct FrameBuffer {
    base: *mut u8,
    len: usize,
    size: Size<isize>,
    delta: usize,
    is_portrait: bool,
}

// unsafe impl Sync for FrameBuffer {}

static BIT_MASKS: [u8; 8] = [0x80, 0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x01];

use uefi::proto::console::gop::GraphicsOutput;
impl From<&mut GraphicsOutput<'_>> for FrameBuffer {
    fn from(gop: &mut GraphicsOutput) -> Self {
        let info = gop.current_mode_info();
        let (mut width, mut height) = info.resolution();
        let mut fb = gop.frame_buffer();
        let delta = info.stride();
        let mut is_portrait = height > width;
        if is_portrait {
            // portrait
            swap(&mut width, &mut height);
        }
        if delta > width {
            // GPD micro PC fake landscape mode
            is_portrait = true;
        }
        FrameBuffer {
            base: fb.as_mut_ptr(),
            len: fb.size(),
            size: Size {
                width: width as isize,
                height: height as isize,
            },
            delta: delta,
            is_portrait: is_portrait,
        }
    }
}

impl FrameBuffer {
    #[inline]
    pub fn size(&self) -> Size<isize> {
        self.size
    }

    #[inline]
    unsafe fn get_fb(&self) -> *mut u32 {
        self.base as *mut u32
    }

    pub fn reset(&self) {
        self.fill_rect(Rect::from(self.size), Color::from(0));
    }

    pub fn fill_rect(&self, rect: Rect<isize>, color: Color) {
        let mut width = rect.size.width;
        let mut height = rect.size.height;
        let mut dx = rect.origin.x;
        let mut dy = rect.origin.y;

        {
            if dx < 0 {
                width += dx;
                dx = 0;
            }
            if dy < 0 {
                height += dy;
                dy = 0;
            }
            let r = dx + width;
            let b = dy + height;
            if r >= self.size.width {
                width = self.size.width - dx;
            }
            if b >= self.size.height {
                height = self.size.height - dy;
            }
            if width <= 0 || height <= 0 {
                return;
            }
        }

        if self.is_portrait {
            let temp = dx;
            dx = self.size.height - dy - height;
            dy = temp;
            swap(&mut width, &mut height);
        }

        unsafe {
            let mut ptr = self.get_fb().add(dx as usize + dy as usize * self.delta);
            let delta_ptr = self.delta - width as usize;
            for _y in 0..height {
                for _x in 0..width {
                    ptr.write_volatile(color.rgb);
                    ptr = ptr.add(1);
                }
                ptr = ptr.add(delta_ptr);
            }
        }
    }

    pub fn draw_pattern(&self, rect: Rect<isize>, pattern: &[u8], color: Color) {
        let width = rect.size.width;
        let mut height = rect.size.height;
        let dx = rect.origin.x;
        let mut dy = rect.origin.y;
        let w8 = (width + 7) / 8;

        let h_limit = self.size.height - dy;
        if h_limit < height {
            height = h_limit;
        }

        // TODO: more better clipping
        if dx < 0 || dx >= self.size.width || dy < 0 || dy >= self.size.height || height == 0 {
            return;
        }

        unsafe {
            if self.is_portrait {
                dy = self.size.height - dy - height;
                let mut ptr = self.get_fb().add(dy as usize + dx as usize * self.delta);
                let delta_ptr = self.delta - height as usize;

                for x in 0..w8 {
                    for mask in BIT_MASKS.iter() {
                        for y in (0..height).rev() {
                            let data = pattern[(x + y * w8) as usize];
                            if (data & mask) != 0 {
                                ptr.write_volatile(color.rgb);
                            }
                            ptr = ptr.add(1);
                        }
                        ptr = ptr.add(delta_ptr);
                    }
                }
            } else {
                let mut src_ptr = 0;
                let mut ptr = self.get_fb().add(dx as usize + dy as usize * self.delta);
                let delta_ptr = self.delta - width as usize;
                for _y in 0..height {
                    for _x in 0..w8 {
                        let data = pattern[src_ptr];
                        for mask in BIT_MASKS.iter() {
                            if (data & mask) != 0 {
                                ptr.write_volatile(color.rgb);
                            }
                            ptr = ptr.add(1);
                        }
                        src_ptr += 1;
                    }
                    ptr = ptr.add(delta_ptr);
                }
            }
        }
    }

    //pub fn blt(&self) {}
}
