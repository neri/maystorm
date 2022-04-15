//! USB Mass Storage Device (Bulk Only Transfer) (08_06_50)

use super::super::*;
use crate::{task::Task, *};
use alloc::sync::Arc;
use core::pin::Pin;
use futures_util::Future;

pub struct UsbMsdStarter;

impl UsbMsdStarter {
    #[inline]
    pub fn new() -> Arc<dyn UsbInterfaceDriverStarter> {
        Arc::new(Self {})
    }
}

impl UsbInterfaceDriverStarter for UsbMsdStarter {
    fn instantiate(
        &self,
        device: &Arc<UsbDeviceControl>,
        if_no: UsbInterfaceNumber,
    ) -> Pin<Box<dyn Future<Output = Result<Task, UsbError>>>> {
        Box::pin(UsbMsdDriver::_instantiate(device.clone(), if_no))
    }
}

pub struct UsbMsdDriver {
    //
}

impl UsbMsdDriver {
    async fn _instantiate(
        device: Arc<UsbDeviceControl>,
        if_no: UsbInterfaceNumber,
    ) -> Result<Task, UsbError> {
        let interface = match device
            .device()
            .current_configuration()
            .find_interface(if_no, None)
        {
            Some(v) => v,
            None => return Err(UsbError::InvalidParameter),
        };
        if interface.class() != UsbClass::MSD_BULK_ONLY {
            return Err(UsbError::Unsupported);
        }
        let endpoint = match interface.endpoints().first() {
            Some(v) => v,
            None => todo!(),
        };
        let ep = endpoint.address();
        let ps = endpoint.descriptor().max_packet_size();

        device.configure_endpoint(endpoint.descriptor()).unwrap();

        Ok(Task::new(Self::_usb_msd_task(
            device.clone(),
            if_no,
            ep,
            ps,
        )))
    }

    async fn _usb_msd_task(
        device: Arc<UsbDeviceControl>,
        if_no: UsbInterfaceNumber,
        _ep: UsbEndpointAddress,
        _ps: u16,
    ) {
        let _max_lun = Self::get_max_lun(&device, if_no).await.unwrap();
        // log!("MAX_LUN {}", max_lun);
    }

    async fn get_max_lun(
        device: &UsbDeviceControl,
        if_no: UsbInterfaceNumber,
    ) -> Result<u8, UsbError> {
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
            .await
            .map(|_| result[0])
    }
}
