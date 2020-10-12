// Graphical Console

use super::*;
use crate::io::fonts::*;
use crate::io::hid::*;
use crate::io::tty::*;
use crate::window::*;
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::fmt::Write;
use core::future::Future;
use core::num::*;
use core::pin::Pin;
use core::task::{Context, Poll};

static DEFAULT_CONSOLE_ATTRIBUTE: NonZeroU8 = unsafe { NonZeroU8::new_unchecked(0x07) };
static DEFAULT_WINDOW_ATTRIBUTE: NonZeroU8 = unsafe { NonZeroU8::new_unchecked(0x0F) };
static DEFAULT_WINDOW_OPACITY: NonZeroU8 = unsafe { NonZeroU8::new_unchecked(0xE0) };
// static DEFAULT_WINDOW_ATTRIBUTE: NonZeroU8 = unsafe { NonZeroU8::new_unchecked(0xF8) };
// static DEFAULT_WINDOW_OPACITY: NonZeroU8 = unsafe { NonZeroU8::new_unchecked(0xFF) };

static DEFAULT_CONSOLE_INSETS: EdgeInsets<isize> = EdgeInsets::padding_all(4);

pub struct GraphicalConsole<'a> {
    handle: Option<WindowHandle>,
    font: &'a FontDriver<'a>,
    bitmap: &'a Box<Bitmap>,
    cursor: (isize, isize),
    dims: (isize, isize),
    insets: EdgeInsets<isize>,
    is_cursor_enabled: bool,
    attribute: NonZeroU8,
    default_attribute: NonZeroU8,
    alpha: u8,
}

impl<'a> From<&'a Box<Bitmap>> for GraphicalConsole<'a> {
    fn from(bitmap: &'a Box<Bitmap>) -> Self {
        let font = FontDriver::system_font_static();
        let insets = DEFAULT_CONSOLE_INSETS;
        let rect = Rect::from(bitmap.size()).insets_by(insets);
        let cols = rect.size.width / font.width();
        let rows = rect.size.height / font.line_height();
        GraphicalConsole {
            handle: None,
            font,
            bitmap,
            insets,
            cursor: (0, 0),
            dims: (cols, rows),
            is_cursor_enabled: false,
            attribute: DEFAULT_CONSOLE_ATTRIBUTE,
            default_attribute: DEFAULT_CONSOLE_ATTRIBUTE,
            alpha: u8::MAX,
        }
    }
}

impl<'a> GraphicalConsole<'a> {
    pub fn new(
        title: &str,
        dims: (isize, isize),
        font: &'a FontDriver<'a>,
        attribute: u8,
        alpha: u8,
    ) -> (Box<GraphicalConsole<'a>>, WindowHandle) {
        let size = Size::new(font.width() * dims.0, font.line_height() * dims.1);
        let window = WindowBuilder::new(title)
            .style_add(WindowStyle::NAKED)
            .size(size + DEFAULT_CONSOLE_INSETS)
            .build();

        let bitmap = window.bitmap().unwrap();
        let insets = window.content_insets() + DEFAULT_CONSOLE_INSETS;
        let rect = Rect::from(bitmap.size()).insets_by(insets);
        let cols = rect.size.width / font.width();
        let rows = rect.size.height / font.line_height();
        let attribute = NonZeroU8::new(attribute).unwrap_or(DEFAULT_WINDOW_ATTRIBUTE);
        let console = Box::new(GraphicalConsole {
            handle: Some(window),
            font,
            bitmap,
            insets,
            cursor: (0, 0),
            dims: (cols, rows),
            is_cursor_enabled: false,
            attribute,
            default_attribute: attribute,
            alpha: NonZeroU8::new(alpha)
                .unwrap_or(DEFAULT_WINDOW_OPACITY)
                .get(),
        });
        window.set_bg_color(console.bg_color());
        (console, window)
    }
}

impl GraphicalConsole<'_> {
    #[inline]
    pub fn window(&self) -> Option<WindowHandle> {
        self.handle
    }

    #[inline]
    fn fg_color(&self) -> Color {
        IndexedColor::from(self.attribute.get() & 0x0F).as_color()
    }

    #[inline]
    fn bg_color(&self) -> Color {
        IndexedColor::from(self.attribute.get() >> 4)
            .as_color()
            .set_opacity(self.alpha)
    }

    #[inline]
    fn draw_char(&self, dims: (isize, isize), c: char) {
        let font = self.font;
        let rect = Rect::new(dims.0, dims.1, font.width(), font.line_height());
        self.bitmap.fill_rect(rect, self.bg_color());
        font.draw_char(c, self.bitmap, rect.origin, self.fg_color());
        if let Some(handle) = self.handle {
            handle.invalidate_rect(rect);
        }
    }

    pub fn putchar(&mut self, c: char) {
        match c {
            '\x08' => {
                self.update_cursor(|x, y| if x > 0 { (x - 1, y) } else { (x, y) });
            }
            '\n' => {
                self.update_cursor(|_, y| (0, y + 1));
            }
            '\r' => {
                self.update_cursor(|_, y| (0, y));
            }
            _ => {
                let old_cursor_state = self.set_cursor_enabled(false);
                let font = self.font;
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
                let font = self.font;
                let mut rect = Rect::new(
                    self.insets.left,
                    self.insets.top + font.line_height(),
                    self.dims.0 * font.width(),
                    y * font.line_height(),
                );
                let origin = Point::new(self.insets.left, self.insets.top);
                self.bitmap.blt(self.bitmap, origin, rect, BltOption::COPY);

                rect.origin.y = self.insets.top + y * font.line_height();
                rect.size.height = font.line_height();
                self.bitmap.fill_rect(rect, self.bg_color());

                handle.invalidate();
            }
        }
        (x, y)
    }

    #[inline]
    fn update_cursor<F>(&mut self, f: F)
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
    fn write_char(&mut self, c: char) -> core::fmt::Result {
        self.putchar(c);
        Ok(())
    }

    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.chars() {
            self.putchar(c);
        }
        Ok(())
    }
}

impl TtyWrite for GraphicalConsole<'_> {
    fn reset(&mut self) -> Result<(), TtyError> {
        let old_cursor_state = self.set_cursor_enabled(false);
        self.set_cursor_position(0, 0);

        if let Some(window) = self.window() {
            window
                .draw(|bitmap| {
                    bitmap.fill_rect(bitmap.bounds(), self.bg_color());
                })
                .unwrap();
        } else {
            self.bitmap.reset();
        }

        if old_cursor_state {
            self.set_cursor_enabled(old_cursor_state);
        }

        Ok(())
    }

    #[inline]
    fn dims(&self) -> (isize, isize) {
        self.dims
    }

    #[inline]
    fn cursor_position(&self) -> (isize, isize) {
        self.cursor
    }

    #[inline]
    fn set_cursor_position(&mut self, x: isize, y: isize) {
        self.update_cursor(move |_, _| (x, y));
    }

    #[inline]
    fn is_cursor_enabled(&self) -> bool {
        self.is_cursor_enabled
    }

    fn set_cursor_enabled(&mut self, enabled: bool) -> bool {
        let old_value = self.is_cursor_enabled;
        self.is_cursor_enabled = enabled;

        if old_value || enabled {
            let font = self.font;
            let cursor_height = font.line_height() / 8;
            let rect = Rect::new(
                self.insets.left + self.cursor.0 * font.width(),
                self.insets.top + (self.cursor.1 + 1) * font.line_height() - cursor_height,
                font.width(),
                cursor_height,
            );
            self.bitmap.fill_rect(
                rect,
                if enabled {
                    self.fg_color()
                } else {
                    self.bg_color()
                },
            );
            if let Some(handle) = self.handle {
                handle.invalidate_rect(rect);
            }
        }

        old_value
    }

    #[inline]
    fn attribute(&self) -> u8 {
        self.attribute.get()
    }

    #[inline]
    fn set_attribute(&mut self, attribute: u8) {
        self.attribute = NonZeroU8::new(attribute).unwrap_or(self.default_attribute);
    }
}

impl TtyRead for GraphicalConsole<'_> {
    fn read_async(&self) -> Pin<Box<dyn Future<Output = TtyReadResult> + '_>> {
        Box::pin(VtReader {
            _vt: Arc::new(self),
        })
    }
}

impl Tty for GraphicalConsole<'_> {}

struct VtReader<'a> {
    _vt: Arc<&'a GraphicalConsole<'a>>,
}

impl Future for VtReader<'_> {
    type Output = TtyReadResult;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        match HidManager::get_key() {
            None => Poll::Pending,
            Some(e) => match e.key_data() {
                Some(key) => Poll::Ready(Ok(key.into())),
                None => Poll::Ready(Err(TtyError::SkipData)),
            },
        }
    }
}
