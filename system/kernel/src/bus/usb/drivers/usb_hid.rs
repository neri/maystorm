//! USB-HID Class Driver

use super::super::*;
use crate::{
    io::hid::*,
    task::{scheduler::Timer, Task},
};
use alloc::{sync::Arc, vec::Vec};
use core::{mem::size_of, time::Duration};
use megstd::io::hid::MouseButton;

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

pub struct UsbHidStarter;

impl UsbHidStarter {
    #[inline]
    pub fn new() -> Arc<dyn UsbInterfaceDriverStarter> {
        Arc::new(Self {})
    }
}

impl UsbInterfaceDriverStarter for UsbHidStarter {
    fn instantiate(&self, device: &UsbDevice, interface: &UsbInterface) -> bool {
        let class = interface.class();
        if class.base() != UsbBaseClass::HID {
            return false;
        }
        let addr = device.addr();
        let if_no = interface.if_no();
        let endpoint = match interface.endpoints().first() {
            Some(v) => v,
            None => todo!(),
        };
        let ep = endpoint.address();
        let ps = endpoint.descriptor().max_packet_size();
        if ps > 64 {
            return false;
        }
        device
            .host()
            .configure_endpoint(endpoint.descriptor())
            .unwrap();

        UsbManager::register_xfer_task(Task::new(UsbHidDriver::_usb_hid_task(
            addr, if_no, ep, class, ps,
        )));

        true
    }
}

struct UsbHidDriver;

impl UsbHidDriver {
    async fn _usb_hid_task(
        addr: UsbDeviceAddress,
        if_no: UsbInterfaceNumber,
        ep: UsbEndpointAddress,
        class: UsbClass,
        ps: u16,
    ) {
        let device = UsbManager::device_by_addr(addr).unwrap();
        match class {
            UsbClass::HID_BOOT_KEYBOARD => {
                match Self::set_boot_protocol(&device, if_no, true) {
                    Ok(_) => (),
                    Err(_err) => return,
                }
                // let flash_count = match device.vid_pid_raw() {
                //     (0x0603, 0x0002) => 0,
                //     _ => 2,
                // };
                // // flash keyboard led
                // for _ in 0..flash_count {
                //     let _ = Self::set_report(&device, if_no, HidReportType::Output, 0, &[7]);
                //     Timer::sleep_async(Duration::from_millis(50)).await;
                //     let _ = Self::set_report(&device, if_no, HidReportType::Output, 0, &[0]);
                //     Timer::sleep_async(Duration::from_millis(150)).await;
                // }
                let mut key_state = KeyboardState::new();
                let mut buffer = KeyReportRaw::default();
                loop {
                    match device.read(ep, &mut buffer).await {
                        Ok(_) => {
                            key_state.process_key_report(buffer);
                        }
                        Err(UsbError::Aborted) => break,
                        Err(_err) => {
                            // TODO: error
                        }
                    }
                }
            }
            UsbClass::HID_BOOT_MOUSE => {
                match Self::set_boot_protocol(&device, if_no, true) {
                    Ok(_) => (),
                    Err(_err) => return,
                }
                let mut buffer = [0u8; 64];
                let mut mouse_state = MouseState::empty();
                loop {
                    match device
                        .read_slice(ep, &mut buffer, size_of::<MouseReportRaw>(), ps as usize)
                        .await
                    {
                        Ok(_) => {
                            let report = MouseReportRaw {
                                buttons: MouseButton::from_bits_truncate(buffer[0]),
                                x: buffer[1] as i8,
                                y: buffer[2] as i8,
                            };
                            mouse_state.process_mouse_report(report);
                        }
                        Err(UsbError::Aborted) => break,
                        Err(_err) => {
                            // TODO: error
                        }
                    }
                }
            }
            _ => {
                // TODO: generic HID
                match Self::set_boot_protocol(&device, if_no, true) {
                    Ok(_) => (),
                    Err(_err) => return,
                }
                let mut buffer = [0u8; 64];
                loop {
                    match device.read_slice(ep, &mut buffer, 0, ps as usize).await {
                        Ok(_size) => {
                            // TODO:
                        }
                        Err(UsbError::Aborted) => break,
                        Err(_err) => (),
                    }
                }
            }
        }
    }

    pub fn set_boot_protocol(
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

    pub fn get_report_desc(
        device: &UsbDevice,
        if_no: UsbInterfaceNumber,
        report_type: u8,
        report_id: u8,
        len: usize,
        vec: &mut Vec<u8>,
    ) -> Result<(), UsbError> {
        match device.host().control(UsbControlSetupData {
            bmRequestType: UsbControlRequestBitmap(0x81),
            bRequest: UsbControlRequest::GET_DESCRIPTOR,
            wValue: (report_type as u16) * 256 + (report_id as u16),
            wIndex: if_no.0 as u16,
            wLength: len as u16,
        }) {
            Ok(result) => {
                vec.resize(result.len(), 0);
                vec.copy_from_slice(result);
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    pub fn set_report(
        device: &UsbDevice,
        if_no: UsbInterfaceNumber,
        report_type: HidReportType,
        report_id: u8,
        data: &[u8],
    ) -> Result<usize, UsbError> {
        device.host().control_send(
            UsbControlSetupData {
                bmRequestType: UsbControlRequestBitmap(0x21),
                bRequest: UsbControlRequest::HID_SET_REPORT,
                wValue: ((report_type as u16) << 8) | report_id as u16,
                wIndex: if_no.0 as u16,
                wLength: data.len() as u16,
            },
            data,
        )
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HidReportType {
    Input = 1,
    Output,
    Feature,
}
