//! USB Hub Class Driver (09_xx_xx)

use super::super::*;
use crate::{
    task::{scheduler::Timer, Task},
    *,
};
use alloc::sync::Arc;
use bitflags::*;
use core::{mem::transmute, num::NonZeroU8, time::Duration};
use num_traits::FromPrimitive;

pub struct UsbHubStarter;

impl UsbHubStarter {
    #[inline]
    pub fn new() -> Arc<dyn UsbClassDriverStarter> {
        Arc::new(Self {})
    }
}

impl UsbClassDriverStarter for UsbHubStarter {
    fn instantiate(&self, device: &Arc<UsbDeviceControl>) -> bool {
        let class = device.device().class();
        match class {
            UsbClass::HUB_FS | UsbClass::HUB_HS_MTT | UsbClass::HUB_HS_STT | UsbClass::HUB_SS => (),
            _ => return false,
        }

        let config = device.device().current_configuration();
        let mut current_interface = None;
        for interface in config.interfaces() {
            if interface.class() == class {
                current_interface = Some(interface);
                break;
            }
        }
        let interface = match current_interface.or(config.interfaces().first()) {
            Some(v) => v,
            None => return false,
        };
        let endpoint = match interface.endpoints().first() {
            Some(v) => v,
            None => todo!(),
        };
        let ep = endpoint.address();
        let ps = endpoint.descriptor().max_packet_size();
        if ps > 8 {
            return false;
        }
        device.configure_endpoint(endpoint.descriptor()).unwrap();

        match class {
            UsbClass::HUB_FS | UsbClass::HUB_HS_MTT | UsbClass::HUB_HS_STT => {
                UsbManager::register_xfer_task(Task::new(Usb2HubDriver::_usb_hub_task(
                    device.clone(),
                    ep,
                    ps,
                )));
            }
            UsbClass::HUB_SS => {
                UsbManager::register_xfer_task(Task::new(Usb3HubDriver::_usb_hub_task(
                    device.clone(),
                    ep,
                    ps,
                )));
            }
            _ => (),
        }

        true
    }
}

pub struct Usb2HubDriver {
    device: Arc<UsbDeviceControl>,
    hub_desc: Usb2HubDescriptor,
}

impl Usb2HubDriver {
    /// USB2 Hub Task (FS, HS, HS-MTT)
    async fn _usb_hub_task(device: Arc<UsbDeviceControl>, ep: UsbEndpointAddress, ps: u16) {
        let addr = device.device().addr();
        let is_mtt = device.device().class() == UsbClass::HUB_HS_MTT;

        let hub_desc: Usb2HubDescriptor =
            match UsbHubCommon::get_hub_descriptor(&device, UsbDescriptorType::Hub, 0).await {
                Ok(v) => v,
                Err(_err) => {
                    // TODO:
                    log!("USB2 GET HUB DESCRIPTOR {:?}", _err);
                    return;
                }
            };
        match device.configure_hub2(&hub_desc, is_mtt) {
            Ok(_) => (),
            Err(_err) => {
                // TODO:
                log!("USB2 COFNIGURE HUB2 {:?}", _err);
                return;
            }
        }
        let hub = Arc::new(Usb2HubDriver {
            device: device.clone(),
            hub_desc,
        });

        let focus = device.focus_hub();
        hub.clone().init_hub().await;
        drop(focus);

        let n_ports = hub_desc.num_ports();
        let mut port_event = [0u8; 8];
        loop {
            match device.read_slice(ep, &mut port_event, 1, ps as usize).await {
                Ok(_) => {
                    let focus = device.focus_hub();
                    let port_change_bitmap = (port_event[0] as u16) | ((port_event[1] as u16) << 8);
                    for i in 1..=n_ports {
                        if (port_change_bitmap & (1 << i)) != 0 {
                            let port =
                                UsbHubPortNumber(unsafe { NonZeroU8::new_unchecked(i as u8) });
                            let status = Self::get_port_status(&device, port).await.unwrap();
                            if status
                                .change
                                .contains(UsbHub2PortChangeBit::C_PORT_CONNECTION)
                            {
                                Timer::sleep_async(hub_desc.power_on_to_power_good()).await;
                                Self::clear_port_feature(
                                    &device,
                                    UsbHub2PortFeatureSel::C_PORT_CONNECTION,
                                    port,
                                )
                                .await
                                .unwrap();

                                if status
                                    .status
                                    .contains(UsbHub2PortStatusBit::PORT_CONNECTION)
                                {
                                    // Attached
                                    hub.clone().attach_device(port).await;
                                } else {
                                    log!("ADDR {} HUB2 PORT {} DETACHED", addr.0, i);
                                    // TODO: Detached
                                }
                            } else {
                                use UsbHub2PortFeatureSel::*;
                                if status.change.contains(UsbHub2PortChangeBit::C_PORT_ENABLE) {
                                    Self::clear_port_feature(&device, C_PORT_ENABLE, port)
                                        .await
                                        .unwrap();
                                }
                                if status.change.contains(UsbHub2PortChangeBit::C_PORT_SUSPEND) {
                                    Self::clear_port_feature(&device, C_PORT_SUSPEND, port)
                                        .await
                                        .unwrap();
                                }
                                if status
                                    .change
                                    .contains(UsbHub2PortChangeBit::C_PORT_OVER_CURRENT)
                                {
                                    Self::clear_port_feature(&device, C_PORT_OVER_CURRENT, port)
                                        .await
                                        .unwrap();
                                }
                                if status.change.contains(UsbHub2PortChangeBit::C_PORT_RESET) {
                                    Self::clear_port_feature(&device, C_PORT_RESET, port)
                                        .await
                                        .unwrap();
                                }
                            }
                        }
                    }
                    drop(focus);
                }
                Err(UsbError::Aborted) => break,
                Err(_err) => {
                    // TODO:
                    log!("USB2 HUB READ ERROR {:?}", _err);
                    return;
                }
            }
        }
    }

    pub async fn init_hub(self: Arc<Self>) {
        let n_ports = self.hub_desc.num_ports();
        for i in 1..=n_ports {
            let port_id = UsbHubPortNumber(unsafe { NonZeroU8::new_unchecked(i as u8) });
            Self::set_port_feature(&self.device, UsbHub2PortFeatureSel::PORT_POWER, port_id)
                .await
                .unwrap();
            Timer::sleep_async(Duration::from_millis(10)).await;
        }
        for i in 1..=n_ports {
            let port_id = UsbHubPortNumber(unsafe { NonZeroU8::new_unchecked(i as u8) });
            Self::clear_port_feature(
                &self.device,
                UsbHub2PortFeatureSel::C_PORT_CONNECTION,
                port_id,
            )
            .await
            .unwrap();
            Timer::sleep_async(Duration::from_millis(10)).await;
        }
        Timer::sleep_async(self.hub_desc.power_on_to_power_good() * 2).await;

        for i in 1..=n_ports {
            let port = UsbHubPortNumber(unsafe { NonZeroU8::new_unchecked(i as u8) });
            let status = Self::get_port_status(&self.device, port).await.unwrap();
            if status
                .status
                .contains(UsbHub2PortStatusBit::PORT_CONNECTION)
            {
                self.clone().attach_device(port).await;
            }
            Timer::sleep_async(Duration::from_millis(10)).await;
        }
    }

    async fn attach_device(self: Arc<Self>, port: UsbHubPortNumber) {
        Self::set_port_feature(&self.device, UsbHub2PortFeatureSel::PORT_RESET, port)
            .await
            .unwrap();
        Timer::sleep_async(self.hub_desc.power_on_to_power_good()).await;

        let status = Self::get_port_status(&self.device, port).await.unwrap();
        if status
            .change
            .contains(UsbHub2PortChangeBit::C_PORT_CONNECTION)
        {
            Self::clear_port_feature(&self.device, UsbHub2PortFeatureSel::C_PORT_CONNECTION, port)
                .await
                .unwrap();
        }
        if status.change.contains(UsbHub2PortChangeBit::C_PORT_ENABLE) {
            Self::clear_port_feature(&self.device, UsbHub2PortFeatureSel::C_PORT_ENABLE, port)
                .await
                .unwrap();
        }
        if status.change.contains(UsbHub2PortChangeBit::C_PORT_SUSPEND) {
            Self::clear_port_feature(&self.device, UsbHub2PortFeatureSel::C_PORT_SUSPEND, port)
                .await
                .unwrap();
        }
        if status
            .change
            .contains(UsbHub2PortChangeBit::C_PORT_OVER_CURRENT)
        {
            Self::clear_port_feature(
                &self.device,
                UsbHub2PortFeatureSel::C_PORT_OVER_CURRENT,
                port,
            )
            .await
            .unwrap();
        }
        if status.change.contains(UsbHub2PortChangeBit::C_PORT_RESET) {
            Self::clear_port_feature(&self.device, UsbHub2PortFeatureSel::C_PORT_RESET, port)
                .await
                .unwrap();
        }
        Timer::sleep_async(self.hub_desc.power_on_to_power_good()).await;

        if status
            .status
            .contains(UsbHub2PortStatusBit::PORT_CONNECTION)
        {
            let speed = status.status.speed();
            let _child = self.device.attach_child_device(port, speed).await.unwrap();
        }
    }

    pub async fn get_port_status(
        device: &UsbDeviceControl,
        port: UsbHubPortNumber,
    ) -> Result<UsbHub2PortStatus, UsbError> {
        UsbHubCommon::get_port_status(device, port).await
    }

    pub async fn set_port_feature(
        device: &UsbDeviceControl,
        feature_sel: UsbHub2PortFeatureSel,
        port: UsbHubPortNumber,
    ) -> Result<(), UsbError> {
        UsbHubCommon::set_port_feature(device, feature_sel, port).await
    }

    pub async fn clear_port_feature(
        device: &UsbDeviceControl,
        feature_sel: UsbHub2PortFeatureSel,
        port: UsbHubPortNumber,
    ) -> Result<(), UsbError> {
        UsbHubCommon::clear_port_feature(device, feature_sel, port).await
    }
}

pub struct Usb3HubDriver {
    device: Arc<UsbDeviceControl>,
    hub_desc: Usb3HubDescriptor,
}

impl Usb3HubDriver {
    async fn _usb_hub_task(device: Arc<UsbDeviceControl>, ep: UsbEndpointAddress, ps: u16) {
        let addr = device.device().addr();
        let hub_desc: Usb3HubDescriptor =
            match UsbHubCommon::get_hub_descriptor(&device, UsbDescriptorType::Hub3, 0).await {
                Ok(v) => v,
                Err(_err) => {
                    // TODO:
                    log!("USB3 GET HUB DESCRIPTOR {:?}", _err);
                    return;
                }
            };
        Self::set_depth(&device).await.unwrap();

        let hub = Arc::new(Usb3HubDriver {
            device: device.clone(),
            hub_desc,
        });

        let focus = device.focus_hub();
        hub.clone().init_hub().await;
        drop(focus);

        let n_ports = hub_desc.num_ports();
        let mut port_event = [0u8; 8];
        loop {
            match device.read_slice(ep, &mut port_event, 1, ps as usize).await {
                Ok(_) => {
                    let port_change_bitmap = (port_event[0] as u16) | ((port_event[1] as u16) << 8);
                    let focus = device.focus_hub();
                    for i in 1..=n_ports {
                        if (port_change_bitmap & (1 << i)) != 0 {
                            let port =
                                UsbHubPortNumber(unsafe { NonZeroU8::new_unchecked(i as u8) });
                            let status = Self::get_port_status(&device, port).await.unwrap();
                            if status
                                .change
                                .contains(UsbHub3PortChangeBit::C_PORT_CONNECTION)
                            {
                                Timer::sleep_async(hub_desc.power_on_to_power_good()).await;
                                Self::clear_port_feature(
                                    &device,
                                    UsbHub3PortFeatureSel::C_PORT_CONNECTION,
                                    port,
                                )
                                .await
                                .unwrap();

                                if status
                                    .status
                                    .contains(UsbHub3PortStatusBit::PORT_CONNECTION)
                                {
                                    // Attached
                                    hub.clone().attach_device(port).await;
                                } else {
                                    log!("ADDR {} HUB3 PORT {} DETACHED", addr.0, i);
                                    // TODO: Detached
                                }
                            } else {
                                use UsbHub3PortFeatureSel::*;
                                if status
                                    .change
                                    .contains(UsbHub3PortChangeBit::C_BH_PORT_RESET)
                                {
                                    Self::clear_port_feature(&device, C_BH_PORT_RESET, port)
                                        .await
                                        .unwrap();
                                }
                                if status.change.contains(UsbHub3PortChangeBit::C_PORT_RESET) {
                                    Self::clear_port_feature(&device, C_PORT_RESET, port)
                                        .await
                                        .unwrap();
                                }
                                if status
                                    .change
                                    .contains(UsbHub3PortChangeBit::C_PORT_OVER_CURRENT)
                                {
                                    Self::clear_port_feature(&device, C_PORT_OVER_CURRENT, port)
                                        .await
                                        .unwrap();
                                }
                                if status
                                    .change
                                    .contains(UsbHub3PortChangeBit::C_PORT_LINK_STATE)
                                {
                                    Self::clear_port_feature(&device, C_PORT_LINK_STATE, port)
                                        .await
                                        .unwrap();
                                }
                                if status
                                    .change
                                    .contains(UsbHub3PortChangeBit::C_PORT_CONFIG_ERROR)
                                {
                                    Self::clear_port_feature(&device, C_PORT_CONFIG_ERROR, port)
                                        .await
                                        .unwrap();
                                }
                            }
                        }
                    }
                    drop(focus);
                }
                Err(UsbError::Aborted) => break,
                Err(_err) => {
                    // TODO:
                    log!("USB3 HUB READ ERROR {:?}", _err);
                    return;
                }
            }
        }
    }

    pub async fn init_hub(self: Arc<Self>) {
        match self.device.configure_hub3(&self.hub_desc) {
            Ok(_) => (),
            Err(_err) => {
                // TODO:
                log!("USB3 COFNIGURE HUB3 {:?}", _err);
                return;
            }
        }
        let n_ports = self.hub_desc.num_ports();

        for i in 1..=n_ports {
            let port = UsbHubPortNumber(unsafe { NonZeroU8::new_unchecked(i as u8) });
            let status = Self::get_port_status(&self.device, port).await.unwrap();
            Self::set_port_feature(&self.device, UsbHub3PortFeatureSel::PORT_POWER, port)
                .await
                .unwrap();
            Timer::sleep_async(Duration::from_millis(10)).await;
            // Timer::sleep_async(hub_desc.power_on_to_power_good()).await;
            if status
                .status
                .contains(UsbHub3PortStatusBit::PORT_CONNECTION | UsbHub3PortStatusBit::PORT_ENABLE)
            {
                self.clone().attach_device(port).await;
            }
        }
    }

    pub async fn attach_device(self: Arc<Self>, port: UsbHubPortNumber) {
        Self::set_port_feature(&self.device, UsbHub3PortFeatureSel::BH_PORT_RESET, port)
            .await
            .unwrap();

        let deadline = Timer::new(self.hub_desc.power_on_to_power_good() * 2);
        loop {
            let status = Self::get_port_status(&self.device, port).await.unwrap();
            if deadline.is_expired()
                || status.status.contains(
                    UsbHub3PortStatusBit::PORT_CONNECTION | UsbHub3PortStatusBit::PORT_ENABLE,
                )
            {
                break;
            }
            Timer::sleep_async(Duration::from_millis(10)).await;
        }

        let status = Self::get_port_status(&self.device, port).await.unwrap();
        if status
            .change
            .contains(UsbHub3PortChangeBit::C_BH_PORT_RESET)
        {
            Self::clear_port_feature(&self.device, UsbHub3PortFeatureSel::C_BH_PORT_RESET, port)
                .await
                .unwrap();
        }
        if status.change.contains(UsbHub3PortChangeBit::C_PORT_RESET) {
            Self::clear_port_feature(&self.device, UsbHub3PortFeatureSel::C_PORT_RESET, port)
                .await
                .unwrap();
        }
        if status
            .change
            .contains(UsbHub3PortChangeBit::C_PORT_OVER_CURRENT)
        {
            Self::clear_port_feature(
                &self.device,
                UsbHub3PortFeatureSel::C_PORT_OVER_CURRENT,
                port,
            )
            .await
            .unwrap();
        }
        if status
            .change
            .contains(UsbHub3PortChangeBit::C_PORT_LINK_STATE)
        {
            Self::clear_port_feature(&self.device, UsbHub3PortFeatureSel::C_PORT_LINK_STATE, port)
                .await
                .unwrap();
        }
        if status
            .change
            .contains(UsbHub3PortChangeBit::C_PORT_CONFIG_ERROR)
        {
            Self::clear_port_feature(
                &self.device,
                UsbHub3PortFeatureSel::C_PORT_CONFIG_ERROR,
                port,
            )
            .await
            .unwrap();
        }

        let status = Self::get_port_status(&self.device, port).await.unwrap();
        if status
            .status
            .contains(UsbHub3PortStatusBit::PORT_CONNECTION | UsbHub3PortStatusBit::PORT_ENABLE)
        {
            let _child = self
                .device
                .attach_child_device(port, PSIV::SS)
                .await
                .unwrap();
        }
    }

    pub async fn set_depth(device: &UsbDeviceControl) -> Result<(), UsbError> {
        device
            .control_nodata(
                UsbControlSetupData::request(
                    UsbControlRequestBitmap::SET_CLASS,
                    UsbControlRequest::SET_HUB_DEPTH,
                )
                .value(device.device().route_string().depth() as u16),
            )
            .await
    }

    pub async fn get_port_status(
        device: &UsbDeviceControl,
        port: UsbHubPortNumber,
    ) -> Result<UsbHub3PortStatus, UsbError> {
        UsbHubCommon::get_port_status(device, port).await
    }

    pub async fn set_port_feature(
        device: &UsbDeviceControl,
        feature_sel: UsbHub3PortFeatureSel,
        port: UsbHubPortNumber,
    ) -> Result<(), UsbError> {
        UsbHubCommon::set_port_feature(device, feature_sel, port).await
    }

    pub async fn clear_port_feature(
        device: &UsbDeviceControl,
        feature_sel: UsbHub3PortFeatureSel,
        port: UsbHubPortNumber,
    ) -> Result<(), UsbError> {
        UsbHubCommon::clear_port_feature(device, feature_sel, port).await
    }
}

pub struct UsbHubCommon;

impl UsbHubCommon {
    #[inline]
    pub async fn get_hub_descriptor<T: UsbDescriptor>(
        device: &UsbDeviceControl,
        desc_type: UsbDescriptorType,
        index: u8,
    ) -> Result<T, UsbError> {
        device
            .get_descriptor(UsbControlRequestBitmap::GET_CLASS, desc_type, index)
            .await
    }

    #[inline]
    pub async fn set_hub_feature(
        device: &UsbDeviceControl,
        feature_sel: UsbHubFeatureSel,
    ) -> Result<(), UsbError> {
        device
            .control_nodata(
                UsbControlSetupData::request(
                    UsbControlRequestBitmap::SET_CLASS,
                    UsbControlRequest::SET_FEATURE,
                )
                .value(feature_sel as u16),
            )
            .await
    }

    #[inline]
    pub async fn clear_hub_feature(
        device: &UsbDeviceControl,
        feature_sel: UsbHubFeatureSel,
    ) -> Result<(), UsbError> {
        device
            .control_nodata(
                UsbControlSetupData::request(
                    UsbControlRequestBitmap::SET_CLASS,
                    UsbControlRequest::CLEAR_FEATURE,
                )
                .value(feature_sel as u16),
            )
            .await
    }

    #[inline]
    pub async fn set_port_feature<T>(
        device: &UsbDeviceControl,
        feature_sel: T,
        port: UsbHubPortNumber,
    ) -> Result<(), UsbError>
    where
        T: Into<u16>,
    {
        device
            .control_nodata(
                UsbControlSetupData::request(
                    UsbControlRequestBitmap(0x23),
                    UsbControlRequest::SET_FEATURE,
                )
                .value(feature_sel.into())
                .index(port.0.get() as u16),
            )
            .await
    }

    #[inline]
    pub async fn clear_port_feature<T>(
        device: &UsbDeviceControl,
        feature_sel: T,
        port: UsbHubPortNumber,
    ) -> Result<(), UsbError>
    where
        T: Into<u16>,
    {
        device
            .control_nodata(
                UsbControlSetupData::request(
                    UsbControlRequestBitmap(0x23),
                    UsbControlRequest::CLEAR_FEATURE,
                )
                .value(feature_sel.into())
                .index(port.0.get() as u16),
            )
            .await
    }

    pub async fn get_port_status<T: Copy>(
        device: &UsbDeviceControl,
        port: UsbHubPortNumber,
    ) -> Result<T, UsbError> {
        let mut data = [0; 4];
        match device
            .control_slice(
                UsbControlSetupData::request(
                    UsbControlRequestBitmap(0xA3),
                    UsbControlRequest::GET_STATUS,
                )
                .value(0)
                .index(port.0.get() as u16),
                &mut data,
            )
            .await
        {
            Ok(_) => {
                let result = unsafe {
                    let p = &data[0] as *const _ as *const T;
                    *p
                };
                Ok(result)
            }
            Err(err) => Err(err),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbHub2PortStatus {
    status: UsbHub2PortStatusBit,
    change: UsbHub2PortChangeBit,
}

impl UsbHub2PortStatus {
    #[inline]
    pub const fn empty() -> Self {
        Self {
            status: UsbHub2PortStatusBit::empty(),
            change: UsbHub2PortChangeBit::empty(),
        }
    }

    #[inline]
    pub const fn as_u32(&self) -> u32 {
        unsafe { transmute(*self) }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbHub3PortStatus {
    status: UsbHub3PortStatusBit,
    change: UsbHub3PortChangeBit,
}

impl UsbHub3PortStatus {
    #[inline]
    pub const fn empty() -> Self {
        Self {
            status: UsbHub3PortStatusBit::empty(),
            change: UsbHub3PortChangeBit::empty(),
        }
    }

    #[inline]
    pub const fn as_u32(&self) -> u32 {
        unsafe { transmute(*self) }
    }
}

/// USB Hub Feature Selector
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UsbHubFeatureSel {
    C_HUB_LOCAL_POWER = 0,
    C_HUB_OVER_CURRENT = 1,
}

/// USB2 Hub Port Feature Selector
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UsbHub2PortFeatureSel {
    PORT_CONNECTION = 0,
    PORT_ENABLE = 1,
    PORT_SUSPEND = 2,
    PORT_OVER_CURRENT = 3,
    PORT_RESET = 4,
    PORT_POWER = 8,
    PORT_LOW_SPEED = 9,
    C_PORT_CONNECTION = 16,
    C_PORT_ENABLE = 17,
    C_PORT_SUSPEND = 18,
    C_PORT_OVER_CURRENT = 19,
    C_PORT_RESET = 20,
    PORT_TEST = 21,
    PORT_INDICATOR = 22,
}

impl From<UsbHub2PortFeatureSel> for u16 {
    #[inline]
    fn from(val: UsbHub2PortFeatureSel) -> Self {
        val as u16
    }
}

/// USB3 Hub Port Feature Selector
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UsbHub3PortFeatureSel {
    PORT_CONNECTION = 0,
    PORT_OVER_CURRENT = 3,
    PORT_RESET = 4,
    PORT_LINK_STATE = 5,
    PORT_POWER = 8,
    C_PORT_CONNECTION = 16,
    C_PORT_OVER_CURRENT = 19,
    C_PORT_RESET = 20,
    PORT_U1_TIMEOUT = 23,
    PORT_U2_TIMEOUT = 24,
    C_PORT_LINK_STATE = 25,
    C_PORT_CONFIG_ERROR = 26,
    PORT_REMOTE_WAKE_MASK = 27,
    BH_PORT_RESET = 28,
    C_BH_PORT_RESET = 29,
    FORCE_LINKPM_ACCEPT = 30,
}

impl From<UsbHub3PortFeatureSel> for u16 {
    #[inline]
    fn from(val: UsbHub3PortFeatureSel) -> Self {
        val as u16
    }
}

bitflags! {
    /// USB2 Hub Port Status Bits
    pub struct UsbHub2PortStatusBit: u16 {
        const PORT_CONNECTION   = 0b0000_0000_0000_0001;
        const PORT_ENABLE       = 0b0000_0000_0000_0010;
        const PORT_SUSPEND      = 0b0000_0000_0000_0100;
        const PORT_OVER_CURRENT = 0b0000_0000_0000_1000;
        const PORT_RESET        = 0b0000_0000_0001_0000;

        const PORT_POWER        = 0b0000_0001_0000_0000;
        const PORT_LOW_SPEED    = 0b0000_0010_0000_0000;
        const PORT_HIGH_SPEED   = 0b0000_0100_0000_0000;
        const PORT_TEST         = 0b0000_1000_0000_0000;
        const PORT_INDICATOR    = 0b0001_0000_0000_0000;
    }
}

impl UsbHub2PortStatusBit {
    #[inline]
    pub fn speed(&self) -> PSIV {
        if self.contains(Self::PORT_LOW_SPEED) {
            PSIV::LS
        } else if self.contains(Self::PORT_HIGH_SPEED) {
            PSIV::HS
        } else {
            PSIV::FS
        }
    }
}

bitflags! {
    /// USB2 Hub Port Status Change Bits
    pub struct UsbHub2PortChangeBit: u16 {
        const C_PORT_CONNECTION     = 0b0000_0000_0000_0001;
        const C_PORT_ENABLE         = 0b0000_0000_0000_0010;
        const C_PORT_SUSPEND        = 0b0000_0000_0000_0100;
        const C_PORT_OVER_CURRENT   = 0b0000_0000_0000_1000;
        const C_PORT_RESET          = 0b0000_0000_0001_0000;
    }
}

bitflags! {
    /// USB3 Hub Port Status Bits
    pub struct UsbHub3PortStatusBit: u16 {
        const PORT_CONNECTION   = 0b0000_0000_0000_0001;
        const PORT_ENABLE       = 0b0000_0000_0000_0010;
        const PORT_OVER_CURRENT = 0b0000_0000_0000_1000;
        const PORT_RESET        = 0b0000_0000_0001_0000;
        const PORT_LINK_STATE   = 0b0000_0001_1110_0000;
        const PORT_POWER        = 0b0000_0010_0000_0000;
        const PORT_SPEED        = 0b0001_1100_0000_0000;
    }
}

impl UsbHub3PortStatusBit {
    #[inline]
    pub const fn link_state_raw(&self) -> usize {
        ((self.bits() & Self::PORT_LINK_STATE.bits()) as usize) >> 5
    }

    #[inline]
    pub fn link_state(&self) -> Option<Usb3LinkState> {
        FromPrimitive::from_usize(self.link_state_raw())
    }
}

bitflags! {
    /// USB3 Hub Port Status Change Bits
    pub struct UsbHub3PortChangeBit: u16 {
        const C_PORT_CONNECTION     = 0b0000_0000_0000_0001;
        const C_PORT_OVER_CURRENT   = 0b0000_0000_0000_1000;
        const C_PORT_RESET          = 0b0000_0000_0001_0000;
        const C_BH_PORT_RESET       = 0b0000_0000_0010_0000;
        const C_PORT_LINK_STATE     = 0b0000_0000_0100_0000;
        const C_PORT_CONFIG_ERROR   = 0b0000_0000_1000_0000;
    }
}
