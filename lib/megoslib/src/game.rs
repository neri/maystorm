//! Retro Game Framework v1

pub mod v1 {

    pub mod prelude {
        pub use super::{GamePresenterImpl, GameWindow};
        pub use megstd::drawing::*;
        pub use megstd::game::v1;
        pub use megstd::game::v1::GamePresenter;
    }

    use crate::window;
    use crate::{syscall::*, window::WindowHandle};
    use core::cell::UnsafeCell;
    use core::ffi::c_void;
    use megstd::drawing::*;
    use megstd::game::v1;

    pub struct GameWindow;

    impl GameWindow {
        #[inline]
        pub fn new(title: &str, size: Size) -> GamePresenterImpl {
            let window = window::WindowBuilder::new()
                .opaque()
                .size(size)
                .build(title);
            GamePresenterImpl::new(window.handle())
        }

        #[inline]
        pub fn with_options(
            title: &str,
            size: Size,
            scale: v1::ScaleMode,
            fps: usize,
        ) -> GamePresenterImpl {
            let window = window::WindowBuilder::new()
                .opaque()
                .size(size * scale.scale_factor() as isize)
                .build(title);
            GamePresenterImpl::new_long(window.handle(), scale, fps)
        }
    }

    static mut SCREEN: UnsafeCell<v1::Screen> = UnsafeCell::new(v1::Screen::new());

    pub struct GamePresenterImpl {
        game_handle: usize,
    }

    impl GamePresenterImpl {
        #[inline]
        fn new(window: WindowHandle) -> Self {
            let game_handle = unsafe { game_v1_init(window.0, SCREEN.get() as *const c_void) };
            Self { game_handle }
        }

        #[inline]
        fn new_long(window: WindowHandle, scale: v1::ScaleMode, fps: usize) -> Self {
            let game_handle = unsafe {
                game_v1_init_long(window.0, SCREEN.get() as *const c_void, scale as usize, fps)
            };
            Self { game_handle }
        }
    }

    impl v1::GamePresenter for GamePresenterImpl {
        #[inline]
        fn screen<'a>(&'a self) -> &'a mut v1::Screen {
            unsafe { &mut *SCREEN.get() }
        }

        #[inline]
        fn buttons(&self) -> u8 {
            game_v1_button(self.game_handle)
        }

        #[inline]
        fn sync(&self) -> usize {
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
        fn move_sprite(&self, index: v1::SpriteIndex, origin: Point) {
            game_v1_move_sprite(
                self.game_handle,
                index as usize,
                origin.x as usize,
                origin.y as usize,
            );
        }

        #[inline]
        fn load_font(&self, start_index: v1::TileIndex, start_char: u8, end_char: u8) {
            game_v1_load_font(
                self.game_handle,
                start_index as usize,
                start_char as usize,
                end_char as usize,
            );
        }
    }
}
