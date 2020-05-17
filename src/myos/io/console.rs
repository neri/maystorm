// Graphics Console
use super::font::*;
use super::graphics::*;
use core::fmt::Write;

static DEFAULT_ATTRIBUTE: u8 = 0x07;

pub struct GraphicalConsole<'a> {
    fb: FrameBuffer,
    font: FontDriver<'a>,
    cursor: (isize, isize),
    dims: (isize, isize),
    is_cursor_enabled: bool,
    attribute: u8,
}

impl<'a> GraphicalConsole<'a> {
    pub fn new(fb: FrameBuffer) -> Self {
        let font = FontDriver::system_font();
        let size = fb.size();
        let cols = size.width / font.width();
        let rows = size.height / font.line_height();
        GraphicalConsole {
            fb: fb,
            font: font,
            cursor: (0, 0),
            dims: (cols, rows),
            is_cursor_enabled: true,
            attribute: DEFAULT_ATTRIBUTE,
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
    }

    #[inline]
    pub fn fb(&self) -> &FrameBuffer {
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
            let font = &self.font;
            let cursor_height = 2;
            self.fb.fill_rect(
                Rect::new(
                    self.cursor.0 * font.width(),
                    (self.cursor.1 + 1) * font.line_height() - cursor_height,
                    font.width(),
                    cursor_height,
                ),
                if enabled {
                    IndexedColor::from(self.attribute & 0x0F).as_color()
                } else {
                    IndexedColor::from(self.attribute >> 4).as_color()
                },
            );
        }

        old_value
    }

    fn draw_char(&self, dims: (isize, isize), c: char) {
        let font = &self.font;
        let area_rect = Rect::new(dims.0, dims.1, font.width(), font.line_height());
        let font_rect = Rect::new(dims.0, dims.1, font.width(), font.height());
        let bg_color = IndexedColor::from(self.attribute >> 4).as_color();
        let fg_color = IndexedColor::from(self.attribute & 0x0F).as_color();
        self.fb.fill_rect(area_rect, bg_color);
        if let Some(glyph) = font.glyph_for(c) {
            self.fb.draw_pattern(font_rect, glyph, fg_color);
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
                let font = &self.font;
                let (x, y) = self.adjust_cursor(self.cursor);
                self.draw_char((x * font.width(), y * font.line_height()), c);
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
            // TODO: scroll
            y = self.dims.1 - 1;
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
        for c in s.chars() {
            self.putchar(c);
        }
        Ok(())
    }
}
