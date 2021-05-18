//! Emergency debugging console

use super::{
    null::Null,
    tty::{Tty, TtyRead, TtyReadResult, TtyWrite},
};
use crate::system::*;
use crate::ui::font::*;
use alloc::boxed::Box;
use core::fmt;
use core::future::Future;
use core::pin::Pin;
use megstd::drawing::*;

pub struct EmConsole {
    x: usize,
    y: usize,
    fg_color: IndexedColor,
    bg_color: IndexedColor,
    font: &'static FixedFontDriver<'static>,
}

impl EmConsole {
    #[inline]
    pub const fn new(font: &'static FixedFontDriver<'static>) -> Self {
        Self {
            x: 0,
            y: 0,
            fg_color: IndexedColor::WHITE,
            bg_color: IndexedColor::BLUE,
            font,
        }
    }

    pub fn write_char(&mut self, c: char) {
        let font = self.font;
        let font_size = Size::new(font.width(), font.line_height());
        let mut bitmap = System::main_screen();
        let bitmap = &mut bitmap;

        // check bounds
        let (cols, rows) = self.dims();
        let cols = cols as usize;
        let rows = rows as usize;
        if self.x >= cols {
            self.x = 0;
            self.y += 1;
        }
        if self.y >= rows {
            self.y = rows - 1;
            let sh = font_size.height() * self.y as isize;
            let mut rect = bitmap.bounds();
            rect.origin.y += font_size.height();
            rect.size.height = sh;
            bitmap.blt_itself(Point::new(0, 0), rect);
            bitmap.fill_rect(
                Rect::new(0, sh, rect.width(), font_size.height()),
                self.bg_color.into(),
            );
        }

        match c {
            '\x08' => {
                if self.x > 0 {
                    self.x -= 1;
                }
            }
            '\r' => {
                self.x = 0;
            }
            '\n' => {
                self.x = 0;
                self.y += 1;
            }
            _ => {
                let origin = Point::new(
                    self.x as isize * font_size.width,
                    self.y as isize * font_size.height,
                );
                bitmap.fill_rect(
                    Rect {
                        origin,
                        size: font_size,
                    },
                    self.bg_color.into(),
                );
                font.draw_char(c, bitmap, origin, font.base_height(), self.fg_color.into());

                self.x += 1;
            }
        }
    }
}

impl fmt::Write for EmConsole {
    #[inline]
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.chars() {
            self.write_char(c);
        }
        Ok(())
    }
}

impl TtyWrite for EmConsole {
    fn reset(&mut self) -> Result<(), super::tty::TtyError> {
        Ok(())
    }

    fn dims(&self) -> (isize, isize) {
        let font = self.font;
        let font_size = Size::new(font.width(), font.line_height());
        let bitmap = System::main_screen();
        let cols = bitmap.width() as isize / font_size.width();
        let rows = bitmap.height() as isize / font_size.height();
        (cols, rows)
    }

    fn cursor_position(&self) -> (isize, isize) {
        (self.x as isize, self.y as isize)
    }

    fn set_cursor_position(&mut self, x: isize, y: isize) {
        self.x = x as usize;
        self.y = y as usize;
    }

    fn is_cursor_enabled(&self) -> bool {
        false
    }

    fn set_cursor_enabled(&mut self, _enabled: bool) -> bool {
        false
    }

    fn attribute(&self) -> u8 {
        0
    }

    fn set_attribute(&mut self, _attribute: u8) {
        //
    }
}

impl TtyRead for EmConsole {
    fn read_async(&self) -> Pin<Box<dyn Future<Output = TtyReadResult> + '_>> {
        Null::null().read_async()
    }
}

impl Tty for EmConsole {}
