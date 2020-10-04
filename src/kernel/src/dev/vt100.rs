// VT100

use crate::dev::uart::*;
use crate::io::tty::*;
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::fmt::Write;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

pub struct Vt100<'a> {
    uart: &'a Box<dyn Uart>,
}

impl<'a> Vt100<'a> {
    pub fn with_uart(uart: &'a Box<dyn Uart>) -> Self {
        Self { uart }
    }
}

impl Vt100<'_> {
    pub fn output_str(&self, s: &str) -> Result<(), TtyError> {
        for c in s.bytes() {
            match self.uart.write(c) {
                Ok(_) => (),
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }
}

impl Write for Vt100<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.output_str(s).map_err(|_| core::fmt::Error)
    }
}

impl TtyWrite for Vt100<'_> {
    fn reset(&mut self) -> Result<(), TtyError> {
        //self.output_str("\x1bc");
        self.output_str("\x1b[H\x1b[2J")
    }

    fn dims(&self) -> (isize, isize) {
        // TODO:
        (80, 24)
    }

    fn cursor_position(&self) -> (isize, isize) {
        todo!()
    }

    fn set_cursor_position(&mut self, x: isize, y: isize) {
        write!(self, "\x1b[{};{}H", y + 1, x + 1).unwrap();
    }

    fn is_cursor_enabled(&self) -> bool {
        false
    }

    fn set_cursor_enabled(&mut self, enabled: bool) -> bool {
        let _ = enabled;
        false
    }

    fn attribute(&self) -> u8 {
        0
    }

    fn set_attribute(&mut self, attribute: u8) {
        let _ = attribute;
        // TODO:
    }
}

impl TtyRead for Vt100<'_> {
    fn read_async(&self) -> Pin<Box<dyn Future<Output = TtyReadResult> + '_>> {
        Box::pin(Vt100Reader { vt: Arc::new(self) })
    }
}

impl Tty for Vt100<'_> {}

struct Vt100Reader<'a> {
    vt: Arc<&'a Vt100<'a>>,
}

impl Future for Vt100Reader<'_> {
    type Output = TtyReadResult;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.vt.uart.read() {
            Ok(c) => Poll::Ready(Ok(c as char)),
            Err(TtyError::NotReady) => Poll::Pending,
            Err(err) => Poll::Ready(Err(err)),
        }
    }
}
