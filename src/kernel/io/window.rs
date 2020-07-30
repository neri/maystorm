// Windows

use super::fonts::*;
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

const WINDOW_BORDER_PADDING: isize = 0;
const WINDOW_BORDER_SHADOW_PADDING: isize = 8;
const WINDOW_TITLE_HEIGHT: isize = 24;
const WINDOW_BASIC_PADDING: isize = 4;
const STATUS_BAR_HEIGHT: isize = 24;

const DESKTOP_COLOR: Color = Color::from_argb(0xFF2196F3);
const STATUS_BAR_BG_COLOR: Color = Color::from_argb(0xC0EEEEEE);
const WINDOW_BORDER_COLOR: Color = Color::from_argb(0xFF777777);
const WINDOW_ACTIVE_TITLE_BG_COLOR: Color = Color::from_argb(0xC0CCCCCC);
const WINDOW_ACTIVE_TITLE_SHADOW_COLOR: Color = Color::from_argb(0x99999999);
const WINDOW_ACTIVE_TITLE_FG_COLOR: Color = Color::from_argb(0xFF000000);
const WINDOW_INACTIVE_TITLE_BG_COLOR: Color = Color::from_argb(0xFFEEEEEE);
const WINDOW_INACTIVE_TITLE_FG_COLOR: Color = Color::from_argb(0xFF999999);

// Mouse Pointer
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

// Close button
const CLOSE_BUTTON_SIZE: usize = 10;
const CLOSE_BUTTON_PALETTE: [u32; 4] = [0x00000000, 0x30000000, 0x60000000, 0x90000000];
const CLOSE_BUTTON_SOURCE: [[u8; CLOSE_BUTTON_SIZE]; CLOSE_BUTTON_SIZE] = [
    [0, 1, 0, 0, 0, 0, 0, 0, 1, 0],
    [1, 3, 2, 0, 0, 0, 0, 2, 3, 1],
    [0, 2, 3, 2, 0, 0, 2, 3, 2, 0],
    [0, 0, 2, 3, 2, 2, 3, 2, 0, 0],
    [0, 0, 0, 2, 3, 3, 2, 0, 0, 0],
    [0, 0, 0, 2, 3, 3, 2, 0, 0, 0],
    [0, 0, 2, 3, 2, 2, 3, 2, 0, 0],
    [0, 2, 3, 2, 0, 0, 2, 3, 2, 0],
    [1, 3, 2, 0, 0, 0, 0, 2, 3, 1],
    [0, 1, 0, 0, 0, 0, 0, 0, 1, 0],
];

#[allow(dead_code)]
struct Window {
    this: Option<WindowHandle>,
    next: Option<WindowHandle>,
    frame: Rect<isize>,
    shadow_insets: EdgeInsets<isize>,
    content_insets: EdgeInsets<isize>,
    attributes: WindowAttributes,
    style: WindowStyle,
    level: WindowLevel,
    bg_color: Color,
    bitmap: Option<Box<Bitmap>>,
    title: [u8; WINDOW_TITLE_LENGTH],
}

bitflags! {
    pub struct WindowStyle: u8 {
        const BORDER = 0b0000_0001;
        const TITLE = 0b0000_0010;
        const CLIENT_RECT = 0b0000_0100;
        const TRANSPARENT = 0b0000_1000;
        const PINCHABLE = 0b0001_0000;

        const DEFAULT = Self::TRANSPARENT.bits | Self::BORDER.bits | Self::TITLE.bits;
    }
}

impl WindowStyle {
    fn as_content_insets(self) -> EdgeInsets<isize> {
        let mut insets = if self.contains(Self::BORDER) {
            EdgeInsets::padding_all(WINDOW_BORDER_PADDING)
        } else {
            EdgeInsets::zero()
        };
        if self.contains(Self::TITLE) {
            insets.top += WINDOW_TITLE_HEIGHT;
        }
        if !self.contains(Self::CLIENT_RECT) {
            insets += EdgeInsets::padding_all(WINDOW_BASIC_PADDING);
        }
        insets
    }
}

struct WindowAttributes(AtomicU8);

#[allow(dead_code)]
impl WindowAttributes {
    pub const EMPTY: Self = Self::new(0);
    pub const NEEDS_REDRAW: u8 = 0b0000_0001;
    pub const VISIBLE: u8 = 0b0000_0010;

    #[inline]
    pub const fn new(value: u8) -> Self {
        Self(AtomicU8::new(value))
    }

    #[inline]
    pub fn contains(&self, value: u8) -> bool {
        (self.0.load(Ordering::Acquire) & value) == value
    }

    #[inline]
    pub fn insert(&self, value: u8) {
        self.0.fetch_or(value, Ordering::AcqRel);
    }

    #[inline]
    pub fn remove(&self, value: u8) {
        self.0.fetch_and(!value, Ordering::AcqRel);
    }
}

impl Window {
    #[inline]
    fn bounds(&self) -> Rect<isize> {
        Rect::from(self.frame.insets_by(self.shadow_insets).size)
    }

    #[inline]
    fn frame(&self) -> Rect<isize> {
        self.frame.insets_by(self.shadow_insets)
    }

    fn set_frame(&mut self, new_frame: Rect<isize>) {
        let old_frame = self.frame;
        let new_frame = Rect::new(
            new_frame.x() - self.shadow_insets.left,
            new_frame.y() - self.shadow_insets.top,
            new_frame.width() + self.shadow_insets.left + self.shadow_insets.right,
            new_frame.height() + self.shadow_insets.top + self.shadow_insets.bottom,
        );
        if old_frame != new_frame {
            self.frame = new_frame;
            if self.attributes.contains(WindowAttributes::VISIBLE) {
                WindowManager::invalidate_screen(old_frame);
                self.draw_frame();
                self.invalidate();
            }
        }
    }

    fn draw_to_screen(&self, rect: Rect<isize>, is_offscreen: bool) {
        let mut frame = rect;
        frame.origin += self.frame.origin;
        let coords1 = match Coordinates::from_rect(frame) {
            Some(coords) => coords,
            None => return,
        };

        let main_screen = WindowManager::shared().main_screen;
        let off_screen = WindowManager::shared().off_screen.as_ref();
        let target_screen = if is_offscreen {
            off_screen
        } else {
            main_screen
        };

        let mut cursor = WindowManager::shared().root.unwrap();

        loop {
            let window = cursor.borrow();
            if let Some(coords2) = Coordinates::from_rect(window.frame) {
                if frame.hit_test(window.frame) {
                    let blt_origin = Point::new(
                        cmp::max(coords1.left, coords2.left),
                        cmp::max(coords1.top, coords2.top),
                    );
                    let x = if coords1.left > coords2.left {
                        coords1.left - coords2.left
                    } else {
                        0
                    };
                    let y = if coords1.top > coords2.top {
                        coords1.top - coords2.top
                    } else {
                        0
                    };
                    let blt_rect = Rect::new(
                        x,
                        y,
                        cmp::min(coords1.right, coords2.right)
                            - cmp::max(coords1.left, coords2.left),
                        cmp::min(coords1.bottom, coords2.bottom)
                            - cmp::max(coords1.top, coords2.top),
                    );

                    if let Some(bitmap) = &window.bitmap {
                        target_screen.blt(bitmap, blt_origin, blt_rect);
                    } else {
                        if window.style.contains(WindowStyle::TRANSPARENT) {
                            target_screen.blend_rect(blt_rect, window.bg_color);
                        } else {
                            target_screen.fill_rect(blt_rect, window.bg_color);
                        }
                    }
                }
            }
            cursor = match window.next {
                Some(next) => next,
                None => break,
            };
        }
        // if is_offscreen {
        //     main_screen.blt(off_screen, rect.origin, rect);
        // }
    }

    fn title_frame(&self) -> Rect<isize> {
        if self.style.contains(WindowStyle::TITLE) {
            Rect::new(
                WINDOW_BORDER_SHADOW_PADDING + WINDOW_BORDER_PADDING,
                WINDOW_BORDER_SHADOW_PADDING + WINDOW_BORDER_PADDING,
                self.frame.width() - WINDOW_BORDER_PADDING * 2 - WINDOW_BORDER_SHADOW_PADDING * 2,
                WINDOW_TITLE_HEIGHT,
            )
        } else {
            Rect::zero()
        }
    }

    fn is_active(&self) -> bool {
        if let Some(active) = WindowManager::shared().active {
            match self.this {
                Some(this) => active == this,
                None => false,
            }
        } else {
            false
        }
    }

    fn draw_frame(&self) {
        if let Some(bitmap) = &self.bitmap {
            if self.style.contains(WindowStyle::BORDER) {
                for n in 0..WINDOW_BORDER_SHADOW_PADDING {
                    let rect = Rect::from(bitmap.size()).insets_by(EdgeInsets::padding_all(n));
                    let light = 1 + n as u8;
                    let color = Color::TRANSPARENT.set_opacity(light * light * 2);
                    bitmap.draw_rect(rect, color);
                }
            }
            if self.style.contains(WindowStyle::TITLE) {
                let shared = WindowManager::shared();
                let pad_x = 8;
                let pad_left = WINDOW_BORDER_SHADOW_PADDING + pad_x;
                let mut pad_right = WINDOW_BORDER_SHADOW_PADDING + pad_x;

                let is_active = self.is_active();

                let rect = self.title_frame();
                bitmap.fill_rect(
                    rect,
                    if is_active {
                        WINDOW_ACTIVE_TITLE_BG_COLOR
                    } else {
                        WINDOW_INACTIVE_TITLE_BG_COLOR
                    },
                );

                {
                    let close = shared.resources.close_button.as_ref().unwrap();
                    let close_pad = (rect.height() - close.height()) / 2;
                    bitmap.blt(
                        close,
                        Point::new(
                            rect.x() + rect.width() - close.width() - close_pad,
                            rect.y() + close_pad,
                        ),
                        close.bounds(),
                    );
                }
                pad_right = rect.height();

                let title_len = self.title[0] as usize;
                if title_len > 0 {
                    let font = FontDriver::system_font();
                    let text = core::str::from_utf8(&self.title[1..title_len]).unwrap();
                    let mut rect = rect;
                    let pad_y = (rect.height() - font.height()) / 2;
                    rect.origin.y += pad_y;
                    rect.size.height -= pad_y * 2;
                    rect.origin.x += pad_left;
                    rect.size.width -= pad_left + pad_right;
                    // bitmap.blend_rect(rect, Color::from_argb(0x40000000));
                    let mut rect2 = rect;
                    rect2.origin += Point::new(1, 1);
                    if is_active {
                        bitmap.draw_string(&font, rect2, WINDOW_ACTIVE_TITLE_SHADOW_COLOR, text);
                        bitmap.draw_string(&font, rect, WINDOW_ACTIVE_TITLE_FG_COLOR, text);
                    } else {
                        bitmap.draw_string(&font, rect, WINDOW_INACTIVE_TITLE_FG_COLOR, text);
                    }
                }
            }
        }
    }

    #[inline]
    fn invalidate(&self) {
        self.invalidate_rect(Rect::from(self.frame.size));
    }

    fn invalidate_rect(&self, rect: Rect<isize>) {
        if self.attributes.contains(WindowAttributes::VISIBLE) {
            self.draw_to_screen(rect, false);
        }
    }

    fn set_title_array(array: &mut [u8; WINDOW_TITLE_LENGTH], title: &str) {
        let mut i = 1;
        for c in title.chars() {
            if i >= WINDOW_TITLE_LENGTH {
                break;
            }
            let c = c as usize;
            if c < 128 {
                array[i] = c as u8;
                i += 1;
            }
        }
        array[0] = i as u8;
    }

    fn set_title(&mut self, title: &str) {
        Window::set_title_array(&mut self.title, title);
        self.draw_frame();
        self.invalidate_rect(self.title_frame());
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
    pub const ROOT: WindowLevel = WindowLevel(0);
    pub const DESKTOP_ITEMS: WindowLevel = WindowLevel(1);
    pub const NORMAL: WindowLevel = WindowLevel(32);
    pub const HIGHER: WindowLevel = WindowLevel(64);
    pub const POPUP_BARRIER: WindowLevel = WindowLevel(96);
    pub const POPUP: WindowLevel = WindowLevel(97);
    pub const POINTER: WindowLevel = WindowLevel(127);
}

pub struct WindowBuilder {
    pub frame: Rect<isize>,
    pub content_insets: EdgeInsets<isize>,
    pub style: WindowStyle,
    pub level: WindowLevel,
    pub bg_color: Color,
    pub bitmap: Option<Box<Bitmap>>,
    pub title: [u8; WINDOW_TITLE_LENGTH],
    pub no_bitmap: bool,
}

impl WindowBuilder {
    pub fn new(title: &str) -> Self {
        let window = Self {
            frame: Rect::new(100, 100, 300, 300),
            content_insets: EdgeInsets::zero(),
            level: WindowLevel::NORMAL,
            style: WindowStyle::DEFAULT,
            bg_color: Color::WHITE,
            bitmap: None,
            title: [0; WINDOW_TITLE_LENGTH],
            no_bitmap: false,
        };
        window.title(title).style(WindowStyle::DEFAULT)
    }
    #[inline]
    pub fn build(mut self) -> WindowHandle {
        let shadow_insets = if self.style.contains(WindowStyle::BORDER) {
            EdgeInsets::padding_all(WINDOW_BORDER_SHADOW_PADDING)
        } else {
            EdgeInsets::zero()
        };
        let content_insets = self.content_insets + shadow_insets;
        let mut frame = self.frame;
        if self.style.contains(WindowStyle::CLIENT_RECT) {
            frame.size.width += self.content_insets.left + self.content_insets.right;
            frame.size.height += self.content_insets.top + self.content_insets.bottom;
        }
        frame.origin -= Point::new(shadow_insets.left, shadow_insets.top);
        frame.size.width += shadow_insets.left + shadow_insets.right;
        frame.size.height += shadow_insets.top + shadow_insets.bottom;

        if !self.no_bitmap && self.bitmap.is_none() {
            let bitmap = Bitmap::new(frame.width() as usize, frame.height() as usize, true);
            bitmap.fill_rect(Rect::from(bitmap.size()), self.bg_color);
            self.bitmap = Some(Box::new(bitmap));
        }

        let attributes = if self.level == WindowLevel::ROOT {
            WindowAttributes::new(WindowAttributes::VISIBLE)
        } else {
            WindowAttributes::EMPTY
        };

        let window = Window {
            this: None,
            next: None,
            frame,
            shadow_insets,
            content_insets: content_insets,
            style: self.style,
            level: self.level,
            bg_color: self.bg_color,
            bitmap: self.bitmap,
            title: self.title,
            attributes,
        };
        window.draw_frame();
        let handle = WindowManager::add(Box::new(window));
        handle.using(|window| {
            window.this = Some(handle);
        });
        handle
    }
    #[inline]
    pub fn style(mut self, style: WindowStyle) -> Self {
        self.style = style;
        self.content_insets = style.as_content_insets();
        self
    }
    pub fn title(mut self, title: &str) -> Self {
        Window::set_title_array(&mut self.title, title);
        self
    }
    #[inline]
    const fn level(mut self, level: WindowLevel) -> Self {
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
    pub const fn bg_color(mut self, bg_color: Color) -> Self {
        self.bg_color = bg_color;
        self
    }
    #[inline]
    pub const fn no_bitmap(mut self) -> Self {
        self.no_bitmap = true;
        self
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

    // :-:-:-:-:

    pub fn set_title(self, title: &str) {
        self.using(|window| {
            window.set_title(title);
        });
    }

    #[inline]
    pub(crate) fn get_bitmap(self) -> Option<&'static Box<Bitmap>> {
        self.borrow().bitmap.as_ref()
    }

    #[inline]
    pub fn frame(self) -> Rect<isize> {
        self.borrow().frame()
    }

    pub fn set_frame(self, rect: Rect<isize>) {
        self.using(|window| {
            window.set_frame(rect);
        });
    }

    #[inline]
    pub fn bounds(self) -> Rect<isize> {
        Rect::from(self.frame().size)
    }

    #[inline]
    pub(crate) fn content_insets(self) -> EdgeInsets<isize> {
        self.borrow().content_insets
    }

    #[inline]
    pub fn move_by(self, delta: Point<isize>) {
        let mut new_rect = self.frame();
        new_rect.origin += delta;
        self.set_frame(new_rect);
    }

    #[inline]
    pub fn move_to(self, new_origin: Point<isize>) {
        let mut new_rect = self.frame();
        new_rect.origin = new_origin;
        self.set_frame(new_rect);
    }

    #[inline]
    pub fn resize_to(self, new_size: Size<isize>) {
        let mut new_rect = self.frame();
        new_rect.size = new_size;
        self.set_frame(new_rect);
    }

    pub fn show(self) {
        WindowManager::synchronized(|| unsafe {
            WindowManager::remove_hierarchy(self);
            WindowManager::add_hierarchy(self);
        });
        // self.borrow().draw_frame();
        // self.invalidate();
        WindowManager::set_active(Some(self));
    }

    pub fn hide(self) {
        let shared = WindowManager::shared();
        let frame = self.borrow().frame;
        if let Some(active) = shared.active {
            if active == self {
                WindowManager::set_active(None);
            }
        }
        WindowManager::synchronized(|| unsafe {
            WindowManager::remove_hierarchy(self);
        });
        WindowManager::invalidate_screen(frame);
    }

    pub fn invalidate_rect(self, rect: Rect<isize>) {
        self.borrow().invalidate_rect(rect);
    }

    #[inline]
    pub fn invalidate(self) {
        self.borrow().invalidate();
    }

    pub fn draw<F>(self, f: F) -> Result<(), ()>
    where
        F: FnOnce(&Bitmap) -> (),
    {
        let window = self.borrow();
        let bitmap = match window.bitmap.as_ref() {
            Some(bitmap) => bitmap,
            None => return Err(()),
        };
        let coords = match Coordinates::from_rect(
            Rect::from(window.frame.size).insets_by(window.content_insets),
        ) {
            Some(coords) => coords,
            None => return Err(()),
        };
        if coords.left > coords.right || coords.top > coords.bottom {
            return Err(());
        }

        let rect = coords.into();
        if let Some(bitmap) = bitmap.view(rect) {
            f(&bitmap);
            window.invalidate_rect(rect);
        }
        Ok(())
    }
}

static mut WM: Option<Box<WindowManager>> = None;

#[derive(Default)]
struct Resources {
    close_button: Option<Box<Bitmap>>,
}

pub struct WindowManager {
    main_screen: &'static Bitmap,
    off_screen: Box<Bitmap>,
    screen_insets: EdgeInsets<isize>,
    resources: Resources,
    lock: Spinlock,
    pool: Vec<Box<Window>>,
    root: Option<WindowHandle>,
    pointer: Option<WindowHandle>,
    active: Option<WindowHandle>,
}

impl WindowManager {
    pub(crate) fn init() {
        let main_screen = stdout().fb();
        let off_screen = Box::new(Bitmap::with_same_size(main_screen));

        let wm = WindowManager {
            main_screen: main_screen,
            off_screen: off_screen,
            screen_insets: EdgeInsets::zero(),
            resources: Resources::default(),
            pool: Vec::with_capacity(MAX_WINDOWS),
            lock: Spinlock::new(),
            root: None,
            pointer: None,
            active: None,
        };
        unsafe {
            WM = Some(Box::new(wm));
        }
        let shared = Self::shared();

        {
            let w = CLOSE_BUTTON_SIZE;
            let h = CLOSE_BUTTON_SIZE;
            let bitmap = Bitmap::new(w, h, true);
            bitmap
                .update_bitmap(|bitmap| {
                    let mut p: usize = 0;
                    for y in 0..h {
                        for x in 0..w {
                            let c = CLOSE_BUTTON_PALETTE[CLOSE_BUTTON_SOURCE[y][x] as usize];
                            bitmap[p] = Color::from_argb(c);
                            p += 1;
                        }
                    }
                })
                .unwrap();
            shared.resources.close_button = Some(Box::new(bitmap));
        };

        {
            // Desktop
            shared.root = Some(
                WindowBuilder::new("Desktop")
                    .style(WindowStyle::CLIENT_RECT)
                    .level(WindowLevel::ROOT)
                    .frame(Rect::from(main_screen.size()))
                    .bg_color(DESKTOP_COLOR)
                    .no_bitmap()
                    .build(),
            );

            shared.root.unwrap().invalidate();
        }

        {
            // Pointer
            let w = MOUSE_POINTER_WIDTH;
            let h = MOUSE_POINTER_HEIGHT;
            shared.pointer = Some(
                WindowBuilder::new("Pointer")
                    .style(WindowStyle::CLIENT_RECT)
                    .level(WindowLevel::POINTER)
                    .size(Size::new(w as isize, h as isize))
                    .origin(Point::new(
                        main_screen.width() / 2,
                        main_screen.height() / 2,
                    ))
                    .bg_color(Color::from_argb(0x80FF00FF))
                    .build(),
            );

            shared
                .pointer
                .unwrap()
                .draw(|bitmap| {
                    for y in 0..h {
                        for x in 0..w {
                            let c = Color::from_argb(
                                MOUSE_POINTER_PALETTE[MOUSE_POINTER_SOURCE[y][x] as usize],
                            );
                            bitmap.draw_pixel(Point::new(x as isize, y as isize), c);
                        }
                    }
                })
                .unwrap();

            shared.pointer.unwrap().show();
        }

        {
            // Status bar
            let window = WindowBuilder::new("Status Bar")
                .style(WindowStyle::CLIENT_RECT)
                .frame(Rect::new(0, 0, main_screen.width(), STATUS_BAR_HEIGHT))
                .bg_color(STATUS_BAR_BG_COLOR)
                .build();
            window.show();
        }
    }

    fn shared() -> &'static mut Self {
        unsafe { WM.as_mut().unwrap() }
    }

    #[inline]
    fn synchronized<F, R>(f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let shared = unsafe { WM.as_ref().unwrap() };
        shared.lock.synchronized(f)
    }

    fn add(window: Box<Window>) -> WindowHandle {
        let len = WindowManager::synchronized(|| {
            let shared = Self::shared();
            shared.pool.push(window);
            shared.pool.len()
        });
        WindowHandle::new(len).unwrap()
    }

    unsafe fn add_hierarchy(window: WindowHandle) {
        let shared = WindowManager::shared();
        let mut cursor = shared.root.unwrap();
        let level = window.borrow().level;

        loop {
            if let Some(next) = cursor.borrow().next {
                if level < next.borrow().level {
                    cursor.using(|cursor| {
                        cursor.next = Some(window);
                    });
                    window.using(|window| {
                        window.next = Some(next);
                    });
                    break;
                } else {
                    cursor = next;
                }
            } else {
                cursor.using(|cursor| {
                    cursor.next = Some(window);
                });
                break;
            }
        }
        window.borrow().attributes.insert(WindowAttributes::VISIBLE);
    }

    unsafe fn remove_hierarchy(window: WindowHandle) {
        let shared = WindowManager::shared();
        let mut cursor = shared.root.unwrap();

        window.borrow().attributes.remove(WindowAttributes::VISIBLE);
        loop {
            if let Some(next) = cursor.borrow().next {
                if next == window {
                    cursor.using(|cursor| {
                        cursor.next = window.borrow().next;
                    });
                    window.using(|window| {
                        window.next = None;
                    });
                    break;
                }
                cursor = next;
            } else {
                break;
            }
        }
    }

    pub(crate) fn move_cursor(point: Point<isize>) {
        let shared = Self::shared();
        shared.pointer.unwrap().move_to(point);
    }

    pub fn main_screen_bounds() -> Rect<isize> {
        let shared = Self::shared();
        shared.main_screen.bounds()
    }

    pub fn invalidate_screen(rect: Rect<isize>) {
        let shared = Self::shared();
        shared.root.unwrap().invalidate_rect(rect);
    }

    fn set_active(window: Option<WindowHandle>) {
        let shared = Self::shared();
        if let Some(old_active) = shared.active {
            shared.active = window;
            old_active.borrow().draw_frame();
            old_active.invalidate();
            if let Some(active) = window {
                active.borrow().draw_frame();
                active.invalidate();
            }
        } else {
            shared.active = window;
            if let Some(active) = window {
                active.borrow().draw_frame();
                active.invalidate();
            }
        }
    }
}
