//! USB Mass Storage Device (Bulk Only Transfer) (08_06_50)

use super::super::*;
use crate::{
    task::{scheduler::Timer, Task},
    *,
};
use alloc::{boxed::Box, collections::BTreeMap, sync::Arc, vec::Vec};
use core::{num::NonZeroU8, time::Duration};
// use num_traits::FromPrimitive;

pub struct UsbMsdStarter;

impl UsbMsdStarter {
    #[inline]
    pub fn new() -> Arc<dyn UsbInterfaceDriverStarter> {
        Arc::new(Self {})
    }
}

impl UsbInterfaceDriverStarter for UsbMsdStarter {
    fn instantiate(&self, device: &UsbDevice, interface: &UsbInterface) -> bool {
        if interface.class() != UsbClass::MSD_BULK_ONLY {
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

        device.configure_endpoint(endpoint.descriptor()).unwrap();

        UsbManager::register_xfer_task(Task::new(UsbMsdDriver::_usb_msd_task(addr, if_no, ep, ps)));

        true
    }
}

pub struct UsbMsdDriver {
    //
}

impl UsbMsdDriver {
    async fn _usb_msd_task(
        addr: UsbDeviceAddress,
        if_no: UsbInterfaceNumber,
        ep: UsbEndpointAddress,
        ps: u16,
    ) {
        let device = UsbManager::device_by_addr(addr).unwrap();

        let max_lun = Self::get_max_lun(&device, if_no).unwrap();
        // log!("MAX_LUN {}", max_lun);
    }

    fn get_max_lun(device: &UsbDevice, if_no: UsbInterfaceNumber) -> Result<u8, UsbError> {
        let mut result = [0; 1];
        device
            .control_slice(
                UsbControlSetupData::request(
                    UsbControlRequestBitmap(0xA1),
                    UsbControlRequest(0xFE),
                )
                .index_if(if_no)
                .length(1),
                &mut result,
            )
            .map(|_| result[0])
    }
}
