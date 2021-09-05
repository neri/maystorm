//! XInput Class Driver

use super::super::*;
use crate::{
    io::hid::{GameInput, GameInputManager},
    sync::RwLock,
    task::Task,
};
use alloc::sync::Arc;

// for debug
use crate::System;
use core::fmt::Write;

macro_rules! print {
    ($($arg:tt)*) => {
        write!(System::em_console(), $($arg)*).unwrap()
    };
}

macro_rules! println {
    ($fmt:expr) => {
        print!(concat!($fmt, "\r\n"))
    };
    ($fmt:expr, $($arg:tt)*) => {
        print!(concat!($fmt, "\r\n"), $($arg)*)
    };
}

pub struct XInputStarter;

impl XInputStarter {
    #[inline]
    pub fn new() -> Arc<dyn UsbInterfaceDriverStarter> {
        Arc::new(Self {})
    }
}

impl UsbInterfaceDriverStarter for XInputStarter {
    fn instantiate(&self, device: &UsbDevice, interface: &UsbInterface) -> bool {
        let class = interface.class();
        if class != UsbClass::XINPUT {
            return false;
        }
        let addr = device.addr();
        let if_no = interface.if_no();
        let endpoint = interface.endpoints().first().unwrap();
        let ep = endpoint.address();
        let ps = endpoint.descriptor().max_packet_size();
        device
            .host()
            .configure_endpoint(endpoint.descriptor())
            .unwrap();

        UsbManager::register_xfer_task(Task::new(XInputDriver::_xinput_task(
            addr, if_no, ep, class, ps,
        )));

        true
    }
}

struct XInputDriver;

impl XInputDriver {
    async fn _xinput_task(
        addr: UsbDeviceAddress,
        _if_no: UsbInterfaceNumber,
        ep: UsbEndpointAddress,
        _class: UsbClass,
        ps: u16,
    ) {
        let device = UsbManager::device_by_addr(addr).unwrap();
        let input = Arc::new(RwLock::new(GameInput::empty()));
        let _handle = GameInputManager::connect_new_input(input.clone());
        let mut buffer = [0u8; 512];
        loop {
            match device
                .read_slice(ep, &mut buffer, XInputMsg14::MIN_LEN as usize, ps as usize)
                .await
            {
                Ok(_) => {
                    if buffer[0] == XInputMsg14::VALID_TYPE && buffer[1] == XInputMsg14::VALID_LEN {
                        let data = unsafe { *(&buffer[2] as *const _ as *const GameInput) };
                        input.write().unwrap().copy_from(&data);
                    }
                }
                Err(UsbError::Aborted) => break,
                Err(err) => {
                    println!("XINPUT READ ERROR {:?} {:?}", addr.0.get(), err);
                }
            }
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct XInputMsg14 {
    _type: u8,
    len: u8,
    button1: u8,
    button2: u8,
    lt: u8,
    rt: u8,
    x1: u16,
    y1: u16,
    x2: u16,
    y2: u16,
}

impl XInputMsg14 {
    pub const VALID_TYPE: u8 = 0;
    pub const VALID_LEN: u8 = 0x14;
    pub const MIN_LEN: usize = 14;
}
