/// Retro Game Framework v0
pub use crate::drawing::*;
pub use crate::game::v0::{self, GamePresenter};

use crate::window;
use crate::{sys::syscall::*, window::WindowHandle};
use core::cell::UnsafeCell;
use core::ffi::c_void;

pub struct GameWindow;

impl GameWindow {
    #[inline]
    pub fn new(title: &str, size: Size) -> GamePresenterImpl {
        let window = window::WindowBuilder::new()
            .opaque()
            .size(size * 2isize)
            .build(title);
        GamePresenterImpl::new(window.handle())
    }

    #[inline]
    pub fn with_options(title: &str, size: Size, fps: usize) -> GamePresenterImpl {
        let window = window::WindowBuilder::new()
            .opaque()
            .size(size * 2isize)
            .build(title);
        GamePresenterImpl::new_long(window.handle(), fps)
    }
}

static mut SCREEN: UnsafeCell<v0::Screen> = UnsafeCell::new(v0::Screen::new());

pub struct GamePresenterImpl {
    game_handle: usize,
}

impl GamePresenterImpl {
    #[inline]
    fn new(window: WindowHandle) -> Self {
        let game_handle = unsafe { game_v0_init(window.0, SCREEN.get() as *const c_void) };
        Self { game_handle }
    }

    #[inline]
    fn new_long(window: WindowHandle, fps: usize) -> Self {
        let game_handle =
            unsafe { game_v0_init_long(window.0, SCREEN.get() as *const c_void, fps) };
        Self { game_handle }
    }
}

impl v0::GamePresenter for GamePresenterImpl {
    #[inline]
    fn screen<'a>(&'a self) -> &'a mut v0::Screen {
        unsafe { &mut *SCREEN.get() }
    }

    #[inline]
    fn buttons(&self) -> u32 {
        game_v0_button(self.game_handle)
    }

    #[inline]
    fn sync(&self) -> usize {
        game_v0_sync(self.game_handle)
    }

    #[inline]
    fn set_needs_display(&self) {
        game_v0_rect(
            self.game_handle,
            0,
            0,
            v0::MAX_WIDTH as usize,
            v0::MAX_HEIGHT as usize,
        );
    }

    #[inline]
    fn invalidate_rect(&self, rect: Rect) {
        game_v0_rect(
            self.game_handle,
            rect.min_x() as usize,
            rect.min_y() as usize,
            rect.width() as usize,
            rect.height() as usize,
        );
    }

    #[inline]
    fn move_sprite(&self, index: v0::SpriteIndex, origin: Point) {
        game_v0_move_sprite(
            self.game_handle,
            index as usize,
            origin.x as usize,
            origin.y as usize,
        );
    }

    #[inline]
    fn load_font(&self, start_index: v0::TileIndex, start_char: u8, end_char: u8) {
        game_v0_load_font(
            self.game_handle,
            start_index as usize,
            start_char as usize,
            end_char as usize,
        );
    }
}
