// TeleTypewriter

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::*;
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
    pub async fn read_line_async(&mut self, max_length: usize) -> Option<String> {
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
                        '\x03' => return None,
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
                                    self.write_char('^').unwrap();
                                    self.write_char((c as u8 | 0x40) as char).unwrap();
                                    buffer.push(c);
                                } else if c < '\x7F' {
                                    self.write_char(c).unwrap();
                                    buffer.push(c);
                                } else {
                                    // TODO:
                                }
                            }
                        }
                    }
                }
                Err(_) => (),
            }
        }
        Some(buffer.as_slice().iter().clone().collect::<String>())
    }
}

#[derive(Debug)]
pub enum TtyError {
    NotReady,
    DeviceError,
    SkipData,
}

pub type TtyReadResult = Result<char, TtyError>;
