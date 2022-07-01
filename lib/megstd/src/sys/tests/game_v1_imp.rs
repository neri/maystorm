/// Retro Game Framework v1
pub use crate::drawing::*;
pub use crate::game::v1::{self, GamePresenter};

// use crate::window;
use core::cell::UnsafeCell;
use core::marker::PhantomData;

pub struct GameWindow;

static mut SCREEN: UnsafeCell<v1::Screen> = UnsafeCell::new(v1::Screen::new());

pub struct GamePresenterImpl {
    _phantom: PhantomData<()>,
}

impl v1::GamePresenter for GamePresenterImpl {
    #[inline]
    fn screen<'a>(&'a self) -> &'a mut v1::Screen {
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
    fn move_sprite(&self, _index: v1::SpriteIndex, _origin: Point) {
        todo!()
    }

    #[inline]
    fn load_font(&self, _start_index: v1::TileIndex, _start_char: u8, _end_char: u8) {
        todo!()
    }
}
