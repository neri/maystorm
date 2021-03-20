// Serial Port (UART)

use super::apic::*;
use super::cpu::*;
use crate::dev::uart::*;
use crate::io::tty::*;
use alloc::boxed::Box;
use alloc::vec::*;

static mut COM_PORTS: Vec<Box<dyn Uart>> = Vec::new();

#[derive(Debug, Copy, Clone)]
pub(super) struct ComPort {
    port: usize,
    base: u16,
    irq: Irq,
}

impl ComPort {
    const TEMPLATES: [ComPort; 4] = [
        ComPort::define(1, 0x3F8, Irq::LPC_COM1),
        ComPort::define(2, 0x2F8, Irq::LPC_COM2),
        ComPort::define(3, 0x3E8, Irq::LPC_COM1),
        ComPort::define(4, 0x2E8, Irq::LPC_COM2),
    ];

    const fn define(port: usize, base: u16, irq: Irq) -> Self {
        Self { port, base, irq }
    }

    pub unsafe fn init_first() -> Option<&'static Box<dyn Uart>> {
        for port in Self::TEMPLATES.iter() {
            if port.exists() {
                COM_PORTS.push(Box::new(*port));
            }
        }
        COM_PORTS.first()
    }

    pub unsafe fn late_init() {
        if COM_PORTS.len() == 0 {
            Self::init_first();
        }
    }

    pub fn ports<'a>() -> &'a [Box<dyn Uart>] {
        unsafe { COM_PORTS.as_slice() }
    }

    unsafe fn exists(&self) -> bool {
        for data in [0xFFu8, 0x00, 0x55, 0xAA].iter() {
            self.write_reg(7, *data);
            let result = self.read_reg(7);
            if *data != result {
                return false;
            }
        }
        true
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

impl Uart for ComPort {
    fn write(&self, data: u8) -> Result<(), TtyError> {
        unsafe {
            if (self.read_reg(5) & 0x20) == 0 {
                Err(TtyError::NotReady)
            } else {
                self.write_reg(0, data as u8);
                Ok(())
            }
        }
    }

    fn read(&self) -> Result<u8, TtyError> {
        unsafe {
            if (self.read_reg(5) & 0x01) == 0 {
                Err(TtyError::NotReady)
            } else {
                Ok(self.read_reg(0))
            }
        }
    }
}
