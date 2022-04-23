// MEG-OS Terminal

use crate::{io::tty::*, ui::font::*, ui::window::*, *};
use alloc::boxed::Box;
use core::{
    fmt::Write,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use megstd::drawing::*;

const DEFAULT_INSETS: EdgeInsets = EdgeInsets::new(0, 0, 0, 0);
const DEFAULT_ATTRIBUTE: u8 = 0x07;
const BG_ALPHA: u8 = 0xE0;
// const DEFAULT_ATTRIBUTE: u8 = 0xF8;
// const BG_ALPHA: u8 = 0xFF;

static mut TA: TerminalAgent = TerminalAgent::new();

pub struct TerminalAgent {
    n_instances: usize,
}

impl TerminalAgent {
    const fn new() -> Self {
        Self { n_instances: 0 }
    }

    fn shared<'a>() -> &'a mut Self {
        unsafe { &mut TA }
    }

    fn next_instance() -> usize {
        let shared = Self::shared();
        let r = shared.n_instances;
        shared.n_instances = r + 1;
        r
    }

    // fn console_thread(_: usize) {
    //     let shared = Self::shared();
    //     Scheduler::spawn_async(Task::new(Self::console_manager()));
    //     Scheduler::perform_tasks();
    // }

    // async fn console_manager() {
    //     loop {
    //         //
    //     }
    // }
}

pub struct Terminal {
    window: WindowHandle,
    alpha: u8,
    font: FontDescriptor,
    cols: usize,
    rows: usize,
    insets: EdgeInsets,
    x: usize,
    y: usize,
    attribute: u8,
    fg_color: Color,
    bg_color: Color,
    is_cursor_enabled: bool,
}

impl Terminal {
    pub fn with_window(
        window: WindowHandle,
        insets: Option<EdgeInsets>,
        font: FontDescriptor,
        alpha: u8,
        attribute: u8,
    ) -> Self {
        let insets = insets.unwrap_or(DEFAULT_INSETS);
        let attribute = if attribute > 0 {
            attribute
        } else {
            DEFAULT_ATTRIBUTE
        };
        let alpha = if alpha > 0 { alpha } else { BG_ALPHA };
        let (fg_color, bg_color) = Self::split_attr(attribute, alpha);

        let rect = window.content_size().bounds().insets_by(insets);
        let cols = (rect.width() / font.em_width()) as usize;
        let rows = (rect.height() / font.line_height()) as usize;

        Self {
            window,
            alpha,
            font,
            cols,
            rows,
            insets,
            x: 0,
            y: 0,
            attribute,
            fg_color,
            bg_color,
            is_cursor_enabled: true,
        }
    }

    pub fn new(cols: usize, rows: usize, font: FontDescriptor) -> Self {
        let insets = DEFAULT_INSETS;
        let attribute = DEFAULT_ATTRIBUTE;
        let alpha = BG_ALPHA;
        let (fg_color, bg_color) = Self::split_attr(attribute, alpha);

        let n_instances = TerminalAgent::next_instance();
        let screen_insets = WindowManager::screen_insets();
        let window_size = Size::new(
            font.em_width() * cols as isize,
            font.line_height() * rows as isize,
        ) + insets;

        let window = WindowBuilder::new()
            .frame(Rect::new(
                screen_insets.left + 16 + 24 * n_instances as isize,
                screen_insets.top + 16 + 24 * n_instances as isize,
                window_size.width,
                window_size.height,
            ))
            .bg_color(bg_color)
            .build("Terminal");

        Self {
            window,
            alpha,
            font,
            cols,
            rows,
            insets,
            x: 0,
            y: 0,
            attribute,
            fg_color,
            bg_color,
            is_cursor_enabled: true,
        }
    }

    fn split_attr(val: u8, alpha: u8) -> (Color, Color) {
        (
            Color::Indexed(IndexedColor(val & 0x0F)),
            Color::from(TrueColor::from(IndexedColor(val >> 4)).with_opacity(alpha)),
        )
    }

    fn scroll_up(&mut self) {
        let h = self.font.line_height();

        let frame = Rect::from(self.window.content_size()).insets_by(self.insets);
        let rect = Rect::new(0, h, frame.width(), frame.height() - h);
        let rect2 = Rect::new(0, frame.height() - h, frame.width(), h);
        self.window
            .draw_in_rect(frame, |bitmap| {
                bitmap.blt_itself(Point::default(), rect);
                bitmap.fill_rect(rect2, self.bg_color);
            })
            .unwrap();
        self.window.set_needs_display();
    }

    fn put_char(&mut self, c: char) -> Option<Rect> {
        match c {
            '\x08' => {
                if self.x > 0 {
                    self.x -= 1;
                }
                None
            }
            '\r' => {
                self.x = 0;
                None
            }
            '\n' => {
                self.x = 0;
                self.y += 1;
                None
            }
            _ => {
                let w = self.font.em_width();
                let h = self.font.line_height();

                if self.x >= self.cols {
                    self.x = 0;
                    self.y += 1;
                }
                if self.y >= self.rows {
                    self.scroll_up();
                    self.y = self.rows - 1;
                }

                let rect = Rect::new(
                    self.insets.left + self.x as isize * w,
                    self.insets.top + self.y as isize * h,
                    w,
                    h,
                );
                self.window
                    .draw_in_rect(rect, |bitmap| {
                        bitmap.fill_rect(bitmap.bounds(), self.bg_color);
                        self.font
                            .draw_char(c, bitmap, Point::default(), self.fg_color);
                    })
                    .unwrap();

                self.x += 1;
                Some(rect)
            }
        }
    }

    fn put_str(&mut self, s: &str) {
        let old_cursor = self.set_cursor_enabled(false);
        let mut coords: Option<Coordinates> = None;
        for c in s.chars() {
            self.put_char(c)
                .and_then(|v| Coordinates::from_rect(v).ok())
                .map(|c2| match &mut coords {
                    Some(v) => *v += c2,
                    None => coords = Some(c2),
                });
        }
        self.set_cursor_enabled(old_cursor);
        if let Some(v) = coords {
            self.window.invalidate_rect(v.into());
        }
    }

    fn set_needs_update_cursor(&mut self) {
        let w = self.font.em_width();
        let h = self.font.line_height();
        let dims = self.dims();
        if self.x >= dims.0 as usize || self.y >= dims.1 as usize {
            return;
        }

        let rect = Rect::new(
            self.insets.left + w * self.x as isize,
            self.insets.top + h * self.y as isize,
            w,
            h,
        );

        self.window
            .draw_in_rect(rect, |bitmap| {
                bitmap.fill_rect(
                    bitmap.bounds(),
                    if self.is_cursor_enabled {
                        self.fg_color
                    } else {
                        self.bg_color
                    },
                );
            })
            .unwrap();
        self.window.invalidate_rect(rect);
    }
}

impl Write for Terminal {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.put_str(s);
        Ok(())
    }
}

impl TtyRead for Terminal {
    fn read_async(
        &self,
    ) -> core::pin::Pin<Box<dyn core::future::Future<Output = TtyReadResult> + '_>> {
        Box::pin(ConsoleReader {
            window: self.window,
        })
    }
}

impl TtyWrite for Terminal {
    fn reset(&mut self) -> Result<(), TtyError> {
        let rect = self.window.content_size().into();
        self.window
            .draw_in_rect(rect, |bitmap| {
                bitmap.fill_rect(bitmap.bounds(), self.bg_color);
            })
            .unwrap();
        self.set_cursor_position(0, 0);
        self.window.set_needs_display();
        Ok(())
    }

    fn dims(&self) -> (isize, isize) {
        (self.cols as isize, self.rows as isize)
    }

    fn cursor_position(&self) -> (isize, isize) {
        (self.x as isize, self.y as isize)
    }

    fn set_cursor_position(&mut self, x: isize, y: isize) {
        let old_cursor = self.set_cursor_enabled(false);
        self.x = x as usize;
        self.y = y as usize;
        self.set_cursor_enabled(old_cursor);
    }

    fn is_cursor_enabled(&self) -> bool {
        self.is_cursor_enabled
    }

    fn set_cursor_enabled(&mut self, enabled: bool) -> bool {
        let r = self.is_cursor_enabled;
        self.is_cursor_enabled = enabled;
        if enabled || r {
            self.set_needs_update_cursor();
        }
        r
    }

    fn set_attribute(&mut self, attribute: u8) {
        let attribute = if attribute > 0 {
            attribute
        } else {
            DEFAULT_ATTRIBUTE
        };
        self.attribute = attribute;
        let (fg_color, bg_color) = Self::split_attr(attribute, self.alpha);
        self.fg_color = fg_color;
        self.bg_color = bg_color;
    }
}

impl Tty for Terminal {}

struct ConsoleReader {
    window: WindowHandle,
}

impl Future for ConsoleReader {
    type Output = TtyReadResult;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match self.window.poll_message(cx) {
                Poll::Ready(v) => {
                    if let Some(message) = v {
                        match message {
                            WindowMessage::Char(c) => return Poll::Ready(Ok(c)),
                            _ => self.window.handle_default_message(message),
                        }
                    }
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}
