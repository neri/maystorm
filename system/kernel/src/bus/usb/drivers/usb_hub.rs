//! USB Hub Class Driver

use super::super::*;
use crate::{
    task::{scheduler::Timer, Task},
    *,
};
use _core::time::Duration;
use alloc::{sync::Arc, vec::Vec};
use bitflags::*;
use core::{mem::transmute, num::NonZeroU8};
use num_traits::FromPrimitive;

pub struct UsbHubStarter;

impl UsbHubStarter {
    #[inline]
    pub fn new() -> Arc<dyn UsbClassDriverStarter> {
        Arc::new(Self {})
    }
}

impl UsbClassDriverStarter for UsbHubStarter {
    fn instantiate(&self, device: &UsbDevice) -> bool {
        let class = device.class();
        match class {
            UsbClass::HUB_FS | UsbClass::HUB_HS_MTT | UsbClass::HUB_HS_STT | UsbClass::HUB_SS => (),
            _ => return false,
        }

        let addr = device.addr();
        let config = device.current_configuration();
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
        let if_no = interface.if_no();
        let endpoint = match interface.endpoints().first() {
            Some(v) => v,
            None => todo!(),
        };
        let ep = endpoint.address();
        let ps = endpoint.descriptor().max_packet_size();
        if ps > 8 {
            return false;
        }
        device
            .host()
            .configure_endpoint(endpoint.descriptor())
            .unwrap();

        match class {
            UsbClass::HUB_FS | UsbClass::HUB_HS_MTT | UsbClass::HUB_HS_STT => {
                UsbManager::register_xfer_task(Task::new(Usb2HubDriver::_usb_hub_task(
                    addr, if_no, ep, class, ps,
                )));
            }
            // UsbClass::HUB_SS => {
            //     UsbManager::register_xfer_task(Task::new(Usb3HubDriver::_usb_hub_task(
            //         addr, if_no, ep, class, ps,
            //     )));
            // }
            _ => (),
        }

        true
    }
}

pub struct Usb2HubDriver;

impl Usb2HubDriver {
    /// USB2 Hub Task (FS, HS, HS-MTT)
    async fn _usb_hub_task(
        addr: UsbDeviceAddress,
        _if_no: UsbInterfaceNumber,
        ep: UsbEndpointAddress,
        class: UsbClass,
        ps: u16,
    ) {
        let is_mtt = class == UsbClass::HUB_HS_MTT;
        let device = UsbManager::device_by_addr(addr).unwrap();

        let hub_desc: UsbHub2Descriptor =
            match UsbHubCommon::get_hub_descriptor(&device, UsbDescriptorType::Hub, 0) {
                Ok(v) => v,
                Err(_err) => {
                    // TODO:
                    log!("USB2 GET HUB DESCRIPTOR {:?}", _err);
                    return;
                }
            };
        match device.host().configure_hub2(&hub_desc, is_mtt) {
            Ok(_) => (),
            Err(_err) => {
                // TODO:
                log!("USB2 COFNIGURE HUB2 {:?}", _err);
                return;
            }
        }

        let n_ports = hub_desc.num_ports();
        for i in 1..=n_ports {
            Self::set_port_feature(
                &device,
                UsbHub2PortFeatureSel::PORT_POWER,
                UsbHubPortNumber(unsafe { NonZeroU8::new_unchecked(i as u8) }),
            )
            .unwrap();
            Timer::sleep_async(Duration::from_millis(10)).await;
        }
        for i in 1..=n_ports {
            Self::clear_port_feature(
                &device,
                UsbHub2PortFeatureSel::C_PORT_CONNECTION,
                UsbHubPortNumber(unsafe { NonZeroU8::new_unchecked(i as u8) }),
            )
            .unwrap();
            Timer::sleep_async(Duration::from_millis(10)).await;
        }
        Timer::sleep_async(hub_desc.power_on_to_power_good() * 2).await;

        for i in 1..=n_ports {
            let port = UsbHubPortNumber(unsafe { NonZeroU8::new_unchecked(i as u8) });
            let status = Self::get_port_status(&device, port).unwrap();
            if status
                .status
                .contains(UsbHub2PortStatusBit::PORT_CONNECTION)
            {
                Self::attach_device(&device, &hub_desc, port).await
            }
            Timer::sleep_async(Duration::from_millis(10)).await;
        }

        let mut port_event = [0u8; 8];
        loop {
            match device.read_slice(ep, &mut port_event, 1, ps as usize).await {
                Ok(_) => {
                    let port_change_bitmap = (port_event[0] as u16) | ((port_event[1] as u16) << 8);
                    for i in 1..n_ports {
                        if (port_change_bitmap & (1 << i)) != 0 {
                            let port =
                                UsbHubPortNumber(unsafe { NonZeroU8::new_unchecked(i as u8) });
                            let status = Self::get_port_status(&device, port).unwrap();
                            log!(
                                "ADDR {} HUB2 PORT {} STATUS CHANGE {:08x}",
                                addr.0,
                                i,
                                status.as_u32()
                            );
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
                                .unwrap();

                                if status
                                    .status
                                    .contains(UsbHub2PortStatusBit::PORT_CONNECTION)
                                {
                                    // Attached
                                    Self::attach_device(&device, &hub_desc, port).await
                                } else {
                                    log!("ADDR {} HUB2 PORT {} DETACHED", addr.0, i);
                                    // Detached
                                }
                            } else if status.change.contains(UsbHub2PortChangeBit::C_PORT_RESET) {
                                Self::clear_port_feature(
                                    &device,
                                    UsbHub2PortFeatureSel::C_PORT_RESET,
                                    port,
                                )
                                .unwrap();
                            } else {
                                // TODO:
                            }
                        }
                    }
                    Timer::sleep_async(hub_desc.power_on_to_power_good()).await;
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

    pub async fn attach_device(
        device: &UsbDevice,
        hub_desc: &UsbHub2Descriptor,
        port: UsbHubPortNumber,
    ) {
        device.host().enter_configuration().await.unwrap();

        Self::set_port_feature(&device, UsbHub2PortFeatureSel::PORT_RESET, port).unwrap();
        Timer::sleep_async(Duration::from_millis(10)).await;

        Self::clear_port_feature(&device, UsbHub2PortFeatureSel::C_PORT_RESET, port).unwrap();
        Timer::sleep_async(hub_desc.power_on_to_power_good()).await;

        let status = Self::get_port_status(&device, port).unwrap();
        let speed = status.status.speed();
        let _child = device.host().attach_device(port, speed, 0).unwrap();

        device.host().leave_configuration().unwrap();
    }

    pub fn get_port_status(
        device: &UsbDevice,
        port: UsbHubPortNumber,
    ) -> Result<UsbHub2PortStatus, UsbError> {
        UsbHubCommon::get_port_status(device, port)
    }

    pub fn set_port_feature(
        device: &UsbDevice,
        feature_sel: UsbHub2PortFeatureSel,
        port: UsbHubPortNumber,
    ) -> Result<(), UsbError> {
        UsbHubCommon::set_port_feature(device, feature_sel, port)
    }

    pub fn clear_port_feature(
        device: &UsbDevice,
        feature_sel: UsbHub2PortFeatureSel,
        port: UsbHubPortNumber,
    ) -> Result<(), UsbError> {
        UsbHubCommon::clear_port_feature(device, feature_sel, port)
    }
}

pub struct Usb3HubDriver;

impl Usb3HubDriver {
    #[allow(dead_code)]
    async fn _usb_hub_task(
        addr: UsbDeviceAddress,
        _if_no: UsbInterfaceNumber,
        ep: UsbEndpointAddress,
        _class: UsbClass,
        ps: u16,
    ) {
        let device = UsbManager::device_by_addr(addr).unwrap();

        let hub_desc: UsbHub3Descriptor =
            match UsbHubCommon::get_hub_descriptor(&device, UsbDescriptorType::Hub3, 0) {
                Ok(v) => v,
                Err(_err) => {
                    // TODO:
                    log!("USB3 GET HUB DESCRIPTOR {:?}", _err);
                    return;
                }
            };
        let ss_dev_cap = match device.ss_dev_cap() {
            Some(v) => v,
            None => {
                // TODO:
                log!("USB3 HUB NO SS DEV CAP ");
                return;
            }
        };
        let max_exit_latency =
            usize::max(ss_dev_cap.u1_dev_exit_lat(), ss_dev_cap.u2_dev_exit_lat());

        match device.host().configure_hub3(&hub_desc, max_exit_latency) {
            Ok(_) => (),
            Err(_err) => {
                // TODO:
                log!("USB3 COFNIGURE HUB3 {:?}", _err);
                return;
            }
        }
        let n_ports = hub_desc.num_ports();

        log!(
            "ADR {} HUB3 ports {} pwr2good {} ",
            addr.0,
            n_ports,
            hub_desc.power_on_to_power_good().as_millis(),
        );

        for i in 1..=n_ports {
            let port = UsbHubPortNumber(unsafe { NonZeroU8::new_unchecked(i as u8) });
            let status = Self::get_port_status(&device, port).unwrap();
            log!(
                "HUB3 {}.{} status1 {:08x} {:?}",
                addr.0,
                port.0,
                status.as_u32(),
                status.status.link_state()
            );
            Timer::sleep_async(Duration::from_millis(10)).await;
        }
        // Timer::sleep_async(hub_desc.power_on_to_power_good() * 2).await;

        for i in 1..=n_ports {
            let port = UsbHubPortNumber(unsafe { NonZeroU8::new_unchecked(i as u8) });
            let status = Self::get_port_status(&device, port).unwrap();
            log!(
                "HUB3 {}.{} status2 {:08x} {:?}",
                addr.0,
                port.0,
                status.as_u32(),
                status.status.link_state()
            );
            Timer::sleep_async(Duration::from_millis(10)).await;
            if status
                .status
                .contains(UsbHub3PortStatusBit::PORT_CONNECTION | UsbHub3PortStatusBit::PORT_ENABLE)
            {
                Self::attach_device(&device, &hub_desc, port, max_exit_latency).await;
            }
        }
        log!("ALL PORTS ARE RESET");
        Timer::sleep_async(hub_desc.power_on_to_power_good() * 2).await;

        let mut port_event = [0u8; 8];
        loop {
            match device.read_slice(ep, &mut port_event, 1, ps as usize).await {
                Ok(_) => {
                    let port_change_bitmap = (port_event[0] as u16) | ((port_event[1] as u16) << 8);
                    for i in 1..n_ports {
                        if (port_change_bitmap & (1 << i)) != 0 {
                            let port =
                                UsbHubPortNumber(unsafe { NonZeroU8::new_unchecked(i as u8) });
                            let status = Self::get_port_status(&device, port).unwrap();
                            log!(
                                "ADDR {} HUB3 PORT {} STATUS CHANGE {:08x}",
                                addr.0,
                                i,
                                status.as_u32()
                            );
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
                                .unwrap();

                                if status
                                    .status
                                    .contains(UsbHub3PortStatusBit::PORT_CONNECTION)
                                {
                                    // Attached
                                    Self::attach_device(&device, &hub_desc, port, max_exit_latency)
                                        .await;
                                } else {
                                    log!("ADDR {} HUB3 PORT {} DETACHED", addr.0, i);
                                    // Detached
                                }
                            } else if status.change.contains(UsbHub3PortChangeBit::C_PORT_RESET) {
                                Self::clear_port_feature(
                                    &device,
                                    UsbHub3PortFeatureSel::C_PORT_RESET,
                                    port,
                                )
                                .unwrap();
                            } else {
                                // TODO:
                            }
                        }
                    }
                    Timer::sleep_async(hub_desc.power_on_to_power_good()).await;
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

    pub async fn attach_device(
        device: &UsbDevice,
        _hub_desc: &UsbHub3Descriptor,
        port: UsbHubPortNumber,
        max_exit_latency: usize,
    ) {
        device.host().enter_configuration().await.unwrap();

        // Self::clear_port_feature(&device, UsbHub3PortFeatureSel::C_PORT_CONNECTION, port).unwrap();

        // Self::set_port_feature(&device, UsbHub3PortFeatureSel::PORT_RESET, port).unwrap();
        // Timer::sleep_async(Duration::from_millis(10)).await;

        // Self::clear_port_feature(&device, UsbHub3PortFeatureSel::C_PORT_RESET, port).unwrap();
        // Timer::sleep_async(hub_desc.power_on_to_power_good()).await;

        // let status = Self::get_port_status(&device, port).unwrap();
        let _child = device
            .host()
            .attach_device(port, PSIV::SS, max_exit_latency)
            .unwrap();

        device.host().leave_configuration().unwrap();
    }

    pub fn get_port_status(
        device: &UsbDevice,
        port: UsbHubPortNumber,
    ) -> Result<UsbHub3PortStatus, UsbError> {
        UsbHubCommon::get_port_status(device, port)
    }

    pub fn set_port_feature(
        device: &UsbDevice,
        feature_sel: UsbHub3PortFeatureSel,
        port: UsbHubPortNumber,
    ) -> Result<(), UsbError> {
        UsbHubCommon::set_port_feature(device, feature_sel, port)
    }

    pub fn clear_port_feature(
        device: &UsbDevice,
        feature_sel: UsbHub3PortFeatureSel,
        port: UsbHubPortNumber,
    ) -> Result<(), UsbError> {
        UsbHubCommon::clear_port_feature(device, feature_sel, port)
    }
}

pub struct UsbHubCommon;

impl UsbHubCommon {
    pub fn get_hub_descriptor<T: UsbDescriptor>(
        device: &UsbDevice,
        desc_type: UsbDescriptorType,
        index: u8,
    ) -> Result<T, UsbError> {
        UsbDevice::get_descriptor(
            &device.host(),
            UsbControlRequestBitmap::GET_CLASS,
            desc_type,
            index,
        )
    }

    pub fn set_hub_feature(
        device: &UsbDevice,
        feature_sel: UsbHubFeatureSel,
    ) -> Result<(), UsbError> {
        device
            .host()
            .control(UsbControlSetupData {
                bmRequestType: UsbControlRequestBitmap(0x20),
                bRequest: UsbControlRequest::SET_FEATURE,
                wValue: feature_sel as u16,
                wIndex: 0,
                wLength: 0,
            })
            .map(|_| ())
    }

    pub fn clear_hub_feature(
        device: &UsbDevice,
        feature_sel: UsbHubFeatureSel,
    ) -> Result<(), UsbError> {
        device
            .host()
            .control(UsbControlSetupData {
                bmRequestType: UsbControlRequestBitmap(0x20),
                bRequest: UsbControlRequest::CLEAR_FEATURE,
                wValue: feature_sel as u16,
                wIndex: 0,
                wLength: 0,
            })
            .map(|_| ())
    }

    pub fn set_port_feature<T>(
        device: &UsbDevice,
        feature_sel: T,
        port: UsbHubPortNumber,
    ) -> Result<(), UsbError>
    where
        T: Into<u16>,
    {
        device
            .host()
            .control(UsbControlSetupData {
                bmRequestType: UsbControlRequestBitmap(0x23),
                bRequest: UsbControlRequest::SET_FEATURE,
                wValue: feature_sel.into(),
                wIndex: port.0.get() as u16,
                wLength: 0,
            })
            .map(|_| ())
    }

    pub fn clear_port_feature<T>(
        device: &UsbDevice,
        feature_sel: T,
        port: UsbHubPortNumber,
    ) -> Result<(), UsbError>
    where
        T: Into<u16>,
    {
        device
            .host()
            .control(UsbControlSetupData {
                bmRequestType: UsbControlRequestBitmap(0x23),
                bRequest: UsbControlRequest::CLEAR_FEATURE,
                wValue: feature_sel.into(),
                wIndex: port.0.get() as u16,
                wLength: 0,
            })
            .map(|_| ())
    }

    pub fn get_port_status<T: Copy>(
        device: &UsbDevice,
        port: UsbHubPortNumber,
    ) -> Result<T, UsbError> {
        match device.host().control(UsbControlSetupData {
            bmRequestType: UsbControlRequestBitmap(0xA3),
            bRequest: UsbControlRequest::GET_STATUS,
            wValue: 0,
            wIndex: port.0.get() as u16,
            wLength: 4,
        }) {
            Ok(result) => {
                let result = unsafe {
                    let p = &result[0] as *const _ as *const T;
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
    /// RESERVED
    PORT_TEST = 21,
    PORT_INDICATOR = 22,
}

impl Into<u16> for UsbHub2PortFeatureSel {
    fn into(self) -> u16 {
        self as u16
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

impl Into<u16> for UsbHub3PortFeatureSel {
    fn into(self) -> u16 {
        self as u16
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
        const PORT_LOW_SPEED    = 0b0000_0010_0000_0001;
        const PORT_HIGH_SPEED   = 0b0000_0100_0000_0001;
        const PORT_TEST         = 0b0000_1000_0000_0001;
        const PORT_INDICATOR    = 0b0001_0000_0000_0001;
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
        const PORT_SPEED        = 0b0001_1100_0000_0001;
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
