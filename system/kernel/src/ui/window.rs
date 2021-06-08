//! A Window System

use super::font::*;
use crate::{
    io::hid::*, sync::atomicflags::*, sync::semaphore::*, sync::spinlock::Spinlock, sync::Mutex,
    task::scheduler::*, util::text::*, *,
};
use alloc::{boxed::Box, collections::btree_map::BTreeMap, sync::Arc};
use bitflags::*;
use core::{
    cell::UnsafeCell,
    cmp,
    future::Future,
    mem::swap,
    num::*,
    pin::Pin,
    sync::atomic::*,
    task::{Context, Poll},
    time::Duration,
};
use crossbeam_queue::ArrayQueue;
use futures_util::task::AtomicWaker;
use megstd::drawing::*;

const WINDOW_SYSTEM_EVENT_QUEUE_SIZE: usize = 100;

const WINDOW_TITLE_LENGTH: usize = 32;

const WINDOW_BORDER_PADDING: isize = 0;
const WINDOW_BORDER_SHADOW_PADDING: isize = 8;
const WINDOW_TITLE_HEIGHT: isize = 24;

// const BARRIER_COLOR: TrueColor = TrueColor::from_argb(0x80000000);
const WINDOW_ACTIVE_TITLE_BG_COLOR: SomeColor = SomeColor::from_argb(0xE0BBDEFB);
const WINDOW_ACTIVE_TITLE_FG_COLOR: SomeColor = SomeColor::from_argb(0xFF212121);
const WINDOW_INACTIVE_TITLE_BG_COLOR: SomeColor = SomeColor::from_argb(0xFFEEEEEE);
const WINDOW_INACTIVE_TITLE_FG_COLOR: SomeColor = SomeColor::from_argb(0xFF9E9E9E);

// Mouse Pointer
const MOUSE_POINTER_WIDTH: usize = 12;
const MOUSE_POINTER_HEIGHT: usize = 20;
const MOUSE_POINTER_SOURCE: [u8; MOUSE_POINTER_WIDTH * MOUSE_POINTER_HEIGHT] = [
    0x0F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F, 0x0F, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F, 0x07, 0x0F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0x0F, 0x00, 0x07, 0x0F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0x0F, 0x00, 0x00, 0x07, 0x0F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F, 0x00, 0x00, 0x00,
    0x07, 0x0F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F, 0x00, 0x00, 0x00, 0x00, 0x07, 0x0F, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0x0F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07, 0x0F, 0xFF, 0xFF, 0xFF, 0xFF,
    0x0F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07, 0x0F, 0xFF, 0xFF, 0xFF, 0x0F, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x07, 0x0F, 0xFF, 0xFF, 0x0F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x07, 0x0F, 0xFF, 0x0F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07, 0x0F,
    0x0F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x00, 0x00, 0x07,
    0x0F, 0x07, 0x00, 0x0F, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F, 0x00, 0x07, 0x0F, 0xFF, 0x0F, 0x00, 0x07,
    0x0F, 0xFF, 0xFF, 0xFF, 0x0F, 0x07, 0x0F, 0xFF, 0xFF, 0x0F, 0x07, 0x00, 0x0F, 0xFF, 0xFF, 0xFF,
    0x0F, 0x0F, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F, 0x00, 0x07, 0x0F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0x0F, 0x07, 0x00, 0x0F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F,
    0x0F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
];

// Close button
// const CLOSE_BUTTON_SIZE: usize = 10;
// const CLOSE_BUTTON_PALETTE: [u32; 4] = [0x00000000, 0x40000000, 0x80000000, 0xC0000000];
// const CLOSE_BUTTON_SOURCE: [[u8; CLOSE_BUTTON_SIZE]; CLOSE_BUTTON_SIZE] = [
//     [0, 1, 0, 0, 0, 0, 0, 0, 1, 0],
//     [1, 3, 2, 0, 0, 0, 0, 2, 3, 1],
//     [0, 2, 3, 2, 0, 0, 2, 3, 2, 0],
//     [0, 0, 2, 3, 2, 2, 3, 2, 0, 0],
//     [0, 0, 0, 2, 3, 3, 2, 0, 0, 0],
//     [0, 0, 0, 2, 3, 3, 2, 0, 0, 0],
//     [0, 0, 2, 3, 2, 2, 3, 2, 0, 0],
//     [0, 2, 3, 2, 0, 0, 2, 3, 2, 0],
//     [1, 3, 2, 0, 0, 0, 0, 2, 3, 1],
//     [0, 1, 0, 0, 0, 0, 0, 0, 1, 0],
// ];

static mut WM: Option<Box<WindowManager<'static>>> = None;

pub struct WindowManager<'a> {
    lock: Spinlock,

    sem_event: Semaphore,
    attributes: AtomicBitflags<WindowManagerAttributes>,
    system_event: ArrayQueue<WindowSystemEvent>,

    pointer_x: AtomicIsize,
    pointer_y: AtomicIsize,
    buttons: AtomicUsize,
    buttons_down: AtomicUsize,
    buttons_up: AtomicUsize,

    main_screen: Bitmap32<'a>,
    screen_size: Size,
    off_screen: BoxedBitmap32<'a>,
    screen_insets: EdgeInsets,

    resources: Resources<'a>,

    window_pool: Mutex<BTreeMap<WindowHandle, Arc<UnsafeCell<Box<RawWindow<'a>>>>>>,

    root: WindowHandle,
    pointer: WindowHandle,
    active: Option<WindowHandle>,
    captured: Option<WindowHandle>,
    captured_origin: Point,
    entered: Option<WindowHandle>,
}

#[allow(dead_code)]
struct Resources<'a> {
    corner_shadow: BoxedBitmap32<'a>,
    title_font: FontDescriptor,
    label_font: FontDescriptor,
}

impl WindowManager<'static> {
    pub(crate) fn init(main_screen: Bitmap32<'static>) {
        let attributes = AtomicBitflags::EMPTY;

        let mut screen_size = main_screen.size();
        if screen_size.width < screen_size.height {
            attributes.insert(WindowManagerAttributes::PORTRAIT);
            swap(&mut screen_size.width, &mut screen_size.height);
        }

        let pointer_x = screen_size.width() / 2;
        let pointer_y = screen_size.height() / 2;
        let off_screen = BoxedBitmap32::new(screen_size, TrueColor::TRANSPARENT);
        let mut window_pool = BTreeMap::new();

        let corner_shadow = {
            let w = WINDOW_BORDER_SHADOW_PADDING;
            let h = WINDOW_BORDER_SHADOW_PADDING;
            let mut bitmap = BoxedBitmap32::new(Size::new(w * 2, h * 2), TrueColor::TRANSPARENT);
            bitmap.draw(|bitmap| {
                let center = bitmap.bounds().center();
                for q in 0..WINDOW_BORDER_SHADOW_PADDING {
                    let r = WINDOW_BORDER_SHADOW_PADDING - q;
                    let density = ((q + 1) * (q + 1)) as u8;
                    bitmap.fill_circle(center, r, TrueColor::gray(0, density));
                }
            });
            bitmap
        };

        let root = {
            let window = WindowBuilder::new("Root")
                .style(WindowStyle::NAKED | WindowStyle::OPAQUE)
                .level(WindowLevel::ROOT)
                .frame(Rect::from(screen_size))
                .bg_color(SomeColor::BLACK)
                .without_message_queue()
                .bitmap_strategy(BitmapStrategy::NonBitmap)
                .build_inner();

            let handle = window.handle;
            window_pool.insert(handle, Arc::new(UnsafeCell::new(window)));
            handle
        };

        let pointer = {
            let pointer_size =
                Size::new(MOUSE_POINTER_WIDTH as isize, MOUSE_POINTER_HEIGHT as isize);
            let window = WindowBuilder::new("Root")
                .style(WindowStyle::NAKED)
                .level(WindowLevel::POINTER)
                .origin(Point::new(pointer_x, pointer_y))
                .size(pointer_size)
                .without_message_queue()
                .build_inner();

            window
                .draw_in_rect(pointer_size.into(), |bitmap| {
                    let cursor = ConstBitmap8::from_bytes(&MOUSE_POINTER_SOURCE, pointer_size);
                    bitmap.blt(&cursor, Point::new(0, 0), pointer_size.into())
                })
                .unwrap();

            let handle = window.handle;
            window_pool.insert(handle, Arc::new(UnsafeCell::new(window)));
            handle
        };

        unsafe {
            WM = Some(Box::new(WindowManager {
                lock: Spinlock::default(),
                sem_event: Semaphore::new(0),
                attributes,
                pointer_x: AtomicIsize::new(pointer_x),
                pointer_y: AtomicIsize::new(pointer_y),
                buttons: AtomicUsize::new(0),
                buttons_down: AtomicUsize::new(0),
                buttons_up: AtomicUsize::new(0),
                main_screen,
                screen_size,
                off_screen,
                screen_insets: EdgeInsets::default(),
                resources: Resources {
                    corner_shadow,
                    title_font: FontManager::title_font(),
                    label_font: FontManager::ui_font(),
                },
                window_pool: Mutex::new(window_pool),
                root,
                pointer,
                active: None,
                captured: None,
                captured_origin: Point::default(),
                entered: None,
                system_event: ArrayQueue::new(WINDOW_SYSTEM_EVENT_QUEUE_SIZE),
            }));
        }

        SpawnOption::with_priority(Priority::High).start_process(
            Self::window_thread,
            0,
            "Window Manager",
        );
    }

    #[track_caller]
    fn add(window: Box<RawWindow<'static>>) {
        let handle = window.handle;
        WindowManager::shared_mut()
            .window_pool
            .lock()
            .unwrap()
            .insert(handle, Arc::new(UnsafeCell::new(window)));
    }

    #[allow(dead_code)]
    fn remove(_window: &WindowHandle) {
        // TODO:
    }

    #[inline]
    fn get<'a>(&self, key: &WindowHandle) -> Option<&'a Box<RawWindow<'static>>> {
        match WindowManager::shared().window_pool.lock() {
            Ok(v) => v
                .get(key)
                .map(|v| v.clone().get())
                .map(|v| unsafe { &(*v) }),
            Err(_) => None,
        }
    }

    fn get_mut<F, R>(&mut self, key: &WindowHandle, f: F) -> Option<R>
    where
        F: FnOnce(&mut RawWindow) -> R,
    {
        let window = match WindowManager::shared_mut().window_pool.lock() {
            Ok(mut v) => v.get_mut(key).map(|v| v.clone()),
            Err(_) => None,
        };
        window.map(|window| unsafe {
            let window = window.get();
            f(&mut *window)
        })
    }
}

impl WindowManager<'_> {
    pub const DEFAULT_BGCOLOR: SomeColor = SomeColor::WHITE;

    #[inline]
    #[track_caller]
    fn shared<'a>() -> &'a WindowManager<'static> {
        unsafe { WM.as_ref().unwrap() }
    }

    #[inline]
    #[track_caller]
    fn shared_mut<'a>() -> &'a mut WindowManager<'static> {
        unsafe { WM.as_mut().unwrap() }
    }

    #[inline]
    fn shared_opt<'a>() -> Option<&'a Box<WindowManager<'static>>> {
        unsafe { WM.as_ref() }
    }

    /// Window Manager's Thread
    fn window_thread(_: usize) {
        let shared = WindowManager::shared_mut();

        loop {
            shared.sem_event.wait();

            if shared
                .attributes
                .test_and_clear(WindowManagerAttributes::NEEDS_REDRAW)
            {
                let desktop = shared.root;
                desktop.as_ref().draw_to_screen(desktop.frame());
            }
            if shared
                .attributes
                .test_and_clear(WindowManagerAttributes::EVENT)
            {
                while let Some(event) = shared.system_event.pop() {
                    match event {
                        WindowSystemEvent::Key(w, e) => {
                            let _ = w.post(WindowMessage::Key(e));
                        }
                    }
                }
            }
            if shared
                .attributes
                .test_and_clear(WindowManagerAttributes::MOUSE_MOVE)
            {
                let position = shared.pointer();
                let current_buttons =
                    MouseButton::from_bits_truncate(shared.buttons.load(Ordering::Acquire) as u8);
                let buttons_down = MouseButton::from_bits_truncate(
                    shared.buttons_down.swap(0, Ordering::SeqCst) as u8,
                );
                let buttons_up = MouseButton::from_bits_truncate(
                    shared.buttons_up.swap(0, Ordering::SeqCst) as u8,
                );

                if let Some(captured) = shared.captured {
                    if current_buttons.contains(MouseButton::LEFT) {
                        if shared.attributes.contains(WindowManagerAttributes::MOVING) {
                            let top = if captured.as_ref().level < WindowLevel::FLOATING {
                                shared.screen_insets.top
                            } else {
                                0
                            };
                            let x = position.x - shared.captured_origin.x;
                            let y = cmp::max(position.y - shared.captured_origin.y, top);
                            captured.move_to(Point::new(x, y));
                        } else {
                            let _ = Self::make_mouse_events(
                                captured,
                                position,
                                current_buttons,
                                buttons_down,
                                buttons_up,
                            );
                        }
                    } else {
                        let _ = Self::make_mouse_events(
                            captured,
                            position,
                            current_buttons,
                            buttons_down,
                            buttons_up,
                        );
                        shared.captured = None;
                        shared.attributes.remove(WindowManagerAttributes::MOVING);

                        let target = Self::window_at_point(position);
                        if let Some(entered) = shared.entered {
                            if entered != target {
                                let _ = Self::make_mouse_events(
                                    captured,
                                    position,
                                    current_buttons,
                                    MouseButton::empty(),
                                    MouseButton::empty(),
                                );
                                let _ = entered.post(WindowMessage::MouseLeave);
                                shared.entered = Some(target);
                                let _ = target.post(WindowMessage::MouseEnter);
                            }
                        }
                    }
                } else {
                    let target = Self::window_at_point(position);

                    if buttons_down.contains(MouseButton::LEFT) {
                        if let Some(active) = shared.active {
                            if active != target {
                                WindowManager::set_active(Some(target));
                            }
                        } else {
                            WindowManager::set_active(Some(target));
                        }
                        let target_window = target.as_ref();
                        if target_window.style.contains(WindowStyle::PINCHABLE) {
                            shared.attributes.insert(WindowManagerAttributes::MOVING);
                        } else {
                            let mut title_frame = target_window.title_frame();
                            title_frame.origin += target_window.frame.origin;
                            if position.is_within(title_frame) {
                                shared.attributes.insert(WindowManagerAttributes::MOVING);
                            } else {
                                let _ = Self::make_mouse_events(
                                    target,
                                    position,
                                    current_buttons,
                                    buttons_down,
                                    buttons_up,
                                );
                            }
                        }
                        shared.captured = Some(target);
                        shared.captured_origin = position - target_window.visible_frame().origin;
                    } else {
                        let _ = Self::make_mouse_events(
                            target,
                            position,
                            current_buttons,
                            buttons_down,
                            buttons_up,
                        );
                    }

                    if let Some(entered) = shared.entered {
                        if entered != target {
                            let _ = entered.post(WindowMessage::MouseLeave);
                            shared.entered = Some(target);
                            let _ = target.post(WindowMessage::MouseEnter);
                        }
                    }
                }

                shared.pointer.move_to(position);
            }
        }
    }

    fn post_system_event(event: WindowSystemEvent) -> Result<(), WindowSystemEvent> {
        let shared = Self::shared();
        let r = shared.system_event.push(event);
        shared.attributes.insert(WindowManagerAttributes::EVENT);
        shared.sem_event.signal();
        r
    }

    fn make_mouse_events(
        target: WindowHandle,
        position: Point,
        buttons: MouseButton,
        down: MouseButton,
        up: MouseButton,
    ) -> Result<(), WindowPostError> {
        let window = target.as_ref();
        let origin = window.frame.insets_by(window.content_insets).origin;
        let point = Point::new(position.x - origin.x, position.y - origin.y);

        if down.is_empty() && up.is_empty() {
            return target.post(WindowMessage::MouseMove(MouseEvent::new(
                point,
                buttons,
                MouseButton::empty(),
            )));
        }
        let mut errors = None;
        if !down.is_empty() {
            match target.post(WindowMessage::MouseDown(MouseEvent::new(
                point, buttons, down,
            ))) {
                Ok(_) => (),
                Err(err) => errors = Some(err),
            };
        }
        if !up.is_empty() {
            match target.post(WindowMessage::MouseUp(MouseEvent::new(point, buttons, up))) {
                Ok(_) => (),
                Err(err) => errors = Some(err),
            };
        }
        match errors {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    #[inline]
    pub fn is_enabled() -> bool {
        unsafe { WM.is_some() }
    }

    #[inline]
    fn synchronized<F, R>(f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let shared = unsafe { WM.as_ref().unwrap() };
        shared.lock.synchronized(f)
    }

    #[inline]
    fn next_window_handle() -> WindowHandle {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
        WindowHandle::new(NEXT_ID.fetch_add(1, Ordering::SeqCst)).unwrap()
    }

    unsafe fn add_hierarchy(window: WindowHandle) {
        WindowManager::remove_hierarchy(window);

        let shared = WindowManager::shared();
        let mut cursor = shared.root;
        let level = window.as_ref().level;

        loop {
            if let Some(next) = cursor.as_ref().next {
                if level < next.as_ref().level {
                    cursor.update(|cursor| {
                        cursor.next = Some(window);
                    });
                    window.update(|window| {
                        window.next = Some(next);
                    });
                    break;
                } else {
                    cursor = next;
                }
            } else {
                cursor.update(|cursor| {
                    cursor.next = Some(window);
                });
                break;
            }
        }
        window.as_ref().attributes.insert(WindowAttributes::VISIBLE);
    }

    unsafe fn remove_hierarchy(window: WindowHandle) {
        let shared = WindowManager::shared();
        let mut cursor = shared.root;

        window.as_ref().attributes.remove(WindowAttributes::VISIBLE);
        loop {
            if let Some(next) = cursor.as_ref().next {
                if next == window {
                    cursor.update(|cursor| {
                        cursor.next = window.as_ref().next;
                    });
                    window.update(|window| {
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

    #[inline]
    pub fn main_screen_bounds() -> Rect {
        let shared = Self::shared();
        shared.screen_size.into()
    }

    #[inline]
    pub fn user_screen_bounds() -> Rect {
        match WindowManager::shared_opt() {
            Some(shared) => Rect::from(shared.screen_size).insets_by(shared.screen_insets),
            None => System::main_screen().size().into(),
        }
    }

    #[inline]
    pub fn screen_insets() -> EdgeInsets {
        let shared = Self::shared();
        shared.screen_insets
    }

    #[inline]
    pub fn add_screen_insets(insets: EdgeInsets) {
        let shared = Self::shared_mut();
        shared.screen_insets += insets;
    }

    #[inline]
    pub fn invalidate_screen(rect: Rect) {
        let shared = Self::shared();
        shared.root.invalidate_rect(rect);
    }

    fn set_active(window: Option<WindowHandle>) {
        let shared = WindowManager::shared_mut();
        if let Some(old_active) = shared.active {
            let _ = old_active.post(WindowMessage::Deactivated);
            shared.active = window;
            let _ = old_active.update_opt(|window| window.refresh_title());
        } else {
            shared.active = window;
        }
        if let Some(active) = window {
            let _ = active.post(WindowMessage::Activated);
            active.show();
        }
    }

    fn window_at_point(point: Point) -> WindowHandle {
        WindowManager::synchronized(|| {
            let shared = Self::shared();
            let mut found = shared.root;
            let mut cursor = found;
            loop {
                let window = cursor.as_ref();
                if window.level == WindowLevel::POINTER {
                    break found;
                }
                if point.is_within(window.frame.insets_by(window.shadow_insets)) {
                    found = cursor;
                }
                match window.next {
                    Some(next) => cursor = next,
                    None => break found,
                };
            }
        })
    }

    fn pointer(&self) -> Point {
        Point::new(
            self.pointer_x.load(Ordering::Relaxed),
            self.pointer_y.load(Ordering::Relaxed),
        )
    }

    fn update_coord(
        coord: &AtomicIsize,
        movement: isize,
        min_value: isize,
        max_value: isize,
    ) -> bool {
        match coord.fetch_update(Ordering::SeqCst, Ordering::Relaxed, |value| {
            let new_value = cmp::min(cmp::max(value + movement, min_value), max_value);
            if value == new_value {
                None
            } else {
                Some(new_value)
            }
        }) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    pub fn post_mouse_event(mouse_state: &mut MouseState) {
        let shared = match Self::shared_opt() {
            Some(v) => v,
            None => return,
        };
        let screen_bounds: Rect = shared.screen_size.into();

        let mut pointer = Point::new(0, 0);
        core::mem::swap(&mut mouse_state.x, &mut pointer.x);
        core::mem::swap(&mut mouse_state.y, &mut pointer.y);
        let button_changes = mouse_state.current_buttons ^ mouse_state.prev_buttons;
        let button_down = button_changes & mouse_state.current_buttons;
        let button_up = button_changes & mouse_state.prev_buttons;
        let button_changed = !button_changes.is_empty();

        if button_changed {
            shared.buttons.store(
                mouse_state.current_buttons.bits() as usize,
                Ordering::SeqCst,
            );
            shared
                .buttons_down
                .fetch_or(button_down.bits() as usize, Ordering::SeqCst);
            shared
                .buttons_up
                .fetch_or(button_up.bits() as usize, Ordering::SeqCst);
        }

        let moved = Self::update_coord(
            &shared.pointer_x,
            pointer.x,
            screen_bounds.x(),
            screen_bounds.width() - 1,
        ) | Self::update_coord(
            &shared.pointer_y,
            pointer.y,
            screen_bounds.y(),
            screen_bounds.height() - 1,
        );

        if button_changed | moved {
            shared
                .attributes
                .insert(WindowManagerAttributes::MOUSE_MOVE);
            shared.sem_event.signal();
        }
    }

    pub fn post_key_event(event: KeyEvent) {
        let shared = match Self::shared_opt() {
            Some(v) => v,
            None => return,
        };
        if event.usage() == Usage::DELETE
            && event.modifier().has_ctrl()
            && event.modifier().has_alt()
        {
            // ctrl alt del
            unsafe {
                System::reset();
            }
        } else if let Some(window) = shared.active {
            let _ = Self::post_system_event(WindowSystemEvent::Key(window, event));
        }
    }

    pub fn set_desktop_color(color: SomeColor) {
        let desktop = Self::shared().root;
        desktop.update(|window| window.bitmap = None);
        desktop.set_bg_color(color);
    }

    pub fn set_desktop_bitmap(bitmap: &ConstBitmap) {
        let shared = Self::shared();
        let _ = shared.root.update_opt(|root| {
            if root.bitmap.is_none() {
                let bitmap = ConstBitmap::from(&shared.main_screen);
                root.bitmap = Some(UnsafeCell::new(BoxedBitmap::same_format(
                    &bitmap,
                    root.frame.size(),
                    root.bg_color,
                )));
            }
            root.bitmap()
                .map(|mut v| v.blt(bitmap, Point::default(), bitmap.bounds()));
            root.set_needs_display();
        });
    }

    #[inline]
    pub fn is_pointer_visible() -> bool {
        Self::shared().pointer.is_visible()
    }

    pub fn set_pointer_visible(visible: bool) -> bool {
        let result = Self::is_pointer_visible();
        if visible {
            Self::shared().pointer.show();
        } else if result {
            Self::shared().pointer.hide();
        }
        result
    }

    #[inline]
    pub fn while_hiding_pointer<F, R>(f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let pointer_visible = Self::set_pointer_visible(false);
        let result = f();
        Self::set_pointer_visible(pointer_visible);
        result
    }

    pub fn save_screen_to(bitmap: &mut Bitmap32, rect: Rect) {
        let shared = Self::shared();
        Self::while_hiding_pointer(|| shared.root.draw_into(bitmap, rect));
    }
}

bitflags! {
    struct WindowManagerAttributes: usize {
        const PORTRAIT      = 0b0000_0001;
        const EVENT         = 0b0000_0010;
        const MOUSE_MOVE    = 0b0000_0100;
        const NEEDS_REDRAW  = 0b0000_1000;
        const MOVING        = 0b0001_0000;
    }
}

impl Into<usize> for WindowManagerAttributes {
    fn into(self) -> usize {
        self.bits()
    }
}

/// Raw implementation of the window
#[allow(dead_code)]
struct RawWindow<'a> {
    /// Refer to the self owned handle
    handle: WindowHandle,

    // Properties
    attributes: AtomicBitflags<WindowAttributes>,
    style: WindowStyle,
    level: WindowLevel,

    // Placement and Size
    frame: Rect,
    shadow_insets: EdgeInsets,
    content_insets: EdgeInsets,

    // Appearances
    bg_color: SomeColor,
    bitmap: Option<UnsafeCell<BoxedBitmap<'a>>>,

    /// Window Title
    title: [u8; WINDOW_TITLE_LENGTH],

    // Messages and Events
    waker: AtomicWaker,
    sem: Semaphore,
    queue: Option<ArrayQueue<WindowMessage>>,

    // TODO: Window Hierachies
    next: Option<WindowHandle>,
}

bitflags! {
    pub struct WindowStyle: u8 {
        const BORDER        = 0b0000_0001;
        const TITLE         = 0b0000_0010;
        const NAKED         = 0b0000_0100;
        const OPAQUE        = 0b0000_1000;
        const PINCHABLE     = 0b0001_0000;
        const FLOATING      = 0b0010_0000;

        const DEFAULT = Self::BORDER.bits | Self::TITLE.bits;
    }
}

impl WindowStyle {
    fn as_content_insets(self) -> EdgeInsets {
        let mut insets = if self.contains(Self::BORDER) {
            EdgeInsets::padding_each(WINDOW_BORDER_PADDING)
        } else {
            EdgeInsets::default()
        };
        if self.contains(Self::TITLE) {
            insets.top += WINDOW_TITLE_HEIGHT;
        }
        insets
    }
}

bitflags! {
    struct WindowAttributes: usize {
        const NEEDS_REDRAW  = 0b0000_0001;
        const VISIBLE       = 0b0000_0010;
    }
}

impl Into<usize> for WindowAttributes {
    fn into(self) -> usize {
        self.bits()
    }
}

impl RawWindow<'_> {
    #[inline]
    fn actual_bounds(&self) -> Rect {
        self.frame.size().into()
    }

    #[inline]
    fn visible_frame(&self) -> Rect {
        self.frame.insets_by(self.shadow_insets)
    }

    fn show(&mut self) {
        self.draw_frame();
        WindowManager::synchronized(|| unsafe {
            WindowManager::add_hierarchy(self.handle);
        });
        WindowManager::invalidate_screen(self.frame);
    }

    fn hide(&self) {
        let shared = WindowManager::shared_mut();
        let frame = self.frame;
        let new_active = if shared.active.contains(&self.handle) {
            self.prev()
        } else {
            None
        };
        if shared.captured.contains(&self.handle) {
            shared.captured = None;
        }
        WindowManager::synchronized(|| unsafe {
            WindowManager::remove_hierarchy(self.handle);
        });
        WindowManager::invalidate_screen(frame);
        if new_active.is_some() {
            WindowManager::set_active(new_active);
        }
    }

    fn set_frame(&mut self, new_frame: Rect) {
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
                self.draw_frame();

                let coords1 = match Coordinates::from_rect(old_frame) {
                    Ok(v) => v,
                    Err(_) => return,
                };
                let coords2 = match Coordinates::from_rect(new_frame) {
                    Ok(v) => v,
                    Err(_) => return,
                };
                let new_coords = Coordinates::new(
                    isize::min(coords1.left, coords2.left),
                    isize::min(coords1.top, coords2.top),
                    isize::max(coords1.right, coords2.right),
                    isize::max(coords1.bottom, coords2.bottom),
                );
                WindowManager::invalidate_screen(new_coords.into());
            }
        }
    }

    fn draw_to_screen(&self, rect: Rect) {
        let mut frame = rect;
        frame.origin += self.frame.origin;
        let shared = WindowManager::shared_mut();
        let main_screen = &mut shared.main_screen;
        let off_screen = shared.off_screen.inner();
        if self.draw_into(off_screen, frame) {
            if shared
                .attributes
                .contains(WindowManagerAttributes::PORTRAIT)
            {
                main_screen.blt_affine(off_screen, frame.origin, frame);
            } else {
                main_screen.blt(off_screen, frame.origin, frame);
            }
        }
    }

    fn draw_into(&self, target_bitmap: &mut Bitmap32, frame: Rect) -> bool {
        let coords1 = match Coordinates::from_rect(frame) {
            Ok(coords) => coords,
            Err(_) => return false,
        };

        let mut cursor = if self.style.contains(WindowStyle::OPAQUE) {
            self.handle
        } else {
            WindowManager::shared().root
        };

        loop {
            let window = cursor.as_ref();
            if let Ok(coords2) = Coordinates::from_rect(window.frame) {
                if frame.is_within_rect(window.frame) {
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

                    if let Some(bitmap) = window.bitmap() {
                        match bitmap {
                            Bitmap::Indexed(bitmap) => {
                                target_bitmap.blt8(
                                    bitmap,
                                    blt_origin,
                                    blt_rect,
                                    &IndexedColor::COLOR_PALETTE,
                                );
                            }
                            Bitmap::Argb32(bitmap) => {
                                if window.style.contains(WindowStyle::OPAQUE) {
                                    target_bitmap.blt(bitmap, blt_origin, blt_rect);
                                } else {
                                    target_bitmap.blt_blend(bitmap, blt_origin, blt_rect);
                                }
                            }
                        }
                    } else {
                        if window.style.contains(WindowStyle::OPAQUE) {
                            target_bitmap.fill_rect(blt_rect, window.bg_color.into());
                        } else {
                            target_bitmap.blend_rect(blt_rect, window.bg_color.into());
                        }
                    }
                }
            }
            cursor = match window.next {
                Some(next) => next,
                None => break,
            };
        }

        true
    }

    fn set_bg_color(&mut self, color: SomeColor) {
        self.bg_color = color;
        if let Some(mut bitmap) = self.bitmap() {
            bitmap.fill_rect(bitmap.bounds(), color.into());
            self.draw_frame();
        }
        self.set_needs_display();
    }

    fn title_frame(&self) -> Rect {
        if self.style.contains(WindowStyle::TITLE) {
            Rect::new(
                WINDOW_BORDER_SHADOW_PADDING + WINDOW_BORDER_PADDING,
                WINDOW_BORDER_SHADOW_PADDING + WINDOW_BORDER_PADDING,
                self.frame.width() - WINDOW_BORDER_PADDING * 2 - WINDOW_BORDER_SHADOW_PADDING * 2,
                WINDOW_TITLE_HEIGHT,
            )
        } else {
            Rect::default()
        }
    }

    #[inline]
    fn is_active(&self) -> bool {
        WindowManager::shared().active.contains(&self.handle)
    }

    #[inline]
    fn refresh_title(&mut self) {
        self.draw_frame();
        if self.style.contains(WindowStyle::TITLE) {
            self.invalidate_rect(self.title_frame());
        }
    }

    fn draw_frame(&mut self) {
        if let Some(mut bitmap) = self.bitmap() {
            let is_active = self.is_active();

            if self.style.contains(WindowStyle::BORDER) {
                match &mut bitmap {
                    Bitmap::Argb32(bitmap) => {
                        let q = WINDOW_BORDER_SHADOW_PADDING;
                        let rect = Rect::from(bitmap.size());
                        for n in 0..q {
                            let rect = rect.insets_by(EdgeInsets::padding_each(n));
                            let light = 1 + n as u8;
                            let color = TrueColor::TRANSPARENT.set_opacity(light * light);
                            bitmap.draw_rect(rect, color);
                        }
                        let shared = WindowManager::shared();
                        let corner = &shared.resources.corner_shadow;
                        bitmap.blt(corner, Point::new(0, 0), Rect::new(0, 0, q, q));
                        bitmap.blt(
                            corner,
                            Point::new(rect.width() - q, 0),
                            Rect::new(q, 0, q, q),
                        );
                        bitmap.blt(
                            corner,
                            Point::new(0, rect.height() - q),
                            Rect::new(0, q, q, q),
                        );
                        bitmap.blt(
                            corner,
                            Point::new(rect.width() - q, rect.height() - q),
                            Rect::new(q, q, q, q),
                        );
                    }
                    _ => (),
                }
            }
            if self.style.contains(WindowStyle::TITLE) {
                let shared = WindowManager::shared();

                let rect = self.title_frame();
                bitmap.fill_rect(
                    rect,
                    if is_active {
                        WINDOW_ACTIVE_TITLE_BG_COLOR
                    } else {
                        WINDOW_INACTIVE_TITLE_BG_COLOR
                    },
                );

                if let Some(text) = self.title() {
                    let font = shared.resources.title_font;
                    let rect = rect.insets_by(EdgeInsets::new(0, 8, 0, 8));
                    AttributedString::new()
                        .font(font)
                        .color(if is_active {
                            WINDOW_ACTIVE_TITLE_FG_COLOR
                        } else {
                            WINDOW_INACTIVE_TITLE_FG_COLOR
                        })
                        .center()
                        .text(text)
                        .draw_text(&mut bitmap, rect, 1);
                }
            }
        }
    }

    #[inline]
    pub fn set_needs_display(&self) {
        match self.handle.post(WindowMessage::Draw) {
            Ok(()) => (),
            Err(_) => {
                let shared = WindowManager::shared();
                shared
                    .attributes
                    .insert(WindowManagerAttributes::NEEDS_REDRAW);
                shared.sem_event.signal();
            }
        }
    }

    fn invalidate_rect(&mut self, rect: Rect) {
        if self.attributes.contains(WindowAttributes::VISIBLE) {
            self.draw_to_screen(rect);
        }
    }

    fn set_title_array(array: &mut [u8; WINDOW_TITLE_LENGTH], title: &str) {
        let mut i = 1;
        for c in title.bytes() {
            if i >= WINDOW_TITLE_LENGTH {
                break;
            }
            array[i] = c;
            i += 1;
        }
        array[0] = i as u8 - 1;
    }

    fn set_title(&mut self, title: &str) {
        RawWindow::set_title_array(&mut self.title, title);
        self.draw_frame();
        self.invalidate_rect(self.title_frame());
    }

    fn prev(&self) -> Option<WindowHandle> {
        WindowManager::synchronized(|| {
            let handle = self.handle;
            let mut cursor = Some(WindowManager::shared().root);
            while let Some(current) = cursor {
                let current = current.as_ref();
                if current.next.contains(&handle) {
                    return Some(current.handle);
                }
                cursor = current.next;
            }
            None
        })
    }
}

impl<'a> RawWindow<'a> {
    #[inline]
    fn bitmap(&self) -> Option<Bitmap<'a>> {
        self.bitmap
            .as_ref()
            .and_then(|v| unsafe { v.get().as_mut() })
            .map(|v| v.as_bitmap())
    }

    fn title<'b>(&self) -> Option<&'b str> {
        let len = self.title[0] as usize;
        match len {
            0 => None,
            _ => core::str::from_utf8(unsafe { core::slice::from_raw_parts(&self.title[1], len) })
                .ok(),
        }
    }

    fn draw_in_rect<F>(&self, rect: Rect, f: F) -> Result<(), WindowDrawingError>
    where
        F: FnOnce(&mut Bitmap) -> (),
    {
        let mut bitmap = match self.bitmap() {
            Some(bitmap) => bitmap,
            None => return Err(WindowDrawingError::NoBitmap),
        };
        let bounds = Rect::from(self.frame.size).insets_by(self.content_insets);
        let origin = Point::new(isize::max(0, rect.x()), isize::max(0, rect.y()));
        let coords = match Coordinates::from_rect(Rect::new(
            origin.x + bounds.x(),
            origin.y + bounds.y(),
            isize::min(rect.width(), bounds.width() - origin.x),
            isize::min(rect.height(), bounds.height() - origin.y),
        )) {
            Ok(coords) => coords,
            Err(_) => return Err(WindowDrawingError::InconsistentCoordinates),
        };
        if coords.left > coords.right || coords.top > coords.bottom {
            return Err(WindowDrawingError::InconsistentCoordinates);
        }

        let rect = coords.into();
        match bitmap.view(rect, |mut bitmap| f(&mut bitmap)) {
            Some(_) => Ok(()),
            None => Err(WindowDrawingError::InconsistentCoordinates),
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct WindowLevel(pub u8);

impl WindowLevel {
    /// Root window (desktop)
    pub const ROOT: WindowLevel = WindowLevel(0);
    /// Items on the desktop
    pub const DESKTOP_ITEMS: WindowLevel = WindowLevel(1);
    /// Normal window
    pub const NORMAL: WindowLevel = WindowLevel(32);
    /// Floating window
    pub const FLOATING: WindowLevel = WindowLevel(64);
    /// Popup barrier
    pub const POPUP_BARRIER: WindowLevel = WindowLevel(96);
    /// Popup window
    pub const POPUP: WindowLevel = WindowLevel(97);
    /// The mouse pointer, which is also the foremost window.
    pub const POINTER: WindowLevel = WindowLevel(127);
}

pub struct WindowBuilder {
    frame: Rect,
    style: WindowStyle,
    level: WindowLevel,
    bg_color: SomeColor,
    title: [u8; WINDOW_TITLE_LENGTH],
    queue_size: usize,
    bitmap_strategy: BitmapStrategy,
}

impl WindowBuilder {
    #[inline]
    pub fn new(title: &str) -> Self {
        let window = Self {
            frame: Rect::new(isize::MIN, isize::MIN, 300, 300),
            level: WindowLevel::NORMAL,
            style: WindowStyle::DEFAULT,
            bg_color: WindowManager::DEFAULT_BGCOLOR,
            title: [0; WINDOW_TITLE_LENGTH],
            queue_size: 100,
            bitmap_strategy: BitmapStrategy::default(),
        };
        window.title(title).style(WindowStyle::DEFAULT)
    }

    #[inline]
    pub fn build(self) -> WindowHandle {
        let window = self.build_inner();
        let handle = window.handle;
        WindowManager::add(window);
        handle
    }

    #[inline]
    fn build_inner<'a>(mut self) -> Box<RawWindow<'a>> {
        let screen_bounds = WindowManager::user_screen_bounds();
        let shadow_insets = if self.style.contains(WindowStyle::BORDER) {
            EdgeInsets::padding_each(WINDOW_BORDER_SHADOW_PADDING)
        } else {
            EdgeInsets::default()
        };
        let window_insets = self.style.as_content_insets();
        let content_insets = window_insets + shadow_insets;
        let mut frame = self.frame;
        if self.style.contains(WindowStyle::NAKED) {
            frame.size += window_insets;
        }
        if frame.x() == isize::MIN {
            frame.origin.x = (screen_bounds.width() - frame.width()) / 2;
        } else if frame.x() < 0 {
            frame.origin.x += screen_bounds.x() + screen_bounds.width();
        }
        if frame.y() == isize::MIN {
            frame.origin.y = isize::max(
                screen_bounds.y(),
                (screen_bounds.height() - frame.height()) / 2,
            );
        } else if frame.y() < 0 {
            frame.origin.y += screen_bounds.y() + screen_bounds.height();
        }
        frame.origin -= Point::new(shadow_insets.left, shadow_insets.top);
        frame.size += shadow_insets;

        if self.style.contains(WindowStyle::FLOATING) {
            self.level = WindowLevel::FLOATING;
        }

        let attributes = if self.level == WindowLevel::ROOT {
            AtomicBitflags::new(WindowAttributes::VISIBLE)
        } else {
            AtomicBitflags::empty()
        };

        let queue = match self.queue_size {
            0 => None,
            _ => Some(ArrayQueue::new(self.queue_size)),
        };

        let handle = WindowManager::next_window_handle();
        let mut window = Box::new(RawWindow {
            handle,
            frame,
            shadow_insets,
            content_insets,
            style: self.style,
            level: self.level,
            bg_color: self.bg_color,
            bitmap: None,
            title: self.title,
            attributes,
            waker: AtomicWaker::new(),
            sem: Semaphore::new(0),
            queue,
            next: None,
        });

        match self.bitmap_strategy {
            BitmapStrategy::NonBitmap => (),
            BitmapStrategy::Native | BitmapStrategy::Compact | BitmapStrategy::Expressive => {
                window.bitmap = Some(UnsafeCell::new(
                    BoxedBitmap32::new(frame.size(), self.bg_color.into()).into(),
                ));
            }
        }

        window
    }

    #[inline]
    pub const fn style(mut self, style: WindowStyle) -> Self {
        self.style = style;
        self
    }

    #[inline]
    pub const fn style_add(mut self, style: WindowStyle) -> Self {
        self.style.bits |= style.bits();
        self
    }

    #[inline]
    pub fn title(mut self, title: &str) -> Self {
        RawWindow::set_title_array(&mut self.title, title);
        self
    }

    #[inline]
    pub const fn level(mut self, level: WindowLevel) -> Self {
        self.level = level;
        self
    }

    #[inline]
    pub const fn frame(mut self, frame: Rect) -> Self {
        self.frame = frame;
        self
    }

    #[inline]
    pub const fn center(mut self) -> Self {
        self.frame.origin = Point::new(isize::MIN, isize::MIN);
        self
    }

    #[inline]
    pub const fn origin(mut self, origin: Point) -> Self {
        self.frame.origin = origin;
        self
    }

    #[inline]
    pub const fn size(mut self, size: Size) -> Self {
        self.frame.size = size;
        self
    }

    #[inline]
    pub const fn bg_color(mut self, bg_color: SomeColor) -> Self {
        self.bg_color = bg_color;
        self
    }

    #[inline]
    pub const fn message_queue_size(mut self, queue_size: usize) -> Self {
        self.queue_size = queue_size;
        self
    }

    #[inline]
    pub const fn without_message_queue(mut self) -> Self {
        self.queue_size = 0;
        self
    }

    #[inline]
    pub const fn bitmap_strategy(mut self, bitmap_strategy: BitmapStrategy) -> Self {
        self.bitmap_strategy = bitmap_strategy;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitmapStrategy {
    NonBitmap,
    Native,
    Compact,
    Expressive,
}

impl Default for BitmapStrategy {
    fn default() -> Self {
        Self::Compact
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct WindowHandle(pub NonZeroUsize);

impl WindowHandle {
    #[inline]
    pub fn new(val: usize) -> Option<Self> {
        NonZeroUsize::new(val).map(|x| Self(x))
    }

    #[inline]
    pub const fn as_usize(&self) -> usize {
        self.0.get()
    }

    #[inline]
    fn get<'a>(&self) -> Option<&'a Box<RawWindow<'static>>> {
        WindowManager::shared().get(self)
    }

    #[inline]
    #[track_caller]
    fn as_ref<'a>(&self) -> &'a RawWindow<'static> {
        self.get().unwrap()
    }

    #[inline]
    fn update_opt<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut RawWindow) -> R,
    {
        WindowManager::shared_mut().get_mut(self, f)
    }

    #[inline]
    #[track_caller]
    fn update<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut RawWindow) -> R,
    {
        self.update_opt(f).unwrap()
    }

    // :-:-:-:-:

    #[inline]
    pub fn is_active(&self) -> bool {
        self.get().map(|v| v.is_active()).unwrap_or(false)
    }

    #[inline]
    pub fn set_title(&self, title: &str) {
        self.update(|window| {
            window.set_title(title);
        });
    }

    #[inline]
    pub fn title<'a>(&self) -> Option<&'a str> {
        self.as_ref().title()
    }

    pub fn set_bg_color(&self, color: SomeColor) {
        self.update(|window| {
            window.set_bg_color(color);
        });
    }

    #[inline]
    pub fn bg_color(&self) -> SomeColor {
        self.as_ref().bg_color
    }

    #[inline]
    pub fn frame(&self) -> Rect {
        self.as_ref().visible_frame()
    }

    pub fn set_frame(&self, rect: Rect) {
        self.update(|window| {
            window.set_frame(rect);
        });
    }

    #[inline]
    pub fn content_insets(&self) -> EdgeInsets {
        self.as_ref().content_insets
    }

    #[inline]
    pub fn content_rect(&self) -> Rect {
        let window = self.as_ref();
        Rect::from(window.frame.size()).insets_by(window.content_insets)
    }

    #[inline]
    pub fn content_size(&self) -> Size {
        self.content_rect().size()
    }

    #[inline]
    pub fn move_by(&self, delta: Point) {
        let mut new_rect = self.frame();
        new_rect.origin += delta;
        self.set_frame(new_rect);
    }

    #[inline]
    pub fn move_to(&self, new_origin: Point) {
        let mut new_rect = self.frame();
        new_rect.origin = new_origin;
        self.set_frame(new_rect);
    }

    #[inline]
    pub fn resize_to(&self, new_size: Size) {
        let mut new_rect = self.frame();
        new_rect.size = new_size;
        self.set_frame(new_rect);
    }

    pub fn show(&self) {
        self.update(|window| window.show());
    }

    pub fn hide(&self) {
        self.update(|window| window.hide());
    }

    #[inline]
    pub fn close(&self) {
        self.hide();
        WindowManager::remove(self);
    }

    #[inline]
    pub fn is_visible(&self) -> bool {
        self.as_ref().attributes.contains(WindowAttributes::VISIBLE)
    }

    #[inline]
    pub fn make_active(&self) {
        WindowManager::set_active(Some(*self));
    }

    #[inline]
    pub fn invalidate_rect(&self, rect: Rect) {
        self.update(|window| {
            let mut frame = rect;
            frame.origin.x += window.content_insets.left;
            frame.origin.y += window.content_insets.top;
            window.invalidate_rect(frame);
        });
    }

    #[inline]
    pub fn set_needs_display(&self) {
        self.as_ref().set_needs_display();
    }

    #[inline]
    pub fn draw<F>(&self, f: F) -> Result<(), WindowDrawingError>
    where
        F: FnOnce(&mut Bitmap) -> (),
    {
        self.update(|window| {
            let rect = window.actual_bounds().insets_by(window.content_insets);
            match self.draw_in_rect(rect.size().into(), f) {
                Ok(_) | Err(WindowDrawingError::NoBitmap) => {
                    window.invalidate_rect(rect);
                    Ok(())
                }
                Err(err) => Err(err),
            }
        })
    }

    pub fn draw_in_rect<F>(&self, rect: Rect, f: F) -> Result<(), WindowDrawingError>
    where
        F: FnOnce(&mut Bitmap) -> (),
    {
        self.as_ref().draw_in_rect(rect, f)
    }

    /// Draws the contents of the window on the screen as a bitmap.
    pub fn draw_into(&self, target_bitmap: &mut Bitmap32, rect: Rect) {
        // self.as_ref().draw_into(target_bitmap, rect);
        let window = self.as_ref();
        let mut frame = rect;
        frame.origin.x += window.frame.x() + window.shadow_insets.left;
        frame.origin.y += window.frame.y() + window.shadow_insets.top;
        window.draw_into(target_bitmap, frame);
    }

    /// Post a window message.
    pub fn post(&self, message: WindowMessage) -> Result<(), WindowPostError> {
        let window = match self.get() {
            Some(window) => window,
            None => return Err(WindowPostError::NotFound),
        };
        if let Some(queue) = window.queue.as_ref() {
            match message {
                WindowMessage::Draw => {
                    window.attributes.insert(WindowAttributes::NEEDS_REDRAW);
                    window.waker.wake();
                    window.sem.signal();
                    Ok(())
                }
                _ => queue
                    .push(message)
                    .map_err(|_| WindowPostError::Full)
                    .map(|_| {
                        window.waker.wake();
                        window.sem.signal();
                    }),
            }
        } else {
            Err(WindowPostError::NotFound)
        }
    }

    /// Read a window message from the message queue.
    pub fn read_message(&self) -> Option<WindowMessage> {
        let window = match self.get() {
            Some(window) => window,
            None => return None,
        };
        if let Some(queue) = window.queue.as_ref() {
            match queue.pop() {
                Some(v) => Some(v),
                _ => {
                    if window
                        .attributes
                        .test_and_clear(WindowAttributes::NEEDS_REDRAW)
                    {
                        Some(WindowMessage::Draw)
                    } else {
                        None
                    }
                }
            }
        } else {
            None
        }
    }

    /// Wait for window messages to be read.
    pub fn wait_message(&self) -> Option<WindowMessage> {
        loop {
            let window = match self.get() {
                Some(window) => window,
                None => return None,
            };
            match self.read_message() {
                Some(message) => return Some(message),
                None => window.sem.wait(),
            }
        }
    }

    /// Supports asynchronous reading of window messages.
    pub fn poll_message(&self, cx: &mut Context<'_>) -> Option<WindowMessage> {
        self.as_ref().waker.register(cx.waker());
        self.read_message().map(|message| {
            self.as_ref().waker.take();
            message
        })
    }

    /// Get the window message asynchronously.
    pub fn get_message(&self) -> Pin<Box<dyn Future<Output = Option<WindowMessage>>>> {
        Box::pin(WindowMessageConsumer { handle: *self })
    }

    /// Process window messages that are not handled.
    pub fn handle_default_message(&self, message: WindowMessage) {
        match message {
            WindowMessage::Draw => {
                self.draw(|_bitmap| {}).unwrap();
            }
            WindowMessage::Key(key) => {
                if let Some(c) = key.key_data().map(|v| v.into_char()) {
                    let _ = self.post(WindowMessage::Char(c));
                }
            }
            _ => (),
        }
    }

    ///
    pub fn refresh_if_needed(&self) {
        let window = match self.get() {
            Some(v) => v,
            None => return,
        };
        if window
            .attributes
            .test_and_clear(WindowAttributes::NEEDS_REDRAW)
        {
            self.draw(|_bitmap| {}).unwrap();
        }
    }

    /// Create a timer associated with a window
    pub fn create_timer(&self, timer_id: usize, duration: Duration) {
        let mut event = TimerEvent::window(*self, timer_id, Timer::new(duration));
        loop {
            if event.until() {
                match Scheduler::schedule_timer(event) {
                    Ok(()) => break,
                    Err(e) => event = e,
                }
            } else {
                break event.fire();
            }
        }
    }
}

struct WindowMessageConsumer {
    handle: WindowHandle,
}

impl Future for WindowMessageConsumer {
    type Output = Option<WindowMessage>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.handle.poll_message(cx) {
            Some(v) => Poll::Ready(Some(v)),
            None => Poll::Pending,
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Copy, Clone)]
pub enum WindowDrawingError {
    NoBitmap,
    InconsistentCoordinates,
}

#[non_exhaustive]
#[derive(Debug, Copy, Clone)]
pub enum WindowPostError {
    NotFound,
    Full,
}

#[non_exhaustive]
#[derive(Debug, Copy, Clone)]
pub enum WindowMessage {
    /// Dummy message
    Nop,
    /// Requested to close the window
    Close,
    /// Needs to be redrawn
    Draw,
    // Active
    Activated,
    Deactivated,
    /// Raw keyboard event
    Key(KeyEvent),
    /// Unicode converted keyboard event
    Char(char),
    // mouse events
    MouseMove(MouseEvent),
    MouseDown(MouseEvent),
    MouseUp(MouseEvent),
    MouseEnter,
    MouseLeave,
    /// Timer event
    Timer(usize),
    /// User Defined
    User(usize),
}

pub enum WindowSystemEvent {
    /// Raw Keyboard event
    Key(WindowHandle, KeyEvent),
}
