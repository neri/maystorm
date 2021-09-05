//! USB Hub Class Driver

use super::super::*;
use crate::{
    io::hid::*,
    task::{scheduler::Timer, Task},
};
use _core::num::NonZeroU8;
use alloc::{sync::Arc, vec::Vec};
use bitflags::*;
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
        if ps > 2 {
            return false;
        }
        device
            .host()
            .configure_endpoint(endpoint.descriptor())
            .unwrap();

        UsbManager::register_xfer_task(Task::new(UsbHubDriver::_usb_hub_task(
            addr, if_no, ep, class, ps,
        )));

        true
    }
}

struct UsbHubDriver;

impl UsbHubDriver {
    async fn _usb_hub_task(
        addr: UsbDeviceAddress,
        if_no: UsbInterfaceNumber,
        ep: UsbEndpointAddress,
        class: UsbClass,
        ps: u16,
    ) {
        let device = UsbManager::device_by_addr(addr).unwrap();

        match class {
            UsbClass::HUB_FS | UsbClass::HUB_HS_MTT | UsbClass::HUB_HS_STT => (),
            // TODO:
            _ => return,
        }

        let hub_desc: UsbHub2Descriptor =
            match Self::get_hub_descriptor(&device, UsbDescriptorType::Hub, 0) {
                Ok(v) => v,
                Err(_err) => {
                    // TODO:
                    println!("USB2 GET HUB DESCRIPTOR {:?}", _err);
                    return;
                }
            };
        match device
            .host()
            .configure_hub2(&hub_desc, class == UsbClass::HUB_HS_MTT)
        {
            Ok(_) => (),
            Err(_err) => {
                // TODO:
                println!("USB2 COFNIGURE HUB2 {:?}", _err);
                return;
            }
        }

        let n_ports = hub_desc.num_ports();
        for i in 1..=n_ports {
            Self::set_port_feature(
                &device,
                UsbHubPortFeatureSel::PORT_POWER,
                UsbHubPortNumber(unsafe { NonZeroU8::new_unchecked(i as u8) }),
            )
            .unwrap();
        }
        Timer::sleep_async(hub_desc.power_on_to_power_good() * 2).await;

        let mut port_event = [0u8; 2];
        loop {
            match device.read_slice(ep, &mut port_event, 1, ps as usize).await {
                Ok(_) => {
                    let port_change_bitmap = (port_event[0] as u16) | ((port_event[1] as u16) << 8);
                    for i in 1..n_ports {
                        if (port_change_bitmap & (1 << i)) != 0 {
                            let port =
                                UsbHubPortNumber(unsafe { NonZeroU8::new_unchecked(i as u8) });
                            let status = Self::get_port_status(&device, port).unwrap();
                            if status
                                .change
                                .contains(UsbHub2PortChangeBit::C_PORT_CONNECTION)
                            {
                                Timer::sleep_async(hub_desc.power_on_to_power_good()).await;
                                Self::clear_port_feature(
                                    &device,
                                    UsbHubPortFeatureSel::C_PORT_CONNECTION,
                                    port,
                                )
                                .unwrap();

                                if status
                                    .status
                                    .contains(UsbHub2PortStatusBit::PORT_CONNECTION)
                                {
                                    // Attached
                                    device.host().enter_configuration().await.unwrap();
                                    Self::set_port_feature(
                                        &device,
                                        UsbHubPortFeatureSel::PORT_RESET,
                                        port,
                                    )
                                    .unwrap();
                                    for _ in 0..3 {
                                        Timer::sleep_async(hub_desc.power_on_to_power_good()).await;
                                        let status = Self::get_port_status(&device, port).unwrap();
                                        if status
                                            .change
                                            .contains(UsbHub2PortChangeBit::C_PORT_RESET)
                                        {
                                            break;
                                        }
                                    }
                                    Self::clear_port_feature(
                                        &device,
                                        UsbHubPortFeatureSel::C_PORT_RESET,
                                        port,
                                    )
                                    .unwrap();
                                    Timer::sleep_async(hub_desc.power_on_to_power_good()).await;
                                    let status = Self::get_port_status(&device, port).unwrap();
                                    let speed = status.status.speed();
                                    let _child = device.host().attach_device(port, speed).unwrap();
                                    device.host().leave_configuration().unwrap();
                                } else {
                                    println!("HUB PORT DETACHED {}", i);
                                    // Detached
                                }
                            } else if status.change.contains(UsbHub2PortChangeBit::C_PORT_RESET) {
                                Self::clear_port_feature(
                                    &device,
                                    UsbHubPortFeatureSel::C_PORT_RESET,
                                    port,
                                )
                                .unwrap();
                            } else {
                                // TODO:
                            }
                        }
                    }
                }
                Err(UsbError::Aborted) => break,
                Err(_err) => {
                    // TODO:
                    println!("USB2 HUB READ ERROR {:?}", _err);
                    return;
                }
            }
        }
    }

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

    pub fn set_port_feature(
        device: &UsbDevice,
        feature_sel: UsbHubPortFeatureSel,
        port: UsbHubPortNumber,
    ) -> Result<(), UsbError> {
        device
            .host()
            .control(UsbControlSetupData {
                bmRequestType: UsbControlRequestBitmap(0x23),
                bRequest: UsbControlRequest::SET_FEATURE,
                wValue: feature_sel as u16,
                wIndex: port.0.get() as u16,
                wLength: 0,
            })
            .map(|_| ())
    }

    pub fn clear_port_feature(
        device: &UsbDevice,
        feature_sel: UsbHubPortFeatureSel,
        port: UsbHubPortNumber,
    ) -> Result<(), UsbError> {
        device
            .host()
            .control(UsbControlSetupData {
                bmRequestType: UsbControlRequestBitmap(0x23),
                bRequest: UsbControlRequest::CLEAR_FEATURE,
                wValue: feature_sel as u16,
                wIndex: port.0.get() as u16,
                wLength: 0,
            })
            .map(|_| ())
    }

    pub fn get_port_status(
        device: &UsbDevice,
        port: UsbHubPortNumber,
    ) -> Result<UsbHub2PortStatus, UsbError> {
        match device.host().control(UsbControlSetupData {
            bmRequestType: UsbControlRequestBitmap(0xA3),
            bRequest: UsbControlRequest::GET_STATUS,
            wValue: 0,
            wIndex: port.0.get() as u16,
            wLength: 4,
        }) {
            Ok(result) => {
                let result = unsafe {
                    let p = &result[0] as *const _ as *const UsbHub2PortStatus;
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
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UsbHubFeatureSel {
    C_HUB_LOCAL_POWER = 0,
    C_HUB_OVER_CURRENT = 1,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UsbHubPortFeatureSel {
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
