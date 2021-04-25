// TeleTypewriter

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
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

impl dyn Tty {
    pub async fn read_line_async(&mut self, max_length: usize) -> Result<String, TtyError> {
        let mut buffer: Vec<char> = Vec::with_capacity(max_length);
        loop {
            self.set_cursor_enabled(true);
            match self.read_async().await {
                Ok(c) => {
                    self.set_cursor_enabled(false);
                    match c {
                        '\r' | '\n' => {
                            self.write_str("\r\n").unwrap();
                            break;
                        }
                        '\x03' => return Err(TtyError::EndOfStream),
                        '\x08' => match buffer.pop() {
                            Some(c) => {
                                if c < ' ' {
                                    self.write_str("\x08\x08  \x08\x08").unwrap();
                                } else {
                                    self.write_str("\x08 \x08").unwrap();
                                }
                            }
                            None => (),
                        },
                        _ => {
                            if buffer.len() < max_length {
                                if c < ' ' {
                                    // Control char
                                    self.write_char('^').unwrap();
                                    self.write_char((c as u8 | 0x40) as char).unwrap();
                                    buffer.push(c);
                                } else if c < '\x7F' {
                                    // Printable ascii
                                    self.write_char(c).unwrap();
                                    buffer.push(c);
                                } else {
                                    // TODO:
                                }
                            }
                        }
                    }
                }
                Err(TtyError::EndOfStream) => return Err(TtyError::EndOfStream),
                Err(_) => (),
            }
        }
        Ok(buffer.as_slice().iter().clone().collect::<String>())
    }
}

#[derive(Debug)]
pub enum TtyError {
    NotReady,
    DeviceError,
    EndOfStream,
}

pub type TtyReadResult = Result<char, TtyError>;

pub struct CombinedTty<'a> {
    stdout: &'a mut dyn Tty,
    stdin: &'a mut dyn Tty,
}

impl<'a> CombinedTty<'a> {
    pub fn new(stdout: &'a mut dyn Tty, stdin: &'a mut dyn Tty) -> Self {
        Self { stdout, stdin }
    }
}

impl Write for CombinedTty<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.stdout.write_str(s)
    }
}

impl TtyWrite for CombinedTty<'_> {
    #[inline]
    fn reset(&mut self) -> Result<(), TtyError> {
        self.stdout.reset()
    }

    #[inline]
    fn dims(&self) -> (isize, isize) {
        self.stdout.dims()
    }

    #[inline]
    fn cursor_position(&self) -> (isize, isize) {
        self.stdout.cursor_position()
    }

    #[inline]
    fn set_cursor_position(&mut self, x: isize, y: isize) {
        self.stdout.set_cursor_position(x, y)
    }

    #[inline]
    fn is_cursor_enabled(&self) -> bool {
        self.stdout.is_cursor_enabled()
    }

    #[inline]
    fn set_cursor_enabled(&mut self, enabled: bool) -> bool {
        self.stdout.set_cursor_enabled(enabled)
    }

    #[inline]
    fn attribute(&self) -> u8 {
        self.stdout.attribute()
    }

    #[inline]
    fn set_attribute(&mut self, attribute: u8) {
        self.stdout.set_attribute(attribute)
    }
}

impl TtyRead for CombinedTty<'_> {
    #[inline]
    fn read_async(&self) -> Pin<Box<dyn Future<Output = TtyReadResult> + '_>> {
        self.stdin.read_async()
    }
}

impl Tty for CombinedTty<'_> {}
