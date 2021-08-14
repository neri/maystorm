//! Retro Game Framework

pub mod v1 {
    use crate::{syscall::*, window::WindowHandle};
    use core::cell::UnsafeCell;
    use core::ffi::c_void;
    use megstd::drawing::*;
    use megstd::game::v1;

    pub struct GamePresenterImpl<'a> {
        window: WindowHandle,
        game_handle: usize,
        screen: &'a UnsafeCell<v1::Screen>,
    }

    impl<'a> GamePresenterImpl<'a> {
        #[inline]
        pub(crate) fn new(
            window: WindowHandle,
            screen: &'a UnsafeCell<v1::Screen>,
            scale: v1::ScaleMode,
            fps: usize,
        ) -> Self {
            let game_handle = unsafe {
                game_v1_init(window.0, screen.get() as *const c_void, scale as usize, fps)
            };
            Self {
                window,
                game_handle,
                screen,
            }
        }
    }

    impl v1::GamePresenter for GamePresenterImpl<'_> {
        fn screen<'a>(&'a mut self) -> &'a mut v1::Screen {
            unsafe { &mut *self.screen.get() }
        }

        #[inline]
        fn sync(&mut self) -> bool {
            game_v1_sync(self.game_handle)
        }

        #[inline]
        fn set_needs_display(&mut self) {
            game_v1_rect(
                self.game_handle,
                0,
                0,
                v1::MAX_WIDTH as usize,
                v1::MAX_HEIGHT as usize,
            );
        }

        #[inline]
        fn display_if_needed(&mut self) {
            game_v1_redraw(self.game_handle);
        }

        #[inline]
        fn invalidate_rect(&mut self, rect: Rect) {
            game_v1_rect(
                self.game_handle,
                rect.x() as usize,
                rect.y() as usize,
                rect.width() as usize,
                rect.height() as usize,
            );
        }

        #[inline]
        fn move_sprite(&mut self, index: v1::PatternIndex, origin: megstd::drawing::Point) {
            game_v1_move_sprite(
                self.game_handle,
                index as usize,
                origin.x as usize,
                origin.y as usize,
            );
        }
    }
}
