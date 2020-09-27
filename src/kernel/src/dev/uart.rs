// Universal Asynchronous Receiver Transmitter

use crate::io::tty::*;

pub trait Uart {
    fn write(&self, data: u8) -> Result<(), TtyError>;

    fn read(&self) -> Result<u8, TtyError>;
}
