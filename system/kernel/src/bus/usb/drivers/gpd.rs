//! USB Driver for GPD (expr)

use super::super::*;
use crate::{io::hid::*, task::Task};
use alloc::sync::Arc;
use megstd::io::hid::MouseButton;

pub struct GpdUsbStarter;

impl GpdUsbStarter {
    #[inline]
    pub fn new() -> Arc<dyn UsbClassDriverStarter> {
        Arc::new(Self {})
    }
}

impl UsbClassDriverStarter for GpdUsbStarter {
    fn instantiate(&self, device: &UsbDevice) -> bool {
        match device.vid_pid_raw() {
            // Mysterious HID device on GPD MicroPC
            (0x6080, 0x8061) => (),
            _ => return false,
        }
        let addr = device.addr();
        let config = device.current_configuration();
        let interface = match config.find_interface(UsbInterfaceNumber(1), None) {
            Some(v) => v,
            None => return false,
        };
        let endpoint = match interface.endpoints().first() {
            Some(v) => v,
            None => return false,
        };
        let ep = endpoint.address();
        let ps = endpoint.descriptor().max_packet_size();
        device
            .host()
            .configure_endpoint(endpoint.descriptor())
            .unwrap();

        UsbManager::register_xfer_task(Task::new(GpdHidDriver::_gpd_hid_task(addr, ep, ps)));

        true
    }
}

struct GpdHidDriver;

impl GpdHidDriver {
    async fn _gpd_hid_task(addr: UsbDeviceAddress, ep: UsbEndpointAddress, ps: u16) {
        let device = UsbManager::device_by_addr(addr).unwrap();
        let mut buffer = [0u8; 64];
        let mut mouse_state = MouseState::empty();
        loop {
            match device.read_slice(ep, &mut buffer, 4, ps as usize).await {
                Ok(_) => {
                    if buffer[0] == 0x01 {
                        let report = MouseReportRaw {
                            buttons: MouseButton::from_bits_truncate(buffer[1]),
                            x: buffer[2] as i8,
                            y: buffer[3] as i8,
                        };
                        mouse_state.process_mouse_report(report);
                    }
                }
                Err(_err) => {
                    // TODO: error
                }
            }
        }
    }
}
