// Graphics Console
use super::fonts::*;
use super::graphics::*;
use crate::kernel::io::window::*;
use alloc::boxed::Box;
use core::fmt::Write;

static DEFAULT_CONSOLE_ATTRIBUTE: u8 = 0x07;
static DEFAULT_WINDOW_ATTRIBUTE: u8 = 0xF8;

pub struct GraphicalConsole<'a> {
    handle: Option<WindowHandle>,
    fb: &'a Box<Bitmap>,
    cursor: (isize, isize),
    dims: (isize, isize),
    insets: EdgeInsets<isize>,
    is_cursor_enabled: bool,
    attribute: u8,
}

impl<'a> From<&'a Box<Bitmap>> for GraphicalConsole<'a> {
    fn from(fb: &'a Box<Bitmap>) -> Self {
        let font = FontDriver::system_font();
        let insets = EdgeInsets::padding_all(4);
        let rect = Rect::from(fb.size()).insets_by(insets);
        let cols = rect.size.width / font.width();
        let rows = rect.size.height / font.line_height();
        GraphicalConsole {
            handle: None,
            fb: fb,
            insets: insets,
            cursor: (0, 0),
            dims: (cols, rows),
            is_cursor_enabled: false,
            attribute: DEFAULT_CONSOLE_ATTRIBUTE,
        }
    }
}

impl<'a> From<WindowHandle> for GraphicalConsole<'a> {
    fn from(window: WindowHandle) -> Self {
        let bitmap = window.get_bitmap().unwrap();
        let font = FontDriver::system_font();
        let insets = window.content_insets();
        let rect = Rect::from(bitmap.size()).insets_by(insets);
        let cols = rect.size.width / font.width();
        let rows = rect.size.height / font.line_height();
        GraphicalConsole {
            handle: Some(window),
            fb: bitmap,
            insets: insets,
            cursor: (0, 0),
            dims: (cols, rows),
            is_cursor_enabled: false,
            attribute: DEFAULT_WINDOW_ATTRIBUTE,
        }
    }
}

impl GraphicalConsole<'_> {
    pub fn reset(&mut self) {
        let old_cursor_state = self.set_cursor_enabled(false);
        self.set_cursor_position(0, 0);
        self.fb.reset();
        if old_cursor_state {
            self.set_cursor_enabled(old_cursor_state);
        }
        if let Some(handle) = self.handle {
            handle.invalidate();
        }
    }

    #[inline]
    pub fn fb(&self) -> &Bitmap {
        &self.fb
    }

    #[inline]
    pub fn dims(&self) -> (isize, isize) {
        self.dims
    }

    #[inline]
    pub fn set_attribute(&mut self, attribute: u8) {
        self.attribute = attribute;
    }

    #[inline]
    pub fn set_color(&mut self, foreground: IndexedColor, background: IndexedColor) {
        self.attribute = (foreground as u8) + ((background as u8) << 4);
    }

    #[inline]
    pub fn cursor_position(&self) -> (isize, isize) {
        self.cursor
    }

    pub fn set_cursor_position(&mut self, x: isize, y: isize) {
        self.edit_cursor(move |_, _| (x, y));
    }

    #[inline]
    pub fn is_cursor_enabled(&self) -> bool {
        self.is_cursor_enabled
    }

    pub fn set_cursor_enabled(&mut self, enabled: bool) -> bool {
        let old_value = self.is_cursor_enabled;
        self.is_cursor_enabled = enabled;

        if old_value || enabled {
            let font = FontDriver::system_font();
            let cursor_height = font.line_height();
            let rect = Rect::new(
                self.insets.left + self.cursor.0 * font.width(),
                self.insets.top + (self.cursor.1 + 1) * font.line_height() - cursor_height,
                font.width(),
                cursor_height,
            );
            self.fb.fill_rect(
                rect,
                if enabled {
                    IndexedColor::from(self.attribute & 0x0F).as_color()
                } else {
                    IndexedColor::from(self.attribute >> 4).as_color()
                },
            );
            if let Some(handle) = self.handle {
                handle.invalidate_rect(rect);
            }
        }

        old_value
    }

    fn draw_char(&self, dims: (isize, isize), c: char) {
        let font = FontDriver::system_font();
        let area_rect = Rect::new(dims.0, dims.1, font.width(), font.line_height());
        let font_rect = Rect::new(dims.0, dims.1 + font.leading(), font.width(), font.height());
        let bg_color = IndexedColor::from(self.attribute >> 4).as_color();
        let fg_color = IndexedColor::from(self.attribute & 0x0F).as_color();
        self.fb.fill_rect(area_rect, bg_color);
        if let Some(glyph) = font.glyph_for(c) {
            self.fb.draw_pattern(font_rect, glyph, fg_color);
        }
        if let Some(handle) = self.handle {
            handle.invalidate_rect(area_rect);
        }
    }

    pub fn putchar(&mut self, c: char) {
        match c {
            '\x08' => {
                self.edit_cursor(|x, y| if x > 0 { (x - 1, y) } else { (x, y) });
            }
            '\n' => {
                self.edit_cursor(|_, y| (0, y + 1));
            }
            '\r' => {
                self.edit_cursor(|_, y| (0, y));
            }
            _ => {
                let old_cursor_state = self.set_cursor_enabled(false);
                let font = FontDriver::system_font();
                let (x, y) = self.adjust_cursor(self.cursor);
                self.draw_char(
                    (
                        self.insets.left + x * font.width(),
                        self.insets.top + y * font.line_height(),
                    ),
                    c,
                );
                self.cursor = self.adjust_cursor((x + 1, y));
                if old_cursor_state {
                    self.set_cursor_enabled(old_cursor_state);
                }
            }
        }
    }

    fn adjust_cursor(&self, cursor: (isize, isize)) -> (isize, isize) {
        let (mut x, mut y) = cursor;
        if x < 0 {
            x = 0;
        }
        if y < 0 {
            y = 0;
        }
        if x >= self.dims.0 {
            x = 0;
            y += 1;
        }
        if y >= self.dims.1 {
            y = self.dims.1 - 1;

            if let Some(handle) = self.handle {
                let font = FontDriver::system_font();
                let mut rect = Rect::new(
                    self.insets.left,
                    self.insets.top + font.line_height(),
                    self.dims.0 * font.width(),
                    y * font.line_height(),
                );
                let origin = Point::new(self.insets.left, self.insets.top);
                self.fb.blt(self.fb, origin, rect);

                rect.origin.y = self.insets.top + y * font.line_height();
                rect.size.height = font.line_height();
                let bg_color = IndexedColor::from(self.attribute >> 4).as_color();
                self.fb.fill_rect(rect, bg_color);

                handle.invalidate();
            }
        }
        (x, y)
    }

    #[inline]
    fn edit_cursor<F>(&mut self, f: F)
    where
        F: FnOnce(isize, isize) -> (isize, isize),
    {
        let old_cursor_state = self.set_cursor_enabled(false);
        self.cursor = self.adjust_cursor(f(self.cursor.0, self.cursor.1));
        if old_cursor_state {
            self.set_cursor_enabled(old_cursor_state);
        }
    }
}

impl Write for GraphicalConsole<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        // self.lock.lock();
        for c in s.chars() {
            self.putchar(c);
        }
        // self.lock.unlock();
        Ok(())
    }
}