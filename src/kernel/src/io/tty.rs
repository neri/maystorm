// TeleTypewriter

use alloc::boxed::Box;
use core::fmt::Write;
use core::future::Future;
use core::pin::Pin;

pub trait TtyWrite: Write {
    fn reset(&mut self) -> Result<(), TtyError>;

    fn dims(&self) -> (isize, isize);

    fn cursor_position(&self) -> (isize, isize);

    fn set_cursor_position(&mut self, x: isize, y: isize);

    fn is_cursor_enabled(&self) -> bool;

    fn set_cursor_enabled(&mut self, enabled: bool) -> bool;

    fn attribute(&self) -> u8;

    fn set_attribute(&mut self, attribute: u8);
}

pub trait TtyRead {
    fn read_async(&self) -> Pin<Box<dyn Future<Output = TtyReadResult> + '_>>;
}

pub trait Tty: TtyWrite + TtyRead {}

#[derive(Debug)]
pub enum TtyError {
    NotReady,
    DeviceError,
}

pub type TtyReadResult = Result<char, TtyError>;
