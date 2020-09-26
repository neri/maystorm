// TeleTypewriter

use core::fmt::Write;

pub trait Tty: Write {
    fn reset(&mut self) -> Result<(), TtyError>;
    fn dims(&self) -> (isize, isize);
    fn cursor_position(&self) -> (isize, isize);
    fn set_cursor_position(&mut self, x: isize, y: isize);
    fn is_cursor_enabled(&self) -> bool;
    fn set_cursor_enabled(&mut self, enabled: bool) -> bool;
    fn attribute(&self) -> u8;
    fn set_attribute(&mut self, attribute: u8);
}

#[derive(Debug)]
pub enum TtyError {
    NotReady,
    DeviceError,
}
