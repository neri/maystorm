//! Theme Manager

use megstd::drawing::SomeColor;

static THEME: Theme = Theme::new();

pub struct Theme {
    _phantom: (),
}

impl Theme {
    #[inline]
    const fn new() -> Self {
        Self { _phantom: () }
    }

    #[inline]
    pub fn shared<'a>() -> &'a Self {
        &THEME
    }

    #[inline]
    pub const fn desktop_color(&self) -> SomeColor {
        SomeColor::from_argb(0xFF2196F3)
    }

    #[inline]
    pub const fn status_bar_background(&self) -> SomeColor {
        SomeColor::from_argb(0xC0ECEFF1)
    }
    #[inline]
    pub const fn status_bar_foreground(&self) -> SomeColor {
        SomeColor::BLACK
    }

    #[inline]
    pub const fn window_title_close(&self) -> SomeColor {
        SomeColor::BLACK
    }

    #[inline]
    pub const fn window_title_active_background(&self) -> SomeColor {
        SomeColor::from_argb(0xE0ECEFF1)
    }

    #[inline]
    pub const fn window_title_active_foreground(&self) -> SomeColor {
        SomeColor::BLACK
    }
    #[inline]
    pub const fn window_title_active_shadow(&self) -> SomeColor {
        SomeColor::from_argb(0x80babdbe)
    }

    #[inline]
    pub const fn window_title_inactive_background(&self) -> SomeColor {
        SomeColor::from_argb(0xFFEEEEEE)
    }

    #[inline]
    pub const fn window_title_inactive_foreground(&self) -> SomeColor {
        SomeColor::from_argb(0xFF9E9E9E)
    }
}
