// Graphics Console
use crate::font::*;
use crate::graphics::*;

static DEFAULT_ATTRIBUTE: u8 = 0x07;

pub struct GraphicalConsole<'a> {
    fb: &'a FrameBuffer,
    font: FontDriver<'a>,
    cursor: (isize, isize),
    dims: (isize, isize),
    is_cursor_enabled: bool,
    attribute: u8,
}

impl<'a> GraphicalConsole<'a> {
    pub fn new(fb: &'a FrameBuffer) -> Self {
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
        self.set_cursor_enabled(old_cursor_state);
    }

    pub fn set_cursor_position(&mut self, x: isize, y: isize) {
        let old_cursor_state = self.set_cursor_enabled(false);
        self.cursor = (x, y);
        self.set_cursor_enabled(old_cursor_state);
    }

    #[inline]
    pub fn cursor(&self) -> (isize, isize) {
        self.cursor
    }

    #[inline]
    pub fn dims(&self) -> (isize, isize) {
        self.dims
    }

    #[inline]
    pub fn is_cursor_enabled(&self) -> bool {
        self.is_cursor_enabled
    }

    pub fn set_cursor_enabled(&mut self, enabled: bool) -> bool {
        let old_value = self.is_cursor_enabled;
        self.is_cursor_enabled = enabled;
        old_value
    }

    #[inline]
    pub fn set_attribute(&mut self, attribute: u8) {
        self.attribute = attribute;
    }

    #[inline]
    pub fn set_color(&mut self, foreground: IndexedColor, background: IndexedColor) {
        self.attribute = (foreground as u8) + ((background as u8) << 4);
    }

    pub fn draw_char(&mut self, dims: (isize, isize), c: char) {
        let old_cursor_state = self.set_cursor_enabled(false);
        let font = &self.font;
        let font_rect = Rect::<isize>::new((dims.0, dims.1, font.width(), font.height()));
        let area_rect = Rect::<isize>::new((dims.0, dims.1, font.width(), font.line_height()));
        let bg_color = Color::from(IndexedColor::from(self.attribute >> 4));
        let fg_color = Color::from(IndexedColor::from(self.attribute & 0x0F));
        self.fb.fill_rect(&area_rect, bg_color);
        if let Some(glyph) = font.glyph_for(c) {
            self.fb.draw_pattern(&font_rect, glyph, fg_color);
        }
        self.set_cursor_enabled(old_cursor_state);
    }

    pub fn putchar(&mut self, c: char) {
        match c {
            '\x08' => {
                if self.cursor.0 > 0 {
                    self.cursor.0 -= 1;
                }
            }
            '\n' => {
                self.cursor.0 = 0;
                self.cursor.1 += 1;
            }
            '\r' => {
                self.cursor.0 = 0;
            }
            _ => {
                let font = &self.font;
                let dims = (
                    self.cursor.0 * font.width(),
                    self.cursor.1 * font.line_height(),
                );
                self.draw_char(dims, c);
                self.cursor.0 += 1;
            }
        }
    }

    pub fn print(&mut self, s: &str) {
        for c in s.chars() {
            self.putchar(c);
        }
    }
}
