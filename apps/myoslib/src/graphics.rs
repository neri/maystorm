// myos Graphics Library

use core::mem::transmute;

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Color {
    argb: u32,
}

impl Color {
    pub const TRANSPARENT: Self = Self::from_argb(0);
    pub const BLACK: Self = Self::from_rgb(0x212121);
    pub const BLUE: Self = Self::from_rgb(0x0D47A1);
    pub const GREEN: Self = Self::from_rgb(0x1B5E20);
    pub const CYAN: Self = Self::from_rgb(0x006064);
    pub const RED: Self = Self::from_rgb(0xb71c1c);
    pub const MAGENTA: Self = Self::from_rgb(0x4A148C);
    pub const BROWN: Self = Self::from_rgb(0x795548);
    pub const LIGHT_GRAY: Self = Self::from_rgb(0x9E9E9E);
    pub const DARK_GRAY: Self = Self::from_rgb(0x616161);
    pub const LIGHT_BLUE: Self = Self::from_rgb(0x2196F3);
    pub const LIGHT_GREEN: Self = Self::from_rgb(0x4CAF50);
    pub const LIGHT_CYAN: Self = Self::from_rgb(0x00BCD4);
    pub const LIGHT_RED: Self = Self::from_rgb(0xf44336);
    pub const LIGHT_MAGENTA: Self = Self::from_rgb(0x9C27B0);
    pub const YELLOW: Self = Self::from_rgb(0xFFEB3B);
    pub const WHITE: Self = Self::from_rgb(0xFFFFFF);

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

#[repr(C)]
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

    pub fn line_to<F>(&self, other: Point, mut f: F)
    where
        F: FnMut(Self),
    {
        let c0 = *self;
        let c1 = other;

        let d = Point::new(
            if c1.x > c0.x {
                c1.x - c0.x
            } else {
                c0.x - c1.x
            },
            if c1.y > c0.y {
                c1.y - c0.y
            } else {
                c0.y - c1.y
            },
        );

        let s = Self::new(
            if c1.x > c0.x { 1 } else { -1 },
            if c1.y > c0.y { 1 } else { -1 },
        );

        let mut c0 = c0;
        let mut e = d.x - d.y;
        loop {
            f(c0);
            if c0.x == c1.x && c0.y == c1.y {
                break;
            }
            let e2 = e + e;
            if e2 > -d.y {
                e -= d.y;
                c0.x += s.x;
            }
            if e2 < d.x {
                e += d.x;
                c0.y += s.y;
            }
        }
    }

    #[inline]
    pub fn is_within(self, rect: Rect) -> bool {
        if let Some(coords) = Coordinates::from_rect(rect) {
            coords.left <= self.x
                && coords.right > self.x
                && coords.top <= self.y
                && coords.bottom > self.y
        } else {
            false
        }
    }
}

#[repr(C)]
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

#[repr(C)]
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

    #[inline]
    pub fn insets_by(self, insets: EdgeInsets) -> Self {
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

impl From<Size> for Rect {
    fn from(size: Size) -> Self {
        Rect {
            origin: Point::new(0, 0),
            size,
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct Coordinates {
    pub left: isize,
    pub top: isize,
    pub right: isize,
    pub bottom: isize,
}

impl Coordinates {
    pub const fn new(left: isize, top: isize, right: isize, bottom: isize) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    #[inline]
    pub fn left_top(self) -> Point {
        Point::new(self.left, self.top)
    }

    #[inline]
    pub fn right_bottom(self) -> Point {
        Point::new(self.right, self.bottom)
    }

    #[inline]
    pub fn left_bottom(self) -> Point {
        Point::new(self.left, self.bottom)
    }

    #[inline]
    pub fn right_top(self) -> Point {
        Point::new(self.right, self.top)
    }

    #[inline]
    pub fn size(self) -> Size {
        Size::new(self.right - self.left, self.bottom - self.top)
    }

    #[inline]
    pub fn from_rect(rect: Rect) -> Option<Coordinates> {
        if rect.size.width == 0 || rect.size.height == 0 {
            None
        } else {
            Some(unsafe { Self::from_rect_unchecked(rect) })
        }
    }

    #[inline]
    pub unsafe fn from_rect_unchecked(rect: Rect) -> Coordinates {
        let left: isize;
        let right: isize;
        if rect.size.width > 0 {
            left = rect.origin.x;
            right = left + rect.size.width;
        } else {
            right = rect.origin.x;
            left = right + rect.size.width;
        }

        let top: isize;
        let bottom: isize;
        if rect.size.height > 0isize {
            top = rect.origin.y;
            bottom = top + rect.size.height;
        } else {
            bottom = rect.origin.y;
            top = bottom + rect.size.height;
        }

        Self {
            left,
            top,
            right,
            bottom,
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct EdgeInsets {
    pub top: isize,
    pub left: isize,
    pub bottom: isize,
    pub right: isize,
}

impl EdgeInsets {
    #[inline]
    pub const fn new(top: isize, left: isize, bottom: isize, right: isize) -> Self {
        Self {
            top,
            left,
            bottom,
            right,
        }
    }

    #[inline]
    pub const fn padding_each(value: isize) -> Self {
        Self {
            top: value,
            left: value,
            bottom: value,
            right: value,
        }
    }
}
