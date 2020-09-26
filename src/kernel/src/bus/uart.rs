// Universal Asynchronous Receiver Transmitter

use crate::io::tty::*;
use core::fmt;

pub trait Uart: fmt::Write {
    fn write(&self, data: u8) -> Result<(), TtyError>;

    fn read(&self) -> Result<u8, TtyError>;

    fn to_write<'a>(&self) -> &'a mut dyn Uart;
}
