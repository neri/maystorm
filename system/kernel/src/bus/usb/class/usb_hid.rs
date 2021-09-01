//! USB-HID Class Driver

use super::super::*;
use crate::io::hid::*;
use crate::task::scheduler::SpawnOption;

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

pub struct UsbHid;

impl UsbHid {
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
        if ps > 8 {
            // There is probably no HID device with a packet size larger than 8 bytes
            return;
        }
        device
            .host()
            .configure_endpoint(endpoint.descriptor())
            .unwrap();

        SpawnOption::new().spawn(
            move || {
                UsbHidDevice::_usb_hid_thread(addr, if_no, ep, class, ps);
            },
            "usb.hid",
        );
    }
}

pub struct UsbHidDevice;

impl UsbHidDevice {
    fn _usb_hid_thread(
        addr: UsbDeviceAddress,
        if_no: UsbInterfaceNumber,
        ep: UsbEndpointAddress,
        class: UsbClass,
        ps: u16,
    ) {
        let device = UsbManager::device_by_addr(addr).unwrap();
        match class {
            UsbClass::HID_BIOS_KEYBOARD => {
                println!(
                    "USB HID BIOS KEYBOARD {} IF#{} EP {:02x}",
                    addr.0.get(),
                    if_no.0,
                    ep.0
                );
                Self::set_boot_protocol(&device, if_no, true).unwrap();
                let mut key_state = KeyboardState::new();
                let mut buffer = KeyReportRaw::default();
                loop {
                    match device.read(ep, &mut buffer) {
                        Ok(8) => {
                            key_state.process_key_report(buffer);
                        }
                        Err(err) => {
                            println!("HID KEYBOARD READ ERROR {:?} {:?}", addr.0.get(), err);
                        }
                        _ => (),
                    }
                }
            }
            UsbClass::HID_BIOS_MOUSE => {
                println!(
                    "USB HID BIOS MOUSE {} IF#{} EP {:02x}",
                    addr.0.get(),
                    if_no.0,
                    ep.0
                );
                Self::set_boot_protocol(&device, if_no, true).unwrap();
                let buffer = 0u64;
                let mut mouse_state = MouseState::empty();
                loop {
                    let buffer = &buffer as *const _ as *mut u8;
                    match device.host().read(ep, buffer, ps as usize) {
                        Ok(_size) => {
                            let report = unsafe { (buffer as *const MouseReportRaw).read() };
                            mouse_state.process_mouse_report(report);
                        }
                        Err(err) => {
                            println!("HID MOUSE READ ERROR {:?} {:?}", addr.0.get(), err);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn set_boot_protocol(
        device: &UsbDevice,
        if_no: UsbInterfaceNumber,
        is_boot: bool,
    ) -> Result<(), UsbError> {
        device
            .host()
            .control(UsbControlSetupData {
                bmRequestType: UsbControlRequestBitmap(0x21),
                bRequest: UsbControlRequest::HID_SET_PROTOCOL,
                wValue: (!is_boot) as u16,
                wIndex: if_no.0 as u16,
                wLength: 0,
            })
            .map(|_| ())
    }
}
