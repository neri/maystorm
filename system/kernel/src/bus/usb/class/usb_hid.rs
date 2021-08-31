//! USB-HID Class Driver

use super::super::*;
use crate::io::hid::*;
use crate::task::scheduler::{SpawnOption, Timer};
use alloc::format;
use alloc::sync::Arc;
use megstd::io::hid::MouseButton;
use megstd::string::Sb255;

// for debug
use crate::System;
use core::fmt::Write;
use core::time::Duration;

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

pub struct UsbHid {
    //
}

impl UsbHid {
    pub fn start(device: Arc<UsbDevice>, if_no: u8) {
        SpawnOption::new().spawn(
            move || {
                Self::_usb_hid_thread(device, if_no);
            },
            "usb.hid",
        );
    }

    fn set_boot_protocol(device: &UsbDevice, if_no: u8, is_boot: bool) -> Result<(), UsbError> {
        device
            .host()
            .control(UsbControlSetupData {
                bmRequestType: UsbControlRequestBitmap(0x21),
                bRequest: UsbControlRequest::HID_SET_PROTOCOL,
                wValue: (!is_boot) as u16,
                wIndex: if_no as u16,
                wLength: 0,
            })
            .map(|_| ())
    }

    fn _usb_hid_thread(device: Arc<UsbDevice>, if_no: u8) {
        let addr = device.addr();
        let props = device.props();
        let interface = props.interface(if_no).unwrap();
        let endpoint = props
            .endpoint_by_bitmap(interface.endpoint_bitmap(), true)
            .unwrap();
        let ep = endpoint.endpoint_address().unwrap();
        let ps = endpoint.max_packet_size();

        match interface.descriptor().class() {
            UsbClass::HID_BIOS_KEYBOARD => {
                println!(
                    "USB KEYBOARD {} IF#{} EP {:08x} {:02x}",
                    addr.0.get(),
                    if_no,
                    interface.endpoint_bitmap(),
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
                            println!("HID READ ERROR {:?}", err);
                        }
                        _ => (),
                    }
                }
            }
            UsbClass::HID_BIOS_MOUSE => {
                println!(
                    "USB MOUSE {} IF#{} EP {:08x} {:02x}",
                    addr.0.get(),
                    if_no,
                    interface.endpoint_bitmap(),
                    ep.0
                );

                Self::set_boot_protocol(&device, if_no, true).unwrap();

                let mut buffer = 0u64;
                let mut mouse_state = MouseState::empty();
                loop {
                    let buffer = &buffer as *const _ as *mut u8;
                    match device.host().read(ep, buffer, ps as usize) {
                        Ok(size) => {
                            let report = unsafe { (buffer as *const MouseReportRaw).read() };
                            mouse_state.process_mouse_report(report);
                        }
                        Err(err) => {
                            println!("HID READ ERROR {:?}", err);
                        }
                        _ => (),
                    }
                }
            }
            _ => {}
        }
    }
}
