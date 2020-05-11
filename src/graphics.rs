// Graphics Services
use crate::num::*;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Point<T: Number> {
    pub x: T,
    pub y: T,
}

impl<T: Number> Point<T> {
    pub fn new(p: (T, T)) -> Self {
        Point { x: p.0, y: p.1 }
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

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Size<T: Number> {
    pub width: T,
    pub height: T,
}

impl<T: Number> Size<T> {
    pub fn new(p: (T, T)) -> Self {
        Size {
            width: p.0,
            height: p.1,
        }
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

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Rect<T: Number> {
    pub origin: Point<T>,
    pub size: Size<T>,
}

impl<T: Number> Rect<T> {
    pub fn new(p: (T, T, T, T)) -> Self {
        Rect {
            origin: Point { x: p.0, y: p.1 },
            size: Size {
                width: p.2,
                height: p.3,
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
#[derive(Debug, Copy, Clone)]
pub struct EdgeInsets<T: Number> {
    pub top: T,
    pub left: T,
    pub bottom: T,
    pub right: T,
}

impl<T: Number> EdgeInsets<T> {
    pub fn new(p: (T, T, T, T)) -> Self {
        EdgeInsets {
            top: p.0,
            left: p.1,
            bottom: p.2,
            right: p.3,
        }
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

#[derive(Copy, Clone)]
pub struct Color {
    rgb: u32,
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Color::from(((r as u32) * 0x10000) + ((g as u32) * 0x100) + (b as u32))
    }

    pub fn components(&self) -> (u8, u8, u8) {
        let r = (self.rgb >> 16) as u8;
        let g = (self.rgb >> 8) as u8;
        let b = self.rgb as u8;
        (r, g, b)
    }
}

impl From<u32> for Color {
    fn from(rgb: u32) -> Self {
        Color { rgb: rgb }
    }
}

#[derive(Debug, Copy, Clone)]
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
    0x000000, 0x0000AA, 0x00AA00, 0x00AAAA, 0xAA0000, 0xAA00AA, 0xAA5500, 0xAAAAAA, 0x555555,
    0x5555FF, 0x55FF55, 0x55FFFF, 0xFF5555, 0xFF55FF, 0xFFFF55, 0xFFFFFF,
];

impl IndexedColor {
    pub fn rgb(&self) -> u32 {
        unsafe { SYSTEM_COLOR_PALETTE[*self as usize] }
    }

    pub fn color(&self) -> Color {
        Color::from(self.rgb())
    }
}

impl From<IndexedColor> for Color {
    fn from(index: IndexedColor) -> Self {
        Color::from(index.rgb())
    }
}

#[repr(C)]
//#[derive(Debug, Copy, Clone)]
pub struct FrameBuffer {
    base: *mut u8,
    len: usize,
    size: Size<isize>,
    delta: usize,
    is_portrait: bool,
}

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
            core::mem::swap(&mut width, &mut height);
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

    // #[inline]
    // unsafe fn fast_write_pixel(&mut self, x: usize, y: usize, color: Color) {
    //     (self.base as *mut u32)
    //         .add(x + y * self.delta)
    //         .write_volatile(color.rgb);
    // }

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
            let temp = width;
            width = height;
            height = temp;
        }

        unsafe {
            let mut ptr = self.get_fb().add(dx as usize + dy as usize * self.delta);
            let ptr_delta = self.delta - width as usize;
            for _y in 0..height {
                for _x in 0..width {
                    ptr.write_volatile(color.rgb);
                    ptr = ptr.add(1);
                }
                ptr = ptr.add(ptr_delta);
            }
        }
    }

    pub fn draw_pattern(&self, rect: Rect<isize>, pattern: &[u8], color: Color) {
        let mut width = rect.size.width;
        let mut height = rect.size.height;
        let mut dx = rect.origin.x;
        let mut dy = rect.origin.y;
        let w8 = (width + 7) / 8;

        let h_limit = self.size.height - dy;
        if h_limit < height {
            height = h_limit;
        }

        if dx < 0
            || dx >= self.size.width
            || dy < 0
            || dy >= self.size.height
            || height == 0
        {
            return;
        }

        unsafe {
            if self.is_portrait {
                dy = self.size.height - dy - height;
                let mut src_ptr = 0;
                let mut ptr = self.get_fb().add(dy as usize + dx as usize * self.delta);
                let ptr_delta = self.delta - height as usize;
        
                for x in 0..w8 {
                    for mask in BIT_MASKS.iter() {
                        for y in (0..height).rev() {
                            let data = pattern[(x + y * w8) as usize];
                            if (data & mask) != 0 {
                                ptr.write_volatile(color.rgb);
                            }
                            ptr = ptr.add(1);
                        }
                        ptr = ptr.add(ptr_delta);
                    }
                }
            } else {
                let mut src_ptr = 0;
                let mut ptr = self.get_fb().add(dx as usize + dy as usize * self.delta);
                let ptr_delta = self.delta - width as usize;
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
                    ptr = ptr.add(ptr_delta);
                }
            }
        }
    }

    //ub fn blt(&self) {}
}
