// myos Window API

use super::*;
use crate::graphics::*;
use myosabi::MyOsAbi;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct WindowHandle(pub usize);

pub struct Window {
    handle: WindowHandle,
}

impl Window {
    #[inline]
    pub fn new(s: &str, size: Size) -> Self {
        let handle = WindowHandle(os_new_window(s, size.width as usize, size.height as usize));
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
    pub fn draw_text(&self, s: &str, origin: Point, color: Color) {
        os_draw_text(
            self.handle.0,
            origin.x as usize,
            origin.y as usize,
            s,
            color.argb(),
        );
    }

    #[inline]
    pub fn fill_rect(&self, rect: Rect, color: Color) {
        os_fill_rect(
            self.handle.0,
            rect.x() as usize,
            rect.y() as usize,
            rect.width() as usize,
            rect.height() as usize,
            color.argb(),
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
    pub fn flash(&self) {
        os_flash_window(self.handle.0)
    }
}
