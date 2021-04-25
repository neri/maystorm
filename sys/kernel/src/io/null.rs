// Null Device

use super::tty::*;
use alloc::boxed::Box;
use core::{
    fmt::Write,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

// Null is singleton
static mut SINGLETON: Null = Null::new();

pub struct Null {}

impl Null {
    const fn new() -> Self {
        Self {}
    }
    pub fn null() -> &'static mut dyn Tty {
        unsafe { &mut SINGLETON }
    }
}

impl Write for Null {
    fn write_str(&mut self, _s: &str) -> core::fmt::Result {
        Ok(())
    }
}

impl TtyWrite for Null {
    fn reset(&mut self) -> Result<(), TtyError> {
        Ok(())
    }

    fn dims(&self) -> (isize, isize) {
        (0, 0)
    }

    fn cursor_position(&self) -> (isize, isize) {
        (0, 0)
    }

    fn set_cursor_position(&mut self, _x: isize, _y: isize) {}

    fn is_cursor_enabled(&self) -> bool {
        false
    }

    fn set_cursor_enabled(&mut self, _enabled: bool) -> bool {
        false
    }

    fn attribute(&self) -> u8 {
        0
    }

    fn set_attribute(&mut self, _attribute: u8) {}
}

impl TtyRead for Null {
    fn read_async(
        &self,
    ) -> core::pin::Pin<Box<dyn core::future::Future<Output = TtyReadResult> + '_>> {
        Box::pin(NullReader {})
    }
}

impl Tty for Null {}

struct NullReader {}

impl Future for NullReader {
    type Output = TtyReadResult;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(Err(TtyError::EndOfStream))
    }
}
