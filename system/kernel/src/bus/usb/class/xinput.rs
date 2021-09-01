//! XInput Class Driver

use super::super::*;
use crate::{
    io::hid::{GameInput, GameInputManager},
    sync::RwLock,
    task::scheduler::SpawnOption,
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

pub struct XInput;

impl XInput {
    pub fn start(device: &UsbDevice, if_no: UsbInterfaceNumber) {
        let addr = device.addr();
        let interface = device
            .current_configuration()
            .find_interface(if_no, None)
            .unwrap();
        let class = interface.class();
        let endpoint = interface.endpoints().first().unwrap();
        let ep = endpoint.address();
        let ps = endpoint.descriptor().max_packet_size();
        device
            .host()
            .configure_endpoint(endpoint.descriptor())
            .unwrap();

        SpawnOption::new().spawn(
            move || {
                XInputDevice::_xinput_thread(addr, if_no, ep, class, ps);
            },
            "usb.xinput",
        );
    }
}

pub struct XInputDevice {
    //
}

impl XInputDevice {
    pub fn _xinput_thread(
        addr: UsbDeviceAddress,
        if_no: UsbInterfaceNumber,
        ep: UsbEndpointAddress,
        _class: UsbClass,
        ps: u16,
    ) {
        let device = UsbManager::device_by_addr(addr).unwrap();

        println!(
            "XINPUT {} IF#{} EP {:02x} PS {}",
            addr.0.get(),
            if_no.0,
            ep.0,
            ps
        );

        let input = Arc::new(RwLock::new(GameInput::empty()));
        let _handle = GameInputManager::connect_new_input(input.clone());
        let buffer = [0u8; 512];
        loop {
            match device
                .host()
                .read(ep, &buffer[0] as *const _ as *mut u8, ps as usize)
            {
                Ok(_len) => {
                    if buffer[0] == XInputMsg14::VALID_TYPE && buffer[1] == XInputMsg14::VALID_LEN {
                        let data = unsafe { *(&buffer[2] as *const _ as *const GameInput) };
                        input.write().unwrap().copy_from(&data);
                    }
                }
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
}
