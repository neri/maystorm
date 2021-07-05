//! Theme Manager

use megstd::drawing::SomeColor;

static THEME: Theme = Theme::new();

/// Theme Manager
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
        SomeColor::from_rgb(0x2196F3)
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
    pub const fn window_default_background(&self) -> SomeColor {
        SomeColor::WHITE
    }

    #[inline]
    pub const fn window_default_foreground(&self) -> SomeColor {
        SomeColor::BLACK
    }

    #[inline]
    pub const fn window_default_border_light(&self) -> SomeColor {
        SomeColor::LIGHT_GRAY
    }

    #[inline]
    pub const fn window_default_border_dark(&self) -> SomeColor {
        SomeColor::DARK_GRAY
    }

    #[inline]
    pub const fn window_title_close_foreground(&self) -> SomeColor {
        SomeColor::BLACK
    }

    #[inline]
    pub const fn window_title_close_active_foreground(&self) -> SomeColor {
        SomeColor::WHITE
    }

    #[inline]
    pub const fn window_title_close_active_background(&self) -> SomeColor {
        SomeColor::LIGHT_RED
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
        SomeColor::from_rgb(0xEEEEEE)
    }

    #[inline]
    pub const fn window_title_inactive_foreground(&self) -> SomeColor {
        SomeColor::LIGHT_GRAY
    }
}
