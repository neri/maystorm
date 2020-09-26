// Serial Port (UART)

use super::cpu::*;
use crate::bus::uart::*;
use crate::io::tty::*;
use alloc::boxed::Box;
use alloc::vec::*;
use core::fmt;

#[derive(Debug, Copy, Clone)]
pub struct SerialPort {
    base: u16,
}

impl SerialPort {
    pub const PORTS: [u16; 4] = [0x3F8, 0x2F8, 0x3E8, 0x2E8];

    pub(super) unsafe fn all_ports() -> Vec<Box<dyn Uart>> {
        let mut vec: Vec<Box<dyn Uart>> = Vec::with_capacity(Self::PORTS.len());
        for port in Self::PORTS.iter() {
            if let Some(item) = Self::with_port(*port) {
                vec.push(Box::new(item));
            }
        }
        vec
    }

    pub(super) unsafe fn with_port(base: u16) -> Option<Self> {
        let temp = Self { base };
        for data in [0xFFu8, 0x00, 0x55, 0xAA].iter() {
            temp.write_reg(7, *data);
            let result = temp.read_reg(7);
            if *data != result {
                return None;
            }
        }
        Some(temp)
    }

    #[inline]
    pub unsafe fn write_reg(&self, reg: u16, data: u8) {
        Cpu::out8(self.base + reg, data);
    }

    #[inline]
    pub unsafe fn read_reg(&self, reg: u16) -> u8 {
        Cpu::in8(self.base + reg)
    }
}

impl fmt::Write for SerialPort {
    fn write_char(&mut self, c: char) -> fmt::Result {
        self.write(c as u8).map_err(|_| fmt::Error)
    }
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            let _ = self.write_char(c);
        }
        Ok(())
    }
}

impl Uart for SerialPort {
    fn to_write<'a>(&self) -> &'a mut dyn Uart {
        #[allow(mutable_transmutes)]
        unsafe {
            core::mem::transmute(self as &dyn Uart)
        }
    }

    fn write(&self, data: u8) -> Result<(), TtyError> {
        unsafe {
            self.write_reg(0, data as u8);
        }
        Ok(())
    }

    fn read(&self) -> Result<u8, TtyError> {
        Ok(unsafe { self.read_reg(0) })
    }
}
