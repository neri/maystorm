// Graphics Services

pub trait Number: PartialOrd + PartialEq {}

impl Number for u16 {}
impl Number for i16 {}
impl Number for u32 {}
impl Number for i32 {}
impl Number for u64 {}
impl Number for i64 {}
impl Number for usize {}
impl Number for isize {}
impl Number for f32 {}
impl Number for f64 {}

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
}

#[derive(Copy, Clone)]
pub struct Color {
    rgb: u32,
}

impl Color {
    pub fn rgb(rgb: u32) -> Self {
        Color { rgb: rgb }
    }

    pub fn components(r: u8, g: u8, b: u8) -> Self {
        Color::rgb(((r as u32) * 0x10000) + ((g as u32) * 0x100) + (b as u32))
    }
}

impl From<IndexedColor> for Color {
    fn from(index: IndexedColor) -> Self {
        Color::rgb(index.rgb())
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
}

#[repr(C)]
//#[derive(Debug, Copy, Clone)]
pub struct FrameBuffer {
    base: *mut u8,
    len: usize,
    size: Size<isize>,
    delta: usize,
    is_rotate: bool,
}

static BIT_MASKS: [u8; 8] = [0x80, 0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x01];

use uefi::proto::console::gop::GraphicsOutput;
impl From<&mut GraphicsOutput<'_>> for FrameBuffer {
    fn from(gop: &mut GraphicsOutput) -> Self {
        let info = gop.current_mode_info();
        let (mut width, mut height) = info.resolution();
        let mut fb = gop.frame_buffer();
        let delta = info.stride();
        let mut is_rotate = height > width;
        if is_rotate {
            // portrait
            core::mem::swap(&mut width, &mut height);
        }
        if delta > width {
            // GPD micro PC fake landscape mode
            is_rotate = true;
        }
        FrameBuffer {
            base: fb.as_mut_ptr(),
            len: fb.size(),
            size: Size {
                width: width as isize,
                height: height as isize,
            },
            delta: delta,
            is_rotate: is_rotate,
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
        self.fill_rect(
            &Rect::new((0, 0, self.size.width as isize, self.size.height as isize)),
            Color::rgb(0),
        );
    }

    // #[inline]
    // unsafe fn fast_write_pixel(&mut self, x: usize, y: usize, color: Color) {
    //     (self.base as *mut u32)
    //         .add(x + y * self.delta)
    //         .write_volatile(color.rgb);
    // }

    pub fn fill_rect(&self, rect: &Rect<isize>, color: Color) {
        let mut width = rect.size.width;
        let mut height = rect.size.height;
        let mut dx = rect.origin.x;
        let mut dy = rect.origin.y;

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
        if r >= self.size.width as isize {
            width = self.size.width as isize - dx;
        }
        if b >= self.size.height as isize {
            height = self.size.height as isize - dy;
        }
        if width <= 0 || height <= 0 {
            return;
        }

        if self.is_rotate {
            let temp = dx;
            dx = self.size.height as isize - dy - height;
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

    pub fn draw_pattern(&self, rect: &Rect<isize>, pattern: &[u8], color: Color) {
        let mut width = rect.size.width;
        let mut height = rect.size.height;
        let mut dx = rect.origin.x;
        let mut dy = rect.origin.y;
        let w8 = (width + 7) / 8;
        let h_limit = self.size.height as isize - dy;
        if h_limit < height {
            height = h_limit;
        }

        if dx < 0
            || dx >= self.size.width as isize
            || dy < 0
            || dy >= self.size.height as isize
            || height == 0
        {
            return;
        }

        unsafe {
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

    //ub fn blt(&self) {}
}
