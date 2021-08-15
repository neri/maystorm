//! Retro Game Framework

pub mod v1 {
    use crate::window;
    use crate::{syscall::*, window::WindowHandle};
    use core::cell::UnsafeCell;
    use core::ffi::c_void;
    use megstd::drawing::*;
    use megstd::game::v1;

    pub struct GameWindow;

    impl GameWindow {
        #[inline]
        pub fn new<'a>(
            title: &str,
            size: Size,
            screen: &'a UnsafeCell<v1::Screen>,
        ) -> GamePresenterImpl<'a> {
            Self::with_options(title, size, screen, v1::ScaleMode::DotByDot, 60)
        }

        #[inline]
        pub fn with_options<'a>(
            title: &str,
            size: Size,
            screen: &'a UnsafeCell<v1::Screen>,
            scale: v1::ScaleMode,
            fps: usize,
        ) -> GamePresenterImpl<'a> {
            let window = window::WindowBuilder::new()
                .opaque()
                .size(size)
                .build(title);
            GamePresenterImpl::new(window.handle(), screen, scale, fps)
        }
    }

    pub struct GamePresenterImpl<'a> {
        game_handle: usize,
        screen: &'a UnsafeCell<v1::Screen>,
    }

    impl<'a> GamePresenterImpl<'a> {
        #[inline]
        fn new(
            window: WindowHandle,
            screen: &'a UnsafeCell<v1::Screen>,
            scale: v1::ScaleMode,
            fps: usize,
        ) -> Self {
            let game_handle = unsafe {
                game_v1_init(window.0, screen.get() as *const c_void, scale as usize, fps)
            };
            Self {
                game_handle,
                screen,
            }
        }
    }

    impl v1::GamePresenter for GamePresenterImpl<'_> {
        #[inline]
        fn screen<'a>(&'a self) -> &'a mut v1::Screen {
            unsafe { &mut *self.screen.get() }
        }

        #[inline]
        fn buttons(&self) -> u8 {
            game_v1_button(self.game_handle)
        }

        #[inline]
        fn sync(&self) -> bool {
            game_v1_sync(self.game_handle)
        }

        #[inline]
        fn display_if_needed(&self) {
            game_v1_redraw(self.game_handle);
        }

        #[inline]
        fn set_needs_display(&self) {
            game_v1_rect(
                self.game_handle,
                0,
                0,
                v1::MAX_WIDTH as usize,
                v1::MAX_HEIGHT as usize,
            );
        }

        #[inline]
        fn invalidate_rect(&self, rect: Rect) {
            game_v1_rect(
                self.game_handle,
                rect.x() as usize,
                rect.y() as usize,
                rect.width() as usize,
                rect.height() as usize,
            );
        }

        #[inline]
        fn move_sprite(&self, index: v1::PatternIndex, origin: Point) {
            game_v1_move_sprite(
                self.game_handle,
                index as usize,
                origin.x as usize,
                origin.y as usize,
            );
        }
    }
}
