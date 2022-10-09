//! A Window System

use super::{font::*, text::*, theme::Theme};
use crate::{
    io::hid_mgr::*,
    res::icon::IconManager,
    sync::atomic::AtomicBitflags,
    sync::{fifo::*, semaphore::*},
    sync::{Mutex, RwLock},
    task::scheduler::*,
    *,
};
use alloc::{boxed::Box, collections::btree_map::BTreeMap, sync::Arc, vec::Vec};
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
use futures_util::task::AtomicWaker;
use megstd::{drawing::*, io::hid::*, sys::megos};

const MAX_WINDOWS: usize = 255;
const WINDOW_SYSTEM_EVENT_QUEUE_SIZE: usize = 100;

const WINDOW_BORDER_WIDTH: isize = 1;
const WINDOW_THICK_BORDER_WIDTH_V: isize = WINDOW_CORNER_RADIUS / 2;
const WINDOW_THICK_BORDER_WIDTH_H: isize = WINDOW_CORNER_RADIUS / 2;
const WINDOW_CORNER_RADIUS: isize = 8;
const WINDOW_TITLE_HEIGHT: isize = 28;
const WINDOW_TITLE_LENGTH: usize = 32;
const WINDOW_SHADOW_PADDING: isize = 16;
const SHADOW_RADIUS: isize = 8;
const SHADOW_OFFSET: Movement = Movement::new(2, 2);
const SHADOW_LEVEL: usize = 96;

// Mouse Pointer
const MOUSE_POINTER_WIDTH: usize = 12;
const MOUSE_POINTER_HEIGHT: usize = 20;
#[rustfmt::skip]
const MOUSE_POINTER_SOURCE: [u8; MOUSE_POINTER_WIDTH * MOUSE_POINTER_HEIGHT] = [
    0x0F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0x0F, 0x0F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0x0F, 0x07, 0x0F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0x0F, 0x00, 0x07, 0x0F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0x0F, 0x00, 0x00, 0x07, 0x0F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0x0F, 0x00, 0x00, 0x00, 0x07, 0x0F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0x0F, 0x00, 0x00, 0x00, 0x00, 0x07, 0x0F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0x0F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07, 0x0F, 0xFF, 0xFF, 0xFF, 0xFF,
    0x0F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07, 0x0F, 0xFF, 0xFF, 0xFF,
    0x0F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07, 0x0F, 0xFF, 0xFF,
    0x0F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07, 0x0F, 0xFF,
    0x0F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07, 0x0F,
    0x0F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F,
    0x0F, 0x00, 0x00, 0x07, 0x0F, 0x00, 0x07, 0x0F, 0xFF, 0xFF, 0xFF, 0xFF,
    0x0F, 0x00, 0x07, 0x0F, 0x0F, 0x07, 0x00, 0x0F, 0xFF, 0xFF, 0xFF, 0xFF,
    0x0F, 0x07, 0x0F, 0xFF, 0xFF, 0x0F, 0x00, 0x07, 0x0F, 0xFF, 0xFF, 0xFF,
    0x0F, 0x0F, 0xFF, 0xFF, 0xFF, 0x0F, 0x07, 0x00, 0x0F, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F, 0x00, 0x0F, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F, 0x0F, 0x0F, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
];

static mut WM: Option<Box<WindowManager<'static>>> = None;

pub struct WindowManager<'a> {
    sem_event: Semaphore,
    attributes: AtomicBitflags<WindowManagerAttributes>,
    system_event: ConcurrentFifo<WindowSystemEvent>,

    pointer_x: AtomicIsize,
    pointer_y: AtomicIsize,
    buttons: AtomicUsize,
    buttons_down: AtomicUsize,
    buttons_up: AtomicUsize,

    main_screen: UnsafeCell<Bitmap32<'a>>,
    screen_size: Size,
    screen_insets: EdgeInsets,
    update_coords: Mutex<Coordinates>,

    resources: Resources<'a>,

    window_pool: RwLock<BTreeMap<WindowHandle, Arc<UnsafeCell<Box<RawWindow>>>>>,
    window_orders: RwLock<Vec<WindowHandle>>,

    root: WindowHandle,
    pointer: WindowHandle,
    barrier: WindowHandle,
    active: Option<WindowHandle>,
    captured: Option<WindowHandle>,
    captured_offset: Movement,
    entered: Option<WindowHandle>,
}

#[allow(dead_code)]
struct Resources<'a> {
    window_button_width: isize,
    close_button: OperationalBitmap,
    back_button: OperationalBitmap,
    title_font: FontDescriptor,
    label_font: FontDescriptor,
    _phantom: &'a (),
}

impl WindowManager<'static> {
    pub fn init(main_screen: Bitmap32<'static>) {
        let attributes = AtomicBitflags::EMPTY;

        let mut screen_size = main_screen.size();
        if screen_size.width < screen_size.height {
            attributes.insert(WindowManagerAttributes::ROTATE);
            screen_size.swap();
        }

        let pointer_x = screen_size.width / 2;
        let pointer_y = screen_size.height / 2;
        let mut window_pool = BTreeMap::new();
        let mut window_orders = Vec::with_capacity(MAX_WINDOWS);

        let window_button_width = WINDOW_TITLE_HEIGHT;
        let close_button = IconManager::mask(r::Icons::Close).unwrap();
        let back_button = IconManager::mask(r::Icons::ChevronLeft).unwrap();

        let root = {
            let window = WindowBuilder::new()
                .style(WindowStyle::OPAQUE | WindowStyle::NO_SHADOW)
                .level(WindowLevel::ROOT)
                .frame(Rect::from(screen_size))
                .bg_color(Color::from_rgb(0x000000))
                .without_message_queue()
                .bitmap_strategy(BitmapStrategy::NonBitmap)
                .build_inner("Desktop");

            let handle = window.handle;
            window_pool.insert(handle, Arc::new(UnsafeCell::new(window)));
            handle
        };
        window_orders.push(root);

        let pointer = {
            let pointer_size =
                Size::new(MOUSE_POINTER_WIDTH as isize, MOUSE_POINTER_HEIGHT as isize);
            let window = WindowBuilder::new()
                .style(WindowStyle::empty())
                .level(WindowLevel::POINTER)
                .origin(Point::new(pointer_x, pointer_y))
                .size(pointer_size)
                .bg_color(Color::Transparent)
                .without_message_queue()
                .build_inner("Pointer");

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

        let barrier = {
            let window = WindowBuilder::new()
                .style(WindowStyle::NO_SHADOW)
                .level(WindowLevel::POPUP_BARRIER)
                .frame(Rect::from(screen_size))
                .bg_color(Color::from_rgb(0))
                .without_message_queue()
                .bitmap_strategy(BitmapStrategy::NonBitmap)
                .build_inner("Barrier");

            let handle = window.handle;
            window_pool.insert(handle, Arc::new(UnsafeCell::new(window)));
            handle
        };

        unsafe {
            WM = Some(Box::new(WindowManager {
                sem_event: Semaphore::new(0),
                attributes,
                pointer_x: AtomicIsize::new(pointer_x),
                pointer_y: AtomicIsize::new(pointer_y),
                buttons: AtomicUsize::new(0),
                buttons_down: AtomicUsize::new(0),
                buttons_up: AtomicUsize::new(0),
                main_screen: UnsafeCell::new(main_screen),
                screen_size,
                screen_insets: EdgeInsets::default(),
                resources: Resources {
                    _phantom: &(),
                    close_button,
                    window_button_width,
                    back_button,
                    title_font: FontManager::title_font(),
                    label_font: FontManager::ui_font(),
                },
                window_pool: RwLock::new(window_pool),
                window_orders: RwLock::new(window_orders),
                root,
                pointer,
                barrier,
                active: None,
                captured: None,
                captured_offset: Movement::default(),
                entered: None,
                system_event: ConcurrentFifo::with_capacity(WINDOW_SYSTEM_EVENT_QUEUE_SIZE),
                update_coords: Mutex::new(Coordinates::VOID),
            }));
        }

        SpawnOption::with_priority(Priority::High).start_process(
            Self::window_thread,
            0,
            "Window Manager",
        );
    }

    #[track_caller]
    fn add(window: Box<RawWindow>) {
        let handle = window.handle;
        WindowManager::shared_mut()
            .window_pool
            .write()
            .unwrap()
            .insert(handle, Arc::new(UnsafeCell::new(window)));
    }

    fn remove(window: &RawWindow) {
        window.hide();
        let shared = WindowManager::shared_mut();
        let window_orders = shared.window_orders.write().unwrap();
        let handle = window.handle;
        shared.window_pool.write().unwrap().remove(&handle);
        drop(window_orders);
    }

    #[inline]
    fn get<'a>(&self, key: &WindowHandle) -> Option<&'a Box<RawWindow>> {
        match WindowManager::shared().window_pool.read() {
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
        let window = match WindowManager::shared_mut().window_pool.write() {
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
                let mut update_coords = shared.update_coords.lock().unwrap();
                if update_coords.is_valid() {
                    let coords = *update_coords;
                    *update_coords = Coordinates::VOID;
                    drop(update_coords);
                    shared.root.as_ref().draw_inner_to_screen(coords.into());
                }
            }
            if shared
                .attributes
                .test_and_clear(WindowManagerAttributes::EVENT)
            {
                while let Some(event) = shared.system_event.dequeue() {
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
                if shared.pointer.is_visible() {
                    let position = shared.pointer();
                    let current_buttons =
                        MouseButton::from_bits_retain(shared.buttons.load(Ordering::Acquire) as u8);
                    let buttons_down = MouseButton::from_bits_retain(
                        shared.buttons_down.swap(0, Ordering::SeqCst) as u8,
                    );
                    let buttons_up = MouseButton::from_bits_retain(
                        shared.buttons_up.swap(0, Ordering::SeqCst) as u8,
                    );

                    if let Some(captured) = shared.captured {
                        if current_buttons.contains(MouseButton::PRIMARY) {
                            if shared
                                .attributes
                                .contains(WindowManagerAttributes::CLOSE_DOWN)
                            {
                                let _ = captured.update_opt(|window| {
                                    if window.test_frame(position, window.close_button_frame()) {
                                        window.set_close_state(ViewActionState::Pressed);
                                    } else {
                                        window.set_close_state(ViewActionState::Normal);
                                    }
                                });
                            } else if shared
                                .attributes
                                .contains(WindowManagerAttributes::BACK_DOWN)
                            {
                                let _ = captured.update_opt(|window| {
                                    if window.test_frame(position, window.back_button_frame()) {
                                        window.set_back_state(ViewActionState::Pressed);
                                    } else {
                                        window.set_back_state(ViewActionState::Normal);
                                    }
                                });
                            } else if shared.attributes.contains(WindowManagerAttributes::MOVING) {
                                // dragging title
                                let top = if captured.as_ref().level < WindowLevel::FLOATING {
                                    shared.screen_insets.top
                                } else {
                                    0
                                };
                                let bottom = shared.screen_size.height()
                                    - WINDOW_TITLE_HEIGHT / 2
                                    - if captured.as_ref().level < WindowLevel::FLOATING {
                                        shared.screen_insets.bottom
                                    } else {
                                        0
                                    };
                                let x = position.x - shared.captured_offset.x;
                                let y = cmp::min(
                                    cmp::max(position.y - shared.captured_offset.y, top),
                                    bottom,
                                );
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
                            if shared
                                .attributes
                                .contains(WindowManagerAttributes::CLOSE_DOWN)
                            {
                                let _ = captured.update_opt(|window| {
                                    window.set_close_state(ViewActionState::Normal);
                                    if window.test_frame(position, window.close_button_frame()) {
                                        let _ = captured.post(WindowMessage::Close);
                                    }
                                });
                            } else if shared
                                .attributes
                                .contains(WindowManagerAttributes::BACK_DOWN)
                            {
                                let _ = captured.update_opt(|window| {
                                    window.set_back_state(ViewActionState::Normal);
                                    if window.test_frame(position, window.back_button_frame()) {
                                        let _ = captured.post(WindowMessage::Back);
                                    }
                                });
                            } else {
                                let _ = Self::make_mouse_events(
                                    captured,
                                    position,
                                    current_buttons,
                                    buttons_down,
                                    buttons_up,
                                );
                            }

                            shared.captured = None;
                            shared.attributes.remove(
                                WindowManagerAttributes::MOVING
                                    | WindowManagerAttributes::CLOSE_DOWN
                                    | WindowManagerAttributes::BACK_DOWN,
                            );

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

                        if buttons_down.contains(MouseButton::PRIMARY) {
                            if let Some(active) = shared.active {
                                if active != target {
                                    WindowManager::set_active(Some(target));
                                }
                            } else {
                                WindowManager::set_active(Some(target));
                            }

                            let target_window = target.as_ref();
                            if target_window.close_button_state != ViewActionState::Disabled
                                && target_window
                                    .test_frame(position, target_window.close_button_frame())
                            {
                                let _ = target.update_opt(|window| {
                                    window.set_close_state(ViewActionState::Pressed)
                                });
                                shared
                                    .attributes
                                    .insert(WindowManagerAttributes::CLOSE_DOWN);
                            } else if target_window.back_button_state != ViewActionState::Disabled
                                && target_window
                                    .test_frame(position, target_window.back_button_frame())
                            {
                                let _ = target.update_opt(|window| {
                                    window.set_back_state(ViewActionState::Pressed)
                                });
                                shared.attributes.insert(WindowManagerAttributes::BACK_DOWN);
                            } else if target_window.style.contains(WindowStyle::PINCHABLE) {
                                shared.attributes.insert(WindowManagerAttributes::MOVING);
                            } else {
                                if target_window.test_frame(position, target_window.title_frame()) {
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
                            shared.captured_offset =
                                position - target_window.visible_frame().origin;
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
    }

    #[inline]
    fn post_system_event(event: WindowSystemEvent) -> Result<(), WindowSystemEvent> {
        let shared = Self::shared();
        let r = shared.system_event.enqueue(event);
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
    fn next_window_handle() -> WindowHandle {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
        WindowHandle::new(NEXT_ID.fetch_add(1, Ordering::SeqCst)).unwrap()
    }

    fn add_hierarchy(window: WindowHandle) {
        let window = match window.get() {
            Some(v) => v,
            None => return,
        };

        Self::remove_hierarchy(window.handle);
        let mut window_orders = WindowManager::shared_mut().window_orders.write().unwrap();

        let mut insert_position = None;
        for (index, lhs) in window_orders.iter().enumerate() {
            if lhs.as_ref().level > window.level {
                insert_position = Some(index);
                break;
            }
        }
        if let Some(insert_position) = insert_position {
            window_orders.insert(insert_position, window.handle);
        } else {
            window_orders.push(window.handle);
        }

        window.as_ref().attributes.insert(WindowAttributes::VISIBLE);

        drop(window_orders);
    }

    fn remove_hierarchy(window: WindowHandle) {
        let window = match window.get() {
            Some(v) => v,
            None => return,
        };

        window.attributes.remove(WindowAttributes::VISIBLE);

        let mut window_orders = WindowManager::shared_mut().window_orders.write().unwrap();
        let mut remove_position = None;
        for (index, lhs) in window_orders.iter().enumerate() {
            if *lhs == window.handle {
                remove_position = Some(index);
                break;
            }
        }
        if let Some(remove_position) = remove_position {
            window_orders.remove(remove_position);
        }

        drop(window_orders);
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
            None => System::main_screen().bounds(),
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
        let mut update_coords = shared.update_coords.lock().unwrap();
        if let Ok(coords) = Coordinates::from_rect(rect) {
            update_coords.merge(coords);
            shared
                .attributes
                .insert(WindowManagerAttributes::NEEDS_REDRAW);
            shared.sem_event.signal();
        }
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
        let shared = WindowManager::shared();
        let window_orders = shared.window_orders.read().unwrap();
        for handle in window_orders.iter().rev().skip(1) {
            let window = handle.as_ref();
            if window.frame.contains(point) {
                return *handle;
            }
        }
        shared.root
    }

    fn pointer(&self) -> Point {
        Point::new(
            self.pointer_x.load(Ordering::Relaxed),
            self.pointer_y.load(Ordering::Relaxed),
        )
    }

    fn _update_relative_coord(
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

    fn _update_absolute_coord(
        coord: &AtomicIsize,
        new_value: isize,
        min_value: isize,
        max_value: isize,
    ) -> bool {
        match coord.fetch_update(Ordering::SeqCst, Ordering::Relaxed, |old_value| {
            let new_value = cmp::min(cmp::max(new_value, min_value), max_value);
            if old_value == new_value {
                None
            } else {
                Some(new_value)
            }
        }) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    pub fn post_relative_pointer(pointer_state: &mut MouseState) {
        let shared = match Self::shared_opt() {
            Some(v) => v,
            None => return,
        };
        let screen_bounds: Rect = shared.screen_size.into();

        let mut pointer = Point::new(0, 0);
        swap(&mut pointer_state.x, &mut pointer.x);
        swap(&mut pointer_state.y, &mut pointer.y);
        let button_changes = pointer_state.current_buttons ^ pointer_state.prev_buttons;
        let button_down = button_changes & pointer_state.current_buttons;
        let button_up = button_changes & pointer_state.prev_buttons;
        let button_changed = !button_changes.is_empty();

        if button_changed {
            shared.buttons.store(
                pointer_state.current_buttons.bits() as usize,
                Ordering::SeqCst,
            );
            shared
                .buttons_down
                .fetch_or(button_down.bits() as usize, Ordering::SeqCst);
            shared
                .buttons_up
                .fetch_or(button_up.bits() as usize, Ordering::SeqCst);
        }

        let moved = Self::_update_relative_coord(
            &shared.pointer_x,
            pointer.x,
            screen_bounds.x(),
            screen_bounds.width() - 1,
        ) | Self::_update_relative_coord(
            &shared.pointer_y,
            pointer.y,
            screen_bounds.y(),
            screen_bounds.height() - 1,
        );

        if button_changed | moved {
            fence(Ordering::SeqCst);
            shared
                .attributes
                .insert(WindowManagerAttributes::MOUSE_MOVE);
            shared.sem_event.signal();
        }
    }

    pub fn post_absolute_pointer(pointer_state: &mut MouseState) {
        let shared = match Self::shared_opt() {
            Some(v) => v,
            None => return,
        };
        let screen_bounds: Rect = shared.screen_size.into();
        let button_changes = pointer_state.current_buttons ^ pointer_state.prev_buttons;
        let button_down = button_changes & pointer_state.current_buttons;
        let button_up = button_changes & pointer_state.prev_buttons;
        let button_changed = !button_changes.is_empty();

        if button_changed {
            shared.buttons.store(
                pointer_state.current_buttons.bits() as usize,
                Ordering::SeqCst,
            );
            shared
                .buttons_down
                .fetch_or(button_down.bits() as usize, Ordering::SeqCst);
            shared
                .buttons_up
                .fetch_or(button_up.bits() as usize, Ordering::SeqCst);
        }

        let pointer_x = screen_bounds.width() * pointer_state.x / pointer_state.max_x;
        let pointer_y = screen_bounds.height() * pointer_state.y / pointer_state.max_y;

        let moved = Self::_update_absolute_coord(
            &shared.pointer_x,
            pointer_x,
            screen_bounds.x(),
            screen_bounds.width() - 1,
        ) | Self::_update_absolute_coord(
            &shared.pointer_y,
            pointer_y,
            screen_bounds.y(),
            screen_bounds.height() - 1,
        );

        if button_changed | moved {
            fence(Ordering::SeqCst);
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
            System::reset();
        } else if let Some(window) = shared.active {
            let _ = Self::post_system_event(WindowSystemEvent::Key(window, event));
        }
    }

    #[inline]
    pub fn current_desktop_window() -> WindowHandle {
        Self::shared().root
    }

    pub fn set_desktop_color(_color: Color) {
        // let desktop = Self::shared().root;
        // desktop.update(|window| {
        //     window.set_bg_color(color);
        // });
    }

    pub fn set_desktop_bitmap<'a, T: AsRef<ConstBitmap<'a>>>(bitmap: &T) {
        let bitmap = bitmap.as_ref();
        let shared = Self::shared();
        let _ = shared.root.update_opt(|root| {
            let (mut r, mut g, mut b, mut a) = (0, 0, 0, 0);
            for pixel in bitmap.all_pixels() {
                let c = pixel.into_true_color().components();
                r += c.r as usize;
                g += c.g as usize;
                b += c.b as usize;
                a += c.a as usize;
            }
            let total_pixels = bitmap.width() * bitmap.height();
            let tint_color = Color::Argb32(TrueColor::from(ColorComponents::from_rgba(
                r.checked_div(total_pixels).unwrap_or_default() as u8,
                g.checked_div(total_pixels).unwrap_or_default() as u8,
                b.checked_div(total_pixels).unwrap_or_default() as u8,
                a.checked_div(total_pixels).unwrap_or_default() as u8,
            )));

            root.set_bg_color(tint_color);
            let mut target = Bitmap::from(root.bitmap());
            let origin = Point::new(
                (target.bounds().width() - bitmap.bounds().width()) / 2,
                (target.bounds().height() - bitmap.bounds().height()) / 2,
            );
            target.blt_transparent(bitmap, origin, bitmap.bounds(), IndexedColor::DEFAULT_KEY);

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

    pub fn get_statistics(sb: &mut impl Write) {
        let shared = Self::shared();

        writeln!(sb, "  # PID Lv Frame",).unwrap();
        for window in shared.window_pool.read().unwrap().values() {
            let window = unsafe { &*window.clone().as_ref().get() };
            let frame = window.frame;
            writeln!(
                sb,
                "{:3} {:3} {:2x} {:4} {:4} {:4} {:4} {}",
                window.handle.0,
                usize::from(window.pid),
                window.level.0,
                frame.x(),
                frame.y(),
                frame.width(),
                frame.height(),
                window.title().unwrap_or("")
            )
            .unwrap();
        }
    }

    pub fn set_barrier_opacity(opacity: u8) {
        let shared = Self::shared();
        let barrier = shared.barrier;
        if opacity > 0 {
            let color = TrueColor::from_gray(0, opacity);
            barrier.set_bg_color(color.into());
            if !barrier.is_visible() {
                barrier.show();
            }
        } else {
            barrier.hide();
        }
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    struct WindowManagerAttributes: u64 {
        const ROTATE        = 0b0000_0001;
        const EVENT         = 0b0000_0010;
        const MOUSE_MOVE    = 0b0000_0100;
        const NEEDS_REDRAW  = 0b0000_1000;
        const MOVING        = 0b0001_0000;
        const CLOSE_DOWN    = 0b0010_0000;
        const BACK_DOWN     = 0b0100_0000;
    }
}

impl Into<usize> for WindowManagerAttributes {
    fn into(self) -> usize {
        self.bits() as usize
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ViewActionState {
    Normal,
    Hover,
    Pressed,
    Disabled,
}

impl Default for ViewActionState {
    #[inline]
    fn default() -> Self {
        Self::Normal
    }
}

/// Raw implementation of the window
#[allow(dead_code)]
struct RawWindow {
    /// Refer to the self owned handle
    handle: WindowHandle,
    pid: ProcessId,

    // Properties
    attributes: AtomicBitflags<WindowAttributes>,
    style: AtomicBitflags<WindowStyle>,
    level: WindowLevel,

    // Placement and Size
    frame: Rect,
    content_insets: EdgeInsets,

    // Appearances
    bg_color: Color,
    accent_color: Color,
    active_title_color: Color,
    inactive_title_color: Color,
    bitmap: UnsafeCell<OwnedBitmap32>,
    shadow_bitmap: Option<UnsafeCell<OperationalBitmap>>,
    back_buffer: UnsafeCell<OwnedBitmap32>,

    /// Window Title
    title: [u8; WINDOW_TITLE_LENGTH],
    close_button_state: ViewActionState,
    back_button_state: ViewActionState,

    // Messages and Events
    waker: AtomicWaker,
    sem: Semaphore,
    queue: Option<ConcurrentFifo<WindowMessage>>,
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct WindowStyle: u64 {
        const BORDER            = 0b0000_0000_0000_0001;
        const THIN_FRAME        = 0b0000_0000_0000_0010;
        const TITLE             = 0b0000_0000_0000_0100;
        const CLOSE_BUTTON      = 0b0000_0000_0000_1000;

        const OPAQUE_CONTENT    = 0b0000_0000_0001_0000;
        const OPAQUE            = 0b0000_0000_0010_0000;
        const NO_SHADOW         = 0b0000_0000_0100_0000;
        const FLOATING          = 0b0000_0000_1000_0000;

        const DARK_BORDER       = 0b0000_0001_0000_0000;
        const DARK_TITLE        = 0b0000_0010_0000_0000;
        const DARK_ACTIVE       = 0b0000_0100_0000_0000;
        const PINCHABLE         = 0b0000_1000_0000_0000;

        const FULLSCREEN        = 0b0001_0000_0000_0000;

        const SUSPENDED         = 0b1000_0000_0000_0000;

        const DEFAULT           = Self::BORDER.bits() | Self::TITLE.bits() | Self::CLOSE_BUTTON.bits();
    }
}

impl const Default for WindowStyle {
    #[inline]
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl const From<WindowStyle> for usize {
    fn from(val: WindowStyle) -> Self {
        val.bits() as usize
    }
}

impl const From<usize> for WindowStyle {
    fn from(val: usize) -> Self {
        Self::from_bits_retain(val as u64)
    }
}

impl WindowStyle {
    fn as_content_insets(self) -> EdgeInsets {
        let insets = if self.contains(Self::BORDER) {
            if self.contains(Self::THIN_FRAME) {
                if self.contains(Self::TITLE) {
                    EdgeInsets::new(
                        WINDOW_BORDER_WIDTH + WINDOW_TITLE_HEIGHT,
                        WINDOW_BORDER_WIDTH,
                        WINDOW_BORDER_WIDTH,
                        WINDOW_BORDER_WIDTH,
                    )
                } else {
                    EdgeInsets::padding_each(WINDOW_BORDER_WIDTH)
                }
            } else {
                if self.contains(Self::TITLE) {
                    EdgeInsets::new(
                        WINDOW_THICK_BORDER_WIDTH_V + WINDOW_TITLE_HEIGHT,
                        WINDOW_THICK_BORDER_WIDTH_H,
                        WINDOW_THICK_BORDER_WIDTH_V,
                        WINDOW_THICK_BORDER_WIDTH_H,
                    )
                } else {
                    EdgeInsets::new(
                        WINDOW_THICK_BORDER_WIDTH_V,
                        WINDOW_THICK_BORDER_WIDTH_H,
                        WINDOW_THICK_BORDER_WIDTH_V,
                        WINDOW_THICK_BORDER_WIDTH_H,
                    )
                }
            }
        } else {
            EdgeInsets::default()
        };
        insets
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    struct WindowAttributes: u64 {
        const NEEDS_REDRAW  = 0b0000_0001;
        const VISIBLE       = 0b0000_0010;
    }
}

impl Into<usize> for WindowAttributes {
    fn into(self) -> usize {
        self.bits() as usize
    }
}

impl RawWindow {
    #[inline]
    fn actual_bounds(&self) -> Rect {
        self.frame.bounds()
    }

    #[inline]
    fn visible_frame(&self) -> Rect {
        self.frame
    }

    #[inline]
    fn shadow_frame(&self) -> Rect {
        if self.style.contains(WindowStyle::NO_SHADOW) {
            self.frame
        } else {
            self.frame + EdgeInsets::padding_each(WINDOW_SHADOW_PADDING)
        }
    }

    fn show(&mut self) {
        self.draw_frame();
        self.update_shadow();
        WindowManager::add_hierarchy(self.handle);

        let frame = self.shadow_frame();
        self.draw_outer_to_screen(frame.origin().into(), frame.bounds(), false);
    }

    fn hide(&self) {
        let shared = WindowManager::shared_mut();
        let frame = self.shadow_frame();
        let new_active = if shared.active.contains(&self.handle) {
            let window_orders = shared.window_orders.read().unwrap();
            window_orders
                .iter()
                .position(|v| *v == self.handle)
                .and_then(|v| window_orders.get(v - 1))
                .map(|&v| v)
        } else {
            None
        };
        if shared.captured.contains(&self.handle) {
            shared.captured = None;
        }
        WindowManager::remove_hierarchy(self.handle);
        WindowManager::invalidate_screen(frame);
        if new_active.is_some() {
            WindowManager::set_active(new_active);
        }
    }

    #[inline]
    pub fn close(&self) {
        WindowManager::remove(self);
    }

    fn set_frame(&mut self, new_frame: Rect) {
        let old_frame = self.frame;
        if old_frame != new_frame {
            let old_frame = self.shadow_frame();
            self.frame = new_frame;
            if self.attributes.contains(WindowAttributes::VISIBLE) {
                self.draw_frame();

                let coords1 = match Coordinates::from_rect(old_frame) {
                    Ok(v) => v,
                    Err(_) => return,
                };
                let coords2 = match Coordinates::from_rect(self.shadow_frame()) {
                    Ok(v) => v,
                    Err(_) => return,
                };
                WindowManager::invalidate_screen(Rect::from(coords1.merged(coords2)));
            }
        }
    }

    fn test_frame(&self, position: Point, frame: Rect) -> bool {
        let mut frame = frame;
        frame.origin += Movement::from(self.frame.origin);
        frame.contains(position)
    }

    fn draw_inner_to_screen(&self, rect: Rect) {
        let coords = match Coordinates::from_rect(rect) {
            Ok(v) => v,
            Err(_) => return,
        };
        let bounds = self.frame.bounds();

        let is_opaque = self.style.contains(WindowStyle::OPAQUE)
            || self.style.contains(WindowStyle::OPAQUE_CONTENT)
                && bounds.insets_by(self.content_insets).contains(rect);

        let shared = WindowManager::shared();
        let is_direct = if is_opaque {
            let window_orders = shared.window_orders.read().unwrap();
            let first_index = 1 + window_orders
                .iter()
                .position(|&v| v == self.handle)
                .unwrap_or(0);
            let screen_rect = rect + Movement::from(self.frame.origin);

            let mut is_direct = true;
            for handle in window_orders[first_index..].iter() {
                let window = match handle.get() {
                    Some(v) => v,
                    None => continue,
                };
                if screen_rect.overlaps(window.shadow_frame()) {
                    is_direct = false;
                    break;
                }
            }
            is_direct
        } else {
            false
        };
        if is_direct {
            let offset = self.frame.origin;
            let bitmap = self.bitmap();
            let main_screen = unsafe { &mut *shared.main_screen.get() };
            if shared.attributes.contains(WindowManagerAttributes::ROTATE) {
                main_screen.blt_rotate(
                    bitmap.as_const(),
                    offset + Movement::from(coords.left_top()),
                    coords.into(),
                );
            } else {
                main_screen.blt(
                    bitmap.as_const(),
                    offset + Movement::from(coords.left_top()),
                    coords.into(),
                );

                // main_screen.draw_rect(rect + Movement::from(offset), Color::YELLOW.into());
            }
        } else {
            drop(shared);

            let inner_coords = match Coordinates::from_rect(bounds) {
                Ok(v) => v,
                Err(_) => return,
            };
            let frame_origin = self.frame.origin;
            let offset = self.shadow_frame().origin;
            let rect = Rect::from(coords.trimmed(inner_coords)) + (frame_origin - offset);
            self.draw_outer_to_screen(Movement::from(offset), rect, is_opaque);
        }
    }

    fn draw_outer_to_screen(&self, offset: Movement, rect: Rect, is_opaque: bool) {
        let screen_rect = rect + offset;
        let shared = WindowManager::shared_mut();
        let main_screen = unsafe { &mut *shared.main_screen.get() };
        let back_buffer = unsafe { &mut *self.back_buffer.get() };
        let back_buffer = back_buffer.as_mut();
        if self.draw_into(back_buffer, offset, screen_rect, is_opaque) {
            if shared.attributes.contains(WindowManagerAttributes::ROTATE) {
                main_screen.blt_rotate(back_buffer.as_const(), rect.origin + offset, rect);
            } else {
                main_screen.blt(back_buffer.as_const(), rect.origin + offset, rect);

                // if is_opaque {
                //     main_screen.draw_rect(rect + offset, Color::BLUE.into());
                // } else {
                //     main_screen.draw_rect(rect + offset, Color::RED.into());
                // }
            }
        }
    }

    fn draw_into(
        &self,
        target_bitmap: &mut Bitmap32,
        offset: Movement,
        frame1: Rect,
        is_opaque: bool,
    ) -> bool {
        let coords1 = match Coordinates::from_rect(frame1) {
            Ok(coords) => coords,
            Err(_) => return false,
        };

        let window_orders = WindowManager::shared().window_orders.read().unwrap();

        let first_index = if is_opaque {
            window_orders
                .iter()
                .position(|&v| v == self.handle)
                .unwrap_or(0)
        } else {
            0
        };

        for handle in window_orders[first_index..].iter() {
            let window = handle.as_ref();
            let frame2 = window.shadow_frame();
            let coords2 = match Coordinates::from_rect(frame2) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if frame2.overlaps(frame1) {
                let adjust_point = window.frame.origin() - coords2.left_top();
                let blt_origin = Point::new(
                    cmp::max(coords1.left, coords2.left),
                    cmp::max(coords1.top, coords2.top),
                ) - offset;
                let target_rect = Rect::new(
                    isize::max(coords1.left - coords2.left, 0),
                    isize::max(coords1.top - coords2.top, 0),
                    cmp::min(coords1.right, coords2.right) - cmp::max(coords1.left, coords2.left),
                    cmp::min(coords1.bottom, coords2.bottom) - cmp::max(coords1.top, coords2.top),
                );

                if !window.frame.contains(frame1) {
                    if let Some(shadow) = window.shadow_bitmap() {
                        shadow.blt_shadow(target_bitmap, blt_origin, target_rect);
                    }
                }

                let bitmap = window.bitmap();
                let blt_rect = target_rect - adjust_point;
                if window.style.contains(WindowStyle::OPAQUE)
                    || self.handle == window.handle && is_opaque
                {
                    target_bitmap.blt(bitmap.as_const(), blt_origin, blt_rect);
                } else {
                    target_bitmap.blt_blend(bitmap.as_const(), blt_origin, blt_rect);
                }
            }
        }

        true
    }

    fn set_bg_color(&mut self, color: Color) {
        self.bg_color = color;
        self.style.set(
            WindowStyle::DARK_BORDER,
            color.brightness().unwrap_or(255) < 128,
        );
        let bitmap = self.bitmap();
        bitmap.fill_rect(bitmap.bounds(), color.into());
        self.draw_frame();
        self.set_needs_display();
    }

    fn title_frame(&self) -> Rect {
        if self.style.contains(WindowStyle::TITLE) {
            Rect::new(
                WINDOW_BORDER_WIDTH,
                WINDOW_BORDER_WIDTH,
                self.frame.width() - WINDOW_BORDER_WIDTH * 2,
                WINDOW_TITLE_HEIGHT,
            )
        } else {
            Rect::default()
        }
    }

    fn close_button_frame(&self) -> Rect {
        let shared = WindowManager::shared();
        let rect = self.title_frame();
        let window_button_width = shared.resources.window_button_width;
        Rect::new(
            rect.max_x() - window_button_width - WINDOW_CORNER_RADIUS,
            rect.y(),
            window_button_width,
            rect.height(),
        )
    }

    fn back_button_frame(&self) -> Rect {
        let shared = WindowManager::shared();
        let rect = self.title_frame();
        let window_button_width = shared.resources.window_button_width;
        Rect::new(
            WINDOW_CORNER_RADIUS,
            rect.y(),
            window_button_width,
            rect.height(),
        )
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
        let mut bitmap = Bitmap::from(self.bitmap());
        let is_active = self.is_active();
        let is_thin = self.style.contains(WindowStyle::THIN_FRAME);
        let is_dark = self.style.contains(WindowStyle::DARK_BORDER);

        if self.style.contains(WindowStyle::TITLE) {
            let shared = WindowManager::shared();
            let padding = 8;
            let left = padding;
            let right = padding;

            let rect = self.title_frame();
            bitmap
                .view(rect, |bitmap| {
                    let rect = bitmap.bounds();

                    if is_thin {
                        bitmap.fill_rect(rect, self.title_background());
                    } else {
                        // let rect = rect.insets_by(EdgeInsets::new(
                        //     0,
                        //     WINDOW_CORNER_RADIUS,
                        //     0,
                        //     WINDOW_CORNER_RADIUS,
                        // ));
                        bitmap.fill_rect(rect, self.title_background());
                    }

                    self.draw_close_button();
                    self.draw_back_button();

                    if let Some(text) = self.title() {
                        let font = shared.resources.title_font;
                        let rect = rect.insets_by(EdgeInsets::new(0, left, 0, right));

                        if is_active {
                            let rect2 = rect + Movement::new(1, 1);
                            AttributedString::new()
                                .font(font)
                                .color(if self.style.contains(WindowStyle::DARK_ACTIVE) {
                                    Theme::shared().window_title_active_shadow_dark()
                                } else {
                                    Theme::shared().window_title_active_shadow()
                                })
                                .center()
                                .text(text)
                                .draw_text(bitmap, rect2, 1);
                        }

                        AttributedString::new()
                            .font(font)
                            .color(self.title_foreground())
                            .center()
                            .text(text)
                            .draw_text(bitmap, rect, 1);
                    }
                })
                .unwrap();
        }

        if self.style.contains(WindowStyle::BORDER) {
            if is_thin {
                // Thin frame
                if WINDOW_BORDER_WIDTH > 0 {
                    let rect = Rect::from(bitmap.size());
                    bitmap.draw_rect(
                        rect,
                        if is_dark {
                            Theme::shared().window_default_border_dark()
                        } else {
                            Theme::shared().window_default_border_light()
                        },
                    );
                }
            } else {
                // Thick frame
                let rect = Rect::from(bitmap.size());
                bitmap.fill_round_rect_outside(rect, WINDOW_CORNER_RADIUS, Color::TRANSPARENT);
                bitmap.draw_round_rect(
                    rect,
                    WINDOW_CORNER_RADIUS,
                    if is_dark {
                        Theme::shared().window_default_border_dark()
                    } else {
                        Theme::shared().window_default_border_light()
                    },
                );
            }
        }
    }

    #[inline]
    fn title_background(&self) -> Color {
        let is_active = self.is_active();
        if is_active {
            self.active_title_color
        } else {
            self.inactive_title_color
        }
    }

    #[inline]
    fn title_foreground(&self) -> Color {
        if self.is_active() {
            if self.style.contains(WindowStyle::DARK_ACTIVE) {
                Theme::shared().window_title_active_foreground_dark()
            } else {
                Theme::shared().window_title_active_foreground()
            }
        } else {
            if self.style.contains(WindowStyle::DARK_TITLE) {
                Theme::shared().window_title_inactive_foreground_dark()
            } else {
                Theme::shared().window_title_inactive_foreground()
            }
        }
    }

    fn draw_close_button(&mut self) {
        if !self.style.contains(WindowStyle::TITLE) {
            return;
        }
        let mut bitmap = Bitmap::from(self.bitmap());
        let shared = WindowManager::shared();
        let state = self.close_button_state;
        let button_frame = self.close_button_frame();
        let is_active = self.is_active() && state != ViewActionState::Disabled;

        let background = match state {
            ViewActionState::Pressed => Theme::shared().window_title_close_active_background(),
            _ => self.title_background(),
        };
        let foreground = match state {
            ViewActionState::Pressed => Theme::shared().window_title_close_active_foreground(),
            _ => {
                if is_active {
                    if self.style.contains(WindowStyle::DARK_ACTIVE) {
                        Theme::shared().window_title_close_foreground_dark()
                    } else {
                        Theme::shared().window_title_close_foreground()
                    }
                } else {
                    if self.style.contains(WindowStyle::DARK_TITLE) {
                        Theme::shared().window_title_inactive_foreground_dark()
                    } else {
                        Theme::shared().window_title_inactive_foreground()
                    }
                }
            }
        }
        .into_true_color();

        bitmap.fill_rect(button_frame, background);

        let button = &shared.resources.close_button;
        let origin = Point::new(
            button_frame.x() + (button_frame.width() - button.width() as isize) / 2,
            button_frame.y() + (button_frame.height() - button.height() as isize) / 2,
        );
        button.draw_to(&mut bitmap, origin, button.bounds(), foreground.into());
    }

    fn draw_back_button(&mut self) {
        if !self.style.contains(WindowStyle::TITLE) {
            return;
        }
        let mut bitmap = Bitmap::from(self.bitmap());
        let state = match self.back_button_state {
            ViewActionState::Disabled => return,
            other => other,
        };
        let shared = WindowManager::shared();
        let button_frame = self.back_button_frame();
        let is_active = self.is_active();

        let background = match state {
            ViewActionState::Pressed => Theme::shared().window_title_close_active_background(),
            _ => self.title_background(),
        };
        let foreground = match state {
            ViewActionState::Pressed => Theme::shared().window_title_close_active_foreground(),
            _ => {
                if is_active {
                    if self.style.contains(WindowStyle::DARK_ACTIVE) {
                        Theme::shared().window_title_close_foreground_dark()
                    } else {
                        Theme::shared().window_title_close_foreground()
                    }
                } else {
                    self.title_foreground()
                }
            }
        }
        .into_true_color();

        bitmap.fill_rect(button_frame, background);

        let button = &shared.resources.back_button;
        let origin = Point::new(
            button_frame.x() + (button_frame.width() - button.width() as isize) / 2,
            button_frame.y() + (button_frame.height() - button.height() as isize) / 2,
        );
        button.draw_to(&mut bitmap, origin, button.bounds(), foreground.into());
    }

    #[inline]
    pub fn set_needs_display(&self) {
        match self.handle.post(WindowMessage::Draw) {
            Ok(_) => {}
            Err(_) => {
                WindowManager::invalidate_screen(self.shadow_frame());
            }
        }
    }

    fn invalidate_rect(&mut self, rect: Rect) {
        if self.attributes.contains(WindowAttributes::VISIBLE) {
            self.draw_inner_to_screen(rect);
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

    #[inline]
    fn set_title(&mut self, title: &str) {
        RawWindow::set_title_array(&mut self.title, title);
        self.draw_frame();
        self.invalidate_rect(self.title_frame());
    }

    #[inline]
    fn set_close_state(&mut self, state: ViewActionState) {
        if self.close_button_state != state {
            self.close_button_state = state;
            self.update_close_button();
        }
    }

    #[inline]
    fn set_back_state(&mut self, state: ViewActionState) {
        if self.back_button_state != state {
            self.back_button_state = state;
            if state == ViewActionState::Disabled {
                self.draw_frame();
                self.invalidate_rect(self.title_frame());
            } else {
                self.update_back_button();
            }
        }
    }

    #[inline]
    fn update_close_button(&mut self) {
        self.draw_close_button();
        self.invalidate_rect(self.close_button_frame());
    }

    #[inline]
    fn update_back_button(&mut self) {
        self.draw_back_button();
        self.invalidate_rect(self.back_button_frame());
    }

    #[inline]
    fn shadow_bitmap<'a>(&'a self) -> Option<&'a mut OperationalBitmap> {
        self.shadow_bitmap
            .as_ref()
            .map(|v| unsafe { &mut *v.get() })
    }

    fn update_shadow(&self) {
        let bitmap = Bitmap::from(self.bitmap());
        let shadow = match self.shadow_bitmap() {
            Some(v) => v,
            None => return,
        };

        shadow.reset();

        let content_rect = Rect::from(self.frame.size());
        let origin = Point::new(
            WINDOW_SHADOW_PADDING - SHADOW_RADIUS,
            WINDOW_SHADOW_PADDING - SHADOW_RADIUS,
        ) + SHADOW_OFFSET;
        shadow.blt_from(&bitmap, origin, content_rect, |a, _| {
            let a = a.into_true_color().opacity();
            a.saturating_add(a)
        });

        shadow.blur(SHADOW_RADIUS, SHADOW_LEVEL);

        shadow.blt_from(
            &bitmap,
            Point::new(WINDOW_SHADOW_PADDING, WINDOW_SHADOW_PADDING),
            bitmap.bounds(),
            |a, b| {
                if a.into_true_color().opacity() >= b {
                    0
                } else {
                    b
                }
            },
        );
    }

    #[inline]
    fn bitmap<'a>(&self) -> &'a mut Bitmap32<'a> {
        unsafe { &mut *self.bitmap.get() }.as_mut()
    }

    #[inline]
    fn title<'a>(&self) -> Option<&'a str> {
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
        let mut bitmap = Bitmap::from(self.bitmap());
        let bounds = self.frame.bounds().insets_by(self.content_insets);
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
        bitmap
            .view(rect, |bitmap| f(bitmap))
            .ok_or(WindowDrawingError::InconsistentCoordinates)
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
    options: u32,
    level: WindowLevel,
    bg_color: Color,
    active_title_color: Option<Color>,
    inactive_title_color: Option<Color>,

    queue_size: usize,
    bitmap_strategy: BitmapStrategy,
}

impl WindowBuilder {
    #[inline]
    pub fn new() -> Self {
        Self {
            frame: Rect::new(isize::MIN, isize::MIN, 300, 300),
            level: WindowLevel::NORMAL,
            style: WindowStyle::default(),
            options: 0,
            bg_color: Theme::shared().window_default_background(),
            active_title_color: None,
            inactive_title_color: None,
            queue_size: 100,
            bitmap_strategy: BitmapStrategy::default(),
        }
    }

    pub fn build(self, title: &str) -> WindowHandle {
        let window = self.build_inner(title);
        let handle = window.handle;
        let style = window.style.value();
        WindowManager::add(window);
        if !style.contains(WindowStyle::SUSPENDED) {
            handle.make_active();
        }
        handle
    }

    fn build_inner<'a>(mut self, title: &str) -> Box<RawWindow> {
        let window_options = self.options;
        if (window_options & megos::window::THIN_FRAME) != 0 {
            self.style.insert(WindowStyle::THIN_FRAME);
        }
        if (window_options & megos::window::OPAQUE_CONTENT) != 0 {
            self.style.insert(WindowStyle::OPAQUE_CONTENT);
        }
        if (window_options & megos::window::USE_BITMAP32) != 0 {
            self.bitmap_strategy = BitmapStrategy::Expressive;
        }
        if (window_options & megos::window::FULLSCREEN) != 0 {
            self.style.insert(WindowStyle::FULLSCREEN);
        }
        if self.style.contains(WindowStyle::THIN_FRAME) {
            self.style.insert(WindowStyle::BORDER);
        }

        let screen_bounds = WindowManager::user_screen_bounds();
        let content_insets = self.style.as_content_insets();
        let frame = if self.style.contains(WindowStyle::FULLSCREEN) {
            WindowManager::user_screen_bounds()
        } else {
            let mut frame = self.frame;
            frame.size += content_insets;
            if frame.x() == isize::MIN {
                frame.origin.x = (screen_bounds.max_x() - frame.width()) / 2;
            } else if frame.x() < 0 {
                frame.origin.x +=
                    screen_bounds.max_x() - (content_insets.left + content_insets.right);
            }
            if frame.y() == isize::MIN {
                frame.origin.y = isize::max(
                    screen_bounds.min_y(),
                    (screen_bounds.max_y() - frame.height()) / 2,
                );
            } else if frame.y() < 0 {
                frame.origin.y +=
                    screen_bounds.max_y() - (content_insets.top + content_insets.bottom);
            }
            frame
        };

        if self.style.contains(WindowStyle::FLOATING) && self.level <= WindowLevel::NORMAL {
            self.level = WindowLevel::FLOATING;
        }

        let attributes = if self.level == WindowLevel::ROOT {
            AtomicBitflags::new(WindowAttributes::VISIBLE)
        } else {
            AtomicBitflags::empty()
        };

        let bg_color = self.bg_color;

        self.style.set(
            WindowStyle::DARK_BORDER,
            bg_color.brightness().unwrap_or(255) < 128,
        );
        let is_dark_mode = false; //self.style.contains(WindowStyle::DARK_BORDER);

        let accent_color = Theme::shared().window_default_accent();
        let active_title_color = self.active_title_color.unwrap_or(if is_dark_mode {
            Theme::shared().window_title_active_background_dark()
        } else {
            Theme::shared().window_title_active_background()
        });
        let inactive_title_color = self.inactive_title_color.unwrap_or(if is_dark_mode {
            bg_color
        } else {
            Theme::shared().window_title_inactive_background()
        });
        self.style.set(
            WindowStyle::DARK_ACTIVE,
            active_title_color.brightness().unwrap_or(255) < 192,
        );
        self.style.set(
            WindowStyle::DARK_TITLE,
            inactive_title_color.brightness().unwrap_or(255) < 128,
        );

        let queue = match self.queue_size {
            0 => None,
            _ => Some(ConcurrentFifo::with_capacity(self.queue_size)),
        };

        let bitmap = UnsafeCell::new(OwnedBitmap32::new(frame.size(), bg_color.into()));

        let shadow_bitmap = if self.style.contains(WindowStyle::NO_SHADOW) {
            None
        } else {
            let mut shadow = OperationalBitmap::new(
                frame.size() + Size::new(WINDOW_SHADOW_PADDING * 2, WINDOW_SHADOW_PADDING * 2),
            );
            shadow.reset();
            Some(UnsafeCell::new(shadow))
        };

        let mut title_array = [0; WINDOW_TITLE_LENGTH];
        RawWindow::set_title_array(&mut title_array, title);

        let close_button_state = if self.style.contains(WindowStyle::CLOSE_BUTTON) {
            Default::default()
        } else {
            ViewActionState::Disabled
        };

        let back_buffer = if let Some(ref shadow_bitmap) = shadow_bitmap {
            let shadow_bitmap = unsafe { &*shadow_bitmap.get() };
            UnsafeCell::new(OwnedBitmap32::new(
                shadow_bitmap.size(),
                TrueColor::TRANSPARENT,
            ))
        } else {
            UnsafeCell::new(OwnedBitmap32::new(frame.size(), TrueColor::TRANSPARENT))
        };

        let handle = WindowManager::next_window_handle();

        Box::new(RawWindow {
            handle,
            frame,
            content_insets,
            style: AtomicBitflags::new(self.style),
            level: self.level,
            bg_color,
            accent_color,
            active_title_color,
            inactive_title_color,
            bitmap,
            shadow_bitmap,
            back_buffer,
            title: title_array,
            close_button_state,
            back_button_state: ViewActionState::Disabled,
            attributes,
            waker: AtomicWaker::new(),
            sem: Semaphore::new(0),
            queue,
            pid: Scheduler::current_pid(),
        })
    }

    #[inline]
    pub const fn style(mut self, style: WindowStyle) -> Self {
        self.style = style;
        self
    }

    #[inline]
    pub const fn style_add(mut self, style: WindowStyle) -> Self {
        self.style = WindowStyle::from_bits_retain(self.style.bits() | style.bits());
        self
    }

    #[inline]
    pub const fn style_sub(mut self, style: WindowStyle) -> Self {
        self.style = WindowStyle::from_bits_retain(self.style.bits() & !style.bits());
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
    pub const fn bg_color(mut self, bg_color: Color) -> Self {
        self.bg_color = bg_color;
        self
    }

    #[inline]
    pub const fn active_title_color(mut self, active_title_color: Color) -> Self {
        self.active_title_color = Some(active_title_color);
        self
    }

    #[inline]
    pub const fn inactive_title_color(mut self, inactive_title_color: Color) -> Self {
        self.inactive_title_color = Some(inactive_title_color);
        self
    }

    #[inline]
    const fn without_message_queue(mut self) -> Self {
        self.queue_size = 0;
        self
    }

    #[inline]
    const fn bitmap_strategy(mut self, bitmap_strategy: BitmapStrategy) -> Self {
        self.bitmap_strategy = bitmap_strategy;
        self
    }

    /// Sets the window's content bitmap to ARGB32 format.
    #[inline]
    pub const fn bitmap_argb32(mut self) -> Self {
        self.options |= megos::window::USE_BITMAP32;
        self
    }

    /// Makes the border of the window a thin border.
    #[inline]
    pub const fn thin_frame(mut self) -> Self {
        self.options |= megos::window::THIN_FRAME;
        self
    }

    /// Content is opaque
    #[inline]
    pub const fn opaque(mut self) -> Self {
        self.options |= megos::window::OPAQUE_CONTENT;
        self
    }

    #[inline]
    pub const fn fullscreen(mut self) -> Self {
        self.options |= megos::window::FULLSCREEN;
        self
    }

    #[inline]
    pub const fn with_options(mut self, options: u32) -> Self {
        self.options = options;
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
    pub fn is_valid(&self) -> Option<Self> {
        self.get().map(|v| v.handle)
    }

    #[inline]
    #[track_caller]
    fn get<'a>(&self) -> Option<&'a Box<RawWindow>> {
        WindowManager::shared().get(self)
    }

    #[inline]
    #[track_caller]
    fn as_ref<'a>(&self) -> &'a RawWindow {
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

    #[inline]
    pub fn set_bg_color(&self, color: Color) {
        self.update(|window| {
            window.set_bg_color(color);
        });
    }

    #[inline]
    pub fn bg_color(&self) -> Color {
        self.as_ref().bg_color
    }

    #[inline]
    pub fn active_title_color(&self) -> Color {
        self.as_ref().active_title_color
    }

    #[inline]
    pub fn inactive_title_color(&self) -> Color {
        self.as_ref().inactive_title_color
    }

    #[inline]
    pub fn frame(&self) -> Rect {
        self.as_ref().visible_frame()
    }

    #[inline]
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
        window.frame.bounds().insets_by(window.content_insets)
    }

    #[inline]
    pub fn content_size(&self) -> Size {
        self.content_rect().size()
    }

    #[inline]
    pub fn move_by(&self, movement: Movement) {
        self.set_frame(self.frame() + movement);
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

    #[inline]
    pub fn show(&self) {
        self.update(|window| window.show());
    }

    #[inline]
    pub fn hide(&self) {
        self.update(|window| window.hide());
    }

    #[inline]
    pub fn close(&self) {
        self.as_ref().close();
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
    pub fn set_close_button_enabled(&self, enabled: bool) {
        self.update(|window| {
            if enabled {
                window.set_close_state(ViewActionState::Normal)
            } else {
                window.set_close_state(ViewActionState::Disabled)
            }
        });
    }

    #[inline]
    pub fn set_back_button_enabled(&self, enabled: bool) {
        self.update(|window| {
            if enabled {
                window.set_back_state(ViewActionState::Normal)
            } else {
                window.set_back_state(ViewActionState::Disabled)
            }
        });
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
    pub fn draw<F>(&self, f: F)
    where
        F: FnOnce(&mut Bitmap) -> (),
    {
        self.update(|window| {
            let rect = window.actual_bounds().insets_by(window.content_insets);
            match self.draw_in_rect(rect.size().into(), f) {
                Ok(_) | Err(WindowDrawingError::NoBitmap) => {
                    window.invalidate_rect(rect);
                }
                Err(_) => (),
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
        let window = self.as_ref();
        window.draw_into(
            target_bitmap,
            Movement::default(),
            rect + Movement::from(window.frame.origin),
            false,
        );
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
                    .enqueue(message)
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
            match queue.dequeue() {
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

    /// Get the window message asynchronously.
    pub fn await_message(&self) -> Pin<Box<dyn Future<Output = Option<WindowMessage>>>> {
        Box::pin(WindowMessageConsumer { handle: *self })
    }

    /// Supports asynchronous reading of window messages.
    pub fn poll_message(&self, cx: &mut Context<'_>) -> Poll<Option<WindowMessage>> {
        let window = match self.get() {
            Some(v) => v.as_ref(),
            None => return Poll::Ready(None),
        };
        window.waker.register(cx.waker());
        match self.read_message().map(|message| {
            self.as_ref().waker.take();
            message
        }) {
            Some(v) => Poll::Ready(Some(v)),
            None => Poll::Pending,
        }
    }

    /// Process window messages that are not handled.
    pub fn handle_default_message(&self, message: WindowMessage) {
        match message {
            WindowMessage::Draw => self.draw(|_| {}),
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
            self.draw(|_| {})
        }
    }

    /// Create a timer associated with a window
    pub fn create_timer(&self, timer_id: usize, duration: Duration) {
        let mut event = TimerEvent::window(*self, timer_id, Timer::new(duration));
        loop {
            if event.is_alive() {
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
        self.handle.poll_message(cx)
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
    ///
    Back,
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

#[non_exhaustive]
#[derive(Debug, Copy, Clone)]
pub enum WindowSystemEvent {
    /// Raw Keyboard event
    Key(WindowHandle, KeyEvent),
}
