// myos Window API

use super::*;
use crate::graphics::*;
use core::num::NonZeroUsize;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct WindowHandle(pub NonZeroUsize);

pub struct Window {
    handle: WindowHandle,
}

impl Window {
    pub fn new(s: &str, size: Size) -> Option<Self> {
        let handle = NonZeroUsize::new(os_new_window(s, size.width as usize, size.height as usize))
            .map(|v| WindowHandle(v));
        handle.map(|handle| Self { handle })
    }

    pub const fn handle(&self) -> WindowHandle {
        self.handle
    }
}
