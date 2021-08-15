// MEG-OS Window API

use super::*;
use megosabi;
use megstd::drawing::*;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct WindowHandle(pub usize);

pub type WindowColor = PackedColor;

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
    pub fn begin_draw(&self) -> DrawingContext {
        DrawingContext {
            ctx: os_begin_draw(self.handle.0),
        }
    }

    #[inline]
    pub fn draw<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&DrawingContext) -> R,
    {
        let context = self.begin_draw();
        f(&context)
    }

    #[inline]
    pub fn wait_char(&self) -> char {
        core::char::from_u32(os_wait_char(self.handle.0)).unwrap_or('\0')
    }

    #[inline]
    pub fn read_char(&self) -> Option<char> {
        match os_read_char(self.handle.0) {
            megosabi::OPTION_CHAR_NONE => None,
            c => Some(unsafe { core::char::from_u32_unchecked(c as u32) }),
        }
    }
}

pub struct DrawingContext {
    ctx: usize,
}

impl DrawingContext {
    #[inline]
    pub const unsafe fn context(&self) -> usize {
        self.ctx
    }

    #[inline]
    pub fn draw_string(&self, s: &str, origin: Point, color: WindowColor) {
        os_win_draw_string(
            self.ctx,
            origin.x as usize,
            origin.y as usize,
            s,
            color.0 as usize,
        );
    }

    #[inline]
    pub fn draw_line(&self, c1: Point, c2: Point, color: WindowColor) {
        os_win_draw_line(
            self.ctx,
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
            self.ctx,
            rect.x() as usize,
            rect.y() as usize,
            rect.width() as usize,
            rect.height() as usize,
            color.0 as usize,
        )
    }

    #[inline]
    pub fn draw_shape(
        &self,
        rect: Rect,
        radius: isize,
        bg_color: WindowColor,
        border_color: WindowColor,
    ) {
        let params = OsDrawShape {
            radius: radius as u32,
            bg_color: bg_color.0,
            border_color: border_color.0,
        };
        os_draw_shape(
            self.ctx,
            rect.x() as usize,
            rect.y() as usize,
            rect.width() as usize,
            rect.height() as usize,
            &params,
        );
    }

    #[inline]
    pub fn blt8<'a, T: AsRef<ConstBitmap8<'a>>>(&self, bitmap: &T, origin: Point) {
        os_blt8(
            self.ctx,
            origin.x as usize,
            origin.y as usize,
            bitmap as *const _ as usize,
        )
    }

    #[inline]
    pub fn blt32<'a, T: AsRef<ConstBitmap32<'a>>>(&self, bitmap: &T, origin: Point) {
        os_blt32(
            self.ctx,
            origin.x as usize,
            origin.y as usize,
            bitmap as *const _ as usize,
        )
    }
}

impl Drop for DrawingContext {
    #[inline]
    fn drop(&mut self) {
        os_end_draw(self.ctx);
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
            size: Size::new(300, 400),
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

    /// Sets the window's content bitmap to ARGB32 format.
    #[inline]
    pub const fn bitmap_argb32(mut self) -> Self {
        self.flag |= megosabi::window::USE_BITMAP32;
        self
    }

    /// Makes the border of the window a thin border.
    #[inline]
    pub const fn thin_frame(mut self) -> Self {
        self.flag |= megosabi::window::THIN_FRAME;
        self
    }

    /// Content is opaque
    #[inline]
    pub const fn opaque(mut self) -> Self {
        self.flag |= megosabi::window::OPAQUE_CONTENT;
        self
    }

    /// Set window options
    #[inline]
    pub const fn with_options(mut self, options: u32) -> Self {
        self.flag = options;
        self
    }
}
