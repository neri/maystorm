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

        device
            .host()
            .configure_endpoint(endpoint.descriptor())
            .unwrap();

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
        // todo!()
    }
}
