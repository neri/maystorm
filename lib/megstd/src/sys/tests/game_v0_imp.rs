/// Retro Game Framework v0
pub use crate::drawing::*;
pub use crate::game::v0::{self, GamePresenter};

// use crate::window;
use core::cell::UnsafeCell;
use core::marker::PhantomData;

pub struct GameWindow;

static mut SCREEN: UnsafeCell<v0::Screen> = UnsafeCell::new(v0::Screen::new());

pub struct GamePresenterImpl {
    _phantom: PhantomData<()>,
}

impl v0::GamePresenter for GamePresenterImpl {
    #[inline]
    fn screen<'a>(&'a self) -> &'a mut v0::Screen {
        unsafe { &mut *SCREEN.get() }
    }

    #[inline]
    fn buttons(&self) -> u32 {
        todo!()
    }

    #[inline]
    fn sync(&self) -> usize {
        todo!()
    }

    #[inline]
    fn set_needs_display(&self) {
        todo!()
    }

    #[inline]
    fn invalidate_rect(&self, _rect: Rect) {
        todo!()
    }

    #[inline]
    fn move_sprite(&self, _index: v0::SpriteIndex, _origin: Point) {
        todo!()
    }

    #[inline]
    fn load_font(&self, _start_index: v0::TileIndex, _start_char: u8, _end_char: u8) {
        todo!()
    }
}
