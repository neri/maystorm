// Windows

use super::graphics::*;
use crate::kernel::mem::Dispose;
use crate::kernel::num::*;
use crate::*;
use alloc::boxed::Box;
use alloc::vec::*;
use bitflags::*;
use core::cmp;
use core::num::*;
use core::sync::atomic::*;

const MAX_WINDOWS: usize = 256;
const WINDOW_TITLE_LENGTH: usize = 32;

const MOUSE_POINTER_WIDTH: usize = 12;
const MOUSE_POINTER_HEIGHT: usize = 20;
const MOUSE_POINTER_PALETTE: [u32; 3] = [0x00FF00FF, 0xFFFFFFFF, 0xFF000000];
const MOUSE_POINTER_SOURCE: [[u8; MOUSE_POINTER_WIDTH]; MOUSE_POINTER_HEIGHT] = [
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 2, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0],
    [1, 2, 2, 2, 2, 1, 0, 0, 0, 0, 0, 0],
    [1, 2, 2, 2, 2, 2, 1, 0, 0, 0, 0, 0],
    [1, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0, 0],
    [1, 2, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0],
    [1, 2, 2, 2, 2, 2, 2, 2, 2, 1, 0, 0],
    [1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 0],
    [1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1],
    [1, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1],
    [1, 2, 2, 2, 1, 2, 2, 1, 0, 0, 0, 0],
    [1, 2, 2, 1, 0, 1, 2, 2, 1, 0, 0, 0],
    [1, 2, 1, 0, 0, 1, 2, 2, 1, 0, 0, 0],
    [1, 1, 0, 0, 0, 0, 1, 2, 2, 1, 0, 0],
    [0, 0, 0, 0, 0, 0, 1, 2, 2, 1, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
];

#[allow(dead_code)]
pub struct Window {
    frame: Rect<isize>,
    content_insets: EdgeInsets<isize>,
    attributes: WindowAttributes,
    style: WindowStyle,
    level: WindowLevel,
    bg_color: Color,
    bitmap: Option<Box<FrameBuffer>>,
    title: [u8; WINDOW_TITLE_LENGTH],
}

bitflags! {
    pub struct WindowStyle: u8 {
        const BORDER = 0b0000_0001;
        const TITLE = 0b0000_0010;
        const CLIENT_RECT = 0b0000_0100;
        const TRANSPARENT = 0b0000_1000;
        const PINCHABLE = 0b0001_0000;

        const DEFAULT = Self::BORDER.bits | Self::TITLE.bits;
    }
}

impl Default for WindowStyle {
    fn default() -> Self {
        Self::DEFAULT
    }
}

pub struct WindowAttributes(pub AtomicU8);

impl WindowAttributes {
    pub const EMPTY: Self = Self::new(0);
    pub const NEEDS_REDRAW: Self = Self::new(0b0000_0001);
    pub const VISIBLE: Self = Self::new(0b0000_0010);

    pub const fn new(value: u8) -> Self {
        Self(AtomicU8::new(value))
    }
}

impl Window {
    fn new() -> Self {
        Self {
            style: WindowStyle::empty(),
            attributes: WindowAttributes::EMPTY,
            frame: Rect::zero(),
            content_insets: EdgeInsets::zero(),
            level: WindowLevel::NORMAL,
            bg_color: Color::TRANSPARENT,
            bitmap: None,
            title: [0u8; WINDOW_TITLE_LENGTH],
        }
    }

    #[inline]
    pub fn frame(&self) -> Rect<isize> {
        self.frame
    }

    #[inline]
    pub fn bounds(&self) -> Rect<isize> {
        Rect::from(self.frame.size)
    }

    #[inline]
    pub fn client_rect(&self) -> Rect<isize> {
        self.bounds().insets_by(self.content_insets)
    }

    pub fn move_to(&mut self, new_origin: Point<isize>) {
        let old_frame = self.frame;
        if old_frame.origin.x != new_origin.x || old_frame.origin.y != new_origin.y {
            self.frame.origin = new_origin;
            WindowManager::shared()
                .desktop
                .unwrap()
                .borrow()
                .invalidate(old_frame);
            self.set_needs_display();
        }
    }

    fn draw(&self, rect: Rect<isize>, is_offscreen: bool) {
        let main_screen = WindowManager::shared().main_screen;
        let off_screen = WindowManager::shared().off_screen.as_ref();
        let target_screen = if is_offscreen {
            off_screen
        } else {
            main_screen
        };

        let mut rect_blt = rect;
        rect_blt.origin = self.convert_point(rect.origin);

        let window = self;
        if let Some(bitmap) = &window.bitmap {
            target_screen.blt(bitmap, rect_blt.origin, rect);
        } else {
            if window.style.contains(WindowStyle::TRANSPARENT) {
                target_screen.blend_rect(rect_blt, window.bg_color);
            } else {
                target_screen.fill_rect(rect_blt, window.bg_color);
            }
        }
        // if is_offscreen {
        //     main_screen.blt(off_screen, rect.origin, rect);
        // }
    }

    #[inline]
    pub fn set_needs_display(&self) {
        self.invalidate(self.bounds());
    }

    pub fn invalidate(&self, rect: Rect<isize>) {
        self.draw(rect, false);
    }

    #[inline]
    pub fn convert_point(&self, point: Point<isize>) -> Point<isize> {
        Point::new(self.frame.origin.x + point.x, self.frame.origin.y + point.y)
    }
}

impl Dispose for Window {
    fn dispose(&mut self) {
        self.bitmap = None;
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct WindowLevel(pub u8);

impl WindowLevel {
    pub const DESKTOP: WindowLevel = WindowLevel(0);
    pub const DESKTOP_ITEMS: WindowLevel = WindowLevel(1);
    pub const NORMAL: WindowLevel = WindowLevel(32);
    pub const HIGHER: WindowLevel = WindowLevel(64);
    pub const POPUP_BARRIER: WindowLevel = WindowLevel(96);
    pub const POPUP: WindowLevel = WindowLevel(97);
    pub const POINTER: WindowLevel = WindowLevel(127);
}

impl Default for WindowLevel {
    fn default() -> Self {
        Self::NORMAL
    }
}

#[derive(Default)]
pub struct WindowBuilder {
    pub frame: Rect<isize>,
    pub content_insets: EdgeInsets<isize>,
    pub style: WindowStyle,
    pub level: WindowLevel,
    pub bg_color: Color,
    pub bitmap: Option<Box<FrameBuffer>>,
    pub title: [u8; WINDOW_TITLE_LENGTH],
}

impl WindowBuilder {
    #[inline]
    pub fn build(self) -> Option<WindowHandle> {
        let window = Window {
            frame: self.frame,
            content_insets: self.content_insets,
            style: self.style,
            level: self.level,
            bg_color: self.bg_color,
            bitmap: self.bitmap,
            title: self.title,
            attributes: WindowAttributes::EMPTY,
        };
        Some(WindowManager::add(window))
    }
    #[inline]
    pub const fn style(mut self, style: WindowStyle) -> Self {
        self.style = style;
        self
    }
    #[inline]
    pub const fn level(mut self, level: WindowLevel) -> Self {
        self.level = level;
        self
    }
    #[inline]
    pub const fn frame(mut self, frame: Rect<isize>) -> Self {
        self.frame = frame;
        self
    }
    #[inline]
    pub const fn origin(mut self, origin: Point<isize>) -> Self {
        self.frame.origin = origin;
        self
    }
    #[inline]
    pub const fn size(mut self, size: Size<isize>) -> Self {
        self.frame.size = size;
        self
    }
    #[inline]
    pub const fn content_insets(mut self, content_insets: EdgeInsets<isize>) -> Self {
        self.content_insets = content_insets;
        self
    }
    #[inline]
    pub const fn bg_color(mut self, bg_color: Color) -> Self {
        self.bg_color = bg_color;
        self
    }
    #[inline]
    pub fn bitmap(mut self, bitmap: FrameBuffer) -> Self {
        let size = bitmap.size();
        if bitmap.is_transparent() {
            self.style.insert(WindowStyle::TRANSPARENT);
        }
        self.bitmap = Some(Box::new(bitmap));
        self.size(size)
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct WindowHandle(NonZeroUsize);

impl WindowHandle {
    pub fn new(val: usize) -> Option<Self> {
        NonZeroUsize::new(val).map(|x| Self(x))
    }

    pub const fn as_usize(self) -> usize {
        self.0.get()
    }

    const fn as_index(self) -> usize {
        self.as_usize() - 1
    }

    #[inline]
    fn using<F, R>(self, f: F) -> R
    where
        F: FnOnce(&mut Window) -> R,
    {
        let shared = WindowManager::shared();
        let window = shared.pool[self.as_index()].as_mut();
        f(window)
    }

    fn borrow(self) -> &'static Window {
        let shared = WindowManager::shared();
        shared.pool[self.as_index()].as_ref()
    }
}

static mut WM: Option<Box<WindowManager>> = None;

pub struct WindowManager {
    main_screen: &'static FrameBuffer,
    off_screen: Box<FrameBuffer>,
    screen_insets: EdgeInsets<isize>,
    lock: Spinlock,
    pool: Vec<Box<Window>>,
    desktop: Option<WindowHandle>,
    pointer: Option<WindowHandle>,
}

impl WindowManager {
    pub(crate) fn init() {
        let main_screen = stdout().fb();
        let off_screen = Box::new(FrameBuffer::with_same_size(main_screen));

        let wm = WindowManager {
            main_screen: main_screen,
            off_screen: off_screen,
            screen_insets: EdgeInsets::zero(),
            pool: Vec::with_capacity(MAX_WINDOWS),
            lock: Spinlock::new(),
            desktop: None,
            pointer: None,
        };
        unsafe {
            WM = Some(Box::new(wm));
        }
        let shared = Self::shared();

        shared.desktop = WindowBuilder::default()
            .style(WindowStyle::CLIENT_RECT)
            .level(WindowLevel::DESKTOP)
            .frame(Rect::from(main_screen.size()))
            .bg_color(Color::from_rgb(0x55AAFF))
            .build();

        let w = MOUSE_POINTER_WIDTH;
        let h = MOUSE_POINTER_HEIGHT;
        let pointer_bitmap = FrameBuffer::new(w, h, true);
        unsafe {
            let mut p = pointer_bitmap.get_fb();
            for y in 0..h {
                for x in 0..w {
                    let c = MOUSE_POINTER_PALETTE[MOUSE_POINTER_SOURCE[y][x] as usize];
                    p.write_volatile(c);
                    p = p.add(1);
                }
            }
        }
        shared.pointer = WindowBuilder::default()
            .style(WindowStyle::CLIENT_RECT)
            .level(WindowLevel::POINTER)
            .bitmap(pointer_bitmap)
            .build();

        shared.desktop.unwrap().borrow().set_needs_display();
    }

    fn shared() -> &'static mut Self {
        unsafe { WM.as_mut().unwrap() }
    }

    fn add(window: Window) -> WindowHandle {
        let shared = Self::shared();
        shared.lock.lock();
        shared.pool.push(Box::new(window));
        let len = shared.pool.len();
        shared.lock.unlock();
        WindowHandle::new(len).unwrap()
    }

    pub(crate) fn move_cursor(point: Point<isize>) {
        let shared = Self::shared();
        shared.pointer.unwrap().using(|x| {
            x.move_to(point);
        });
    }

    pub fn main_screen_bounds() -> Rect<isize> {
        let shared = Self::shared();
        shared.main_screen.bounds()
    }
}
