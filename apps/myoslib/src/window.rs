// myos Window API

use super::*;
use megstd::drawing::*;
use myosabi::MyOsAbi;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct WindowHandle(pub usize);

pub type WindowColor = IndexedColor;

pub struct Window {
    handle: WindowHandle,
}

impl Window {
    #[inline]
    pub fn new(title: &str, size: Size) -> Self {
        let handle = WindowHandle(os_new_window1(
            title,
            size.width as usize,
            size.height as usize,
        ));
        Self { handle }
    }

    #[inline]
    pub fn close(self) {
        os_close_window(self.handle.0);
    }

    #[inline]
    pub const fn handle(&self) -> WindowHandle {
        self.handle
    }

    #[inline]
    pub fn draw_string(&self, s: &str, origin: Point, color: WindowColor) {
        os_win_draw_string(
            self.handle.0,
            origin.x as usize,
            origin.y as usize,
            s,
            color.0 as usize,
        );
    }

    #[inline]
    pub fn draw_line(&self, c1: Point, c2: Point, color: WindowColor) {
        os_win_draw_line(
            self.handle.0,
            c1.x as usize,
            c1.y as usize,
            c2.x as usize,
            c2.y as usize,
            color.0 as usize,
        )
    }

    #[inline]
    pub fn fill_rect(&self, rect: Rect, color: WindowColor) {
        os_win_fill_rect(
            self.handle.0,
            rect.x() as usize,
            rect.y() as usize,
            rect.width() as usize,
            rect.height() as usize,
            color.0 as usize,
        )
    }

    #[inline]
    pub fn blt8<'a, T: AsRef<ConstBitmap8<'a>>>(&self, bitmap: &T, origin: Point) {
        os_blt8(
            self.handle.0,
            origin.x as usize,
            origin.y as usize,
            bitmap as *const _ as usize,
        )
    }

    #[inline]
    pub fn blt32<'a, T: AsRef<ConstBitmap32<'a>>>(&self, bitmap: &T, origin: Point) {
        os_blt32(
            self.handle.0,
            origin.x as usize,
            origin.y as usize,
            bitmap as *const _ as usize,
        )
    }

    #[inline]
    pub fn wait_char(&self) -> char {
        core::char::from_u32(os_wait_char(self.handle.0)).unwrap_or('\0')
    }

    #[inline]
    pub fn read_char(&self) -> Option<char> {
        match os_read_char(self.handle.0) {
            MyOsAbi::OPTION_CHAR_NONE => None,
            c => Some(unsafe { core::char::from_u32_unchecked(c as u32) }),
        }
    }

    #[inline]
    pub fn refresh(&self) {
        os_refresh_window(self.handle.0)
    }
}

pub struct WindowBuilder {
    size: Size,
    bg_color: WindowColor,
    flag: u32,
}

impl WindowBuilder {
    #[inline]
    pub const fn new() -> Self {
        Self {
            size: Size::new(240, 240),
            bg_color: WindowColor::WHITE,
            flag: 0,
        }
    }

    /// Create a window from the specified options.
    #[inline]
    pub fn build(self, title: &str) -> Window {
        let handle = WindowHandle(os_new_window2(
            title,
            self.size.width() as usize,
            self.size.height() as usize,
            self.bg_color.0 as usize,
            self.flag as usize,
        ));
        Window { handle }
    }

    /// Set window size
    #[inline]
    pub const fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Set background color
    #[inline]
    pub const fn bg_color(mut self, bg_color: WindowColor) -> Self {
        self.bg_color = bg_color;
        self
    }

    /// Make window's bitmap to expressive (32bit)
    #[inline]
    pub const fn expressive(mut self) -> Self {
        self.flag |= MyOsAbi::WINDOW_32BIT_BITMAP;
        self
    }
}
