//! Universal Serial Bus

use super::types::*;
use crate::sync::RwLock;
use alloc::{boxed::Box, string::String, sync::Arc, vec::Vec};
use core::{
    cell::UnsafeCell,
    mem::{size_of, MaybeUninit},
    num::NonZeroU8,
    slice,
};
use num_traits::FromPrimitive;

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

/// USB Driver to Host interface
pub trait UsbHostInterface {
    fn configure_endpoint(&self, desc: &UsbEndpointDescriptor) -> Result<(), UsbError>;

    /// Performs a control transfer
    fn control<'a>(&self, setup: UsbControlSetupData) -> Result<&'a [u8], UsbError>;

    fn write(
        &self,
        ep: UsbEndpointAddress,
        buffer: *const u8,
        len: usize,
    ) -> Result<usize, UsbError>;

    fn read(&self, ep: UsbEndpointAddress, buffer: *mut u8, len: usize) -> Result<usize, UsbError>;

    // fn enter_configuration(&self);
    // fn leave_configuration(&self);
}

static mut USB_MANAGER: UnsafeCell<UsbManager> = UnsafeCell::new(UsbManager::new());

pub struct UsbManager {
    devices: RwLock<Vec<Arc<UsbDevice>>>,
}

impl UsbManager {
    const fn new() -> Self {
        Self {
            devices: RwLock::new(Vec::new()),
        }
    }

    pub unsafe fn init() {
        // SpawnOption::with_priority(Priority::High).spawn(Self::_usb_thread, "usb");
    }

    #[inline]
    fn shared<'a>() -> &'a Self {
        unsafe { &*USB_MANAGER.get() }
    }

    /// Initialize the USB device and tie it to the appropriate class driver.
    pub fn instantiate(addr: UsbDeviceAddress, ctx: Box<dyn UsbHostInterface>) {
        match UsbDevice::new(addr, ctx) {
            Ok(device) => {
                let device = Arc::new(device);
                Self::shared().devices.write().unwrap().push(device.clone());

                for interface in device.current_configuration().interfaces() {
                    let class = interface.class();
                    let if_no = interface.if_no();
                    if class == UsbClass::XINPUT {
                        super::class::xinput::XInput::start(&device, if_no);
                    } else if class.base() == UsbBaseClass::HID {
                        super::class::usb_hid::UsbHid::start(&device, if_no);
                    }
                }
            }
            Err(err) => {
                println!("USB Device Initialize Error {:?}", err);
            }
        }
    }

    pub fn devices() -> UsbDeviceIter {
        UsbDeviceIter { index: 0 }
    }

    pub fn device_by_addr(addr: UsbDeviceAddress) -> Option<Arc<UsbDevice>> {
        let devices = Self::shared().devices.read().unwrap();
        devices
            .binary_search_by_key(&addr, |v| v.addr())
            .ok()
            .and_then(|index| devices.get(index))
            .map(|v| v.clone())
    }

    // fn _usb_thread() {
    //     loop {
    //         Timer::sleep(Duration::from_millis(1000));
    //     }
    // }
}

pub struct UsbDeviceIter {
    index: usize,
}

impl Iterator for UsbDeviceIter {
    type Item = Arc<UsbDevice>;

    fn next(&mut self) -> Option<Self::Item> {
        let devices = UsbManager::shared().devices.read().unwrap();
        if self.index < devices.len() {
            match devices.get(self.index) {
                Some(device) => {
                    let result = device.clone();
                    self.index += 1;
                    drop(devices);
                    Some(result)
                }
                None => todo!(),
            }
        } else {
            None
        }
    }
}

pub struct UsbDevice {
    host: Box<dyn UsbHostInterface>,
    addr: UsbDeviceAddress,
    parent: Option<UsbDeviceAddress>,
    vid: UsbVendorId,
    pid: UsbProductId,
    class: UsbClass,
    manufacturer_string: Option<String>,
    product_string: Option<String>,
    serial_number: Option<String>,
    device_desc: UsbDeviceDescriptor,

    current_configuration: UsbConfigurationValue,
    configurations: Vec<UsbConfiguration>,
}

impl UsbDevice {
    #[inline]
    fn new(addr: UsbDeviceAddress, host: Box<dyn UsbHostInterface>) -> Result<Self, UsbError> {
        let device_desc: UsbDeviceDescriptor =
            match Self::get_descriptor(&host, UsbDescriptorType::Device, 0) {
                Ok(v) => v,
                Err(err) => return Err(err),
            };

        let manufacturer_string = device_desc
            .manufacturer_index()
            .and_then(|index| Self::get_string(&host, index).ok());
        let product_string = device_desc
            .product_index()
            .and_then(|index| Self::get_string(&host, index).ok());
        let serial_number = device_desc
            .serial_number_index()
            .and_then(|index| Self::get_string(&host, index).ok());

        let config_desc: UsbConfigurationDescriptor =
            match Self::get_descriptor(&host, UsbDescriptorType::Configuration, 0) {
                Ok(v) => v,
                Err(err) => return Err(err),
            };

        let blob = match host.control(UsbControlSetupData {
            bmRequestType: UsbControlRequestBitmap::GET_DEVICE,
            bRequest: UsbControlRequest::GET_DESCRIPTOR,
            wValue: (UsbDescriptorType::Configuration as u16) << 8,
            wIndex: 0,
            wLength: config_desc.total_length(),
        }) {
            Ok(v) => v,
            Err(err) => {
                println!("CONFIG DESCRIPTOR FAILED");
                return Err(err);
            }
        };

        let mut cursor = 0;
        let mut configurations = Vec::new();
        let mut current_configuration = None;
        let mut current_interface = None;
        while cursor < blob.len() {
            let len = blob[cursor] as usize;
            let desc_type_raw = blob[cursor + 1];

            let desc_type: UsbDescriptorType = match FromPrimitive::from_u8(desc_type_raw) {
                Some(v) => v,
                None => {
                    cursor += len;
                    continue;
                }
            };

            match desc_type {
                UsbDescriptorType::Configuration => {
                    let descriptor = unsafe {
                        &*(&blob[cursor] as *const _ as *const UsbConfigurationDescriptor)
                    };
                    if let Some(current_configuration) = current_configuration {
                        configurations.push(current_configuration);
                    }
                    current_configuration = Some(UsbConfiguration {
                        descriptor: *descriptor,
                        configuration_value: descriptor.configuration_value(),
                        interfaces: Vec::new(),
                    });
                    current_interface = None;
                }
                UsbDescriptorType::Interface => {
                    let descriptor =
                        unsafe { &*(&blob[cursor] as *const _ as *const UsbInterfaceDescriptor) };
                    let current_configuration = match current_configuration {
                        Some(ref mut v) => v,
                        None => {
                            println!("BAD USB Descriptor {:?}", addr);
                            return Err(UsbError::InvalidDescriptor);
                        }
                    };
                    if let Some(current_interface) = current_interface {
                        current_configuration.interfaces.push(current_interface);
                    }
                    current_interface = Some(UsbInterface {
                        descriptor: *descriptor,
                        class: descriptor.class(),
                        if_no: descriptor.if_no(),
                        alternate_setting: descriptor.alternate_setting(),
                        endpoints: Vec::new(),
                    });
                }
                UsbDescriptorType::Endpoint => {
                    let descriptor =
                        unsafe { &*(&blob[cursor] as *const _ as *const UsbEndpointDescriptor) };
                    let current_interface = match current_interface {
                        Some(ref mut v) => v,
                        None => {
                            return Err(UsbError::InvalidDescriptor);
                        }
                    };
                    let address = match descriptor.endpoint_address() {
                        Some(v) => v,
                        None => {
                            return Err(UsbError::InvalidDescriptor);
                        }
                    };
                    let ep_type = match descriptor.attributes() {
                        Some(v) => v,
                        None => {
                            return Err(UsbError::InvalidDescriptor);
                        }
                    };
                    current_interface.endpoints.push(UsbEndpoint {
                        descriptor: *descriptor,
                        address,
                        ep_type,
                    });
                }
                _ => (),
            }
            cursor += len;
        }

        let mut current_configuration = match current_configuration {
            Some(v) => v,
            None => {
                return Err(UsbError::InvalidDescriptor);
            }
        };
        let current_interface = match current_interface {
            Some(v) => v,
            None => {
                return Err(UsbError::InvalidDescriptor);
            }
        };
        current_configuration.interfaces.push(current_interface);
        configurations.push(current_configuration);

        let current_configuration = configurations.first().unwrap();
        Self::set_configuration(&host, current_configuration.configuration_value())?;

        Ok(Self {
            host,
            addr,
            parent: None,
            device_desc,
            vid: device_desc.vid(),
            pid: device_desc.pid(),
            class: device_desc.class(),
            manufacturer_string,
            product_string,
            serial_number,
            current_configuration: current_configuration.configuration_value(),
            configurations,
        })
    }

    /// Get an instance of the USB host controller interface that implements the [UsbHostInterface] trait.
    #[inline]
    pub fn host(&self) -> &Box<dyn UsbHostInterface> {
        &self.host
    }

    /// Get the USB address of this device.
    #[inline]
    pub const fn addr(&self) -> UsbDeviceAddress {
        self.addr
    }

    /// Get the vendor ID for this device.
    #[inline]
    pub const fn vid(&self) -> UsbVendorId {
        self.vid
    }

    /// Get the product ID for this device.
    #[inline]
    pub const fn pid(&self) -> UsbProductId {
        self.pid
    }

    /// Get the device class of this device.
    #[inline]
    pub const fn class(&self) -> UsbClass {
        self.class
    }

    /// Get the manufacturer's string for this device if possible.
    #[inline]
    pub fn manufacturer_string(&self) -> Option<&String> {
        self.manufacturer_string.as_ref()
    }

    /// Get the product name string for this device, if possible.
    #[inline]
    pub fn product_string(&self) -> Option<&String> {
        self.product_string.as_ref()
    }

    /// Get the serial number string of this device, if possible.
    #[inline]
    pub fn serial_number(&self) -> Option<&String> {
        self.serial_number.as_ref()
    }

    #[inline]
    pub fn device_desc(&self) -> &UsbDeviceDescriptor {
        &self.device_desc
    }

    #[inline]
    pub fn current_configuration(&self) -> &UsbConfiguration {
        self.configurations
            .binary_search_by_key(&self.current_configuration, |v| v.configuration_value)
            .ok()
            .and_then(|index| self.configurations.get(index))
            .unwrap()
    }

    #[inline]
    pub fn configurations(&self) -> &[UsbConfiguration] {
        self.configurations.as_slice()
    }

    /// Get descriptor
    pub fn get_descriptor<T: UsbDescriptor>(
        host: &Box<dyn UsbHostInterface>,
        desc_type: UsbDescriptorType,
        index: u8,
    ) -> Result<T, UsbError> {
        let mut result = MaybeUninit::<T>::zeroed();
        match host.control(UsbControlSetupData {
            bmRequestType: UsbControlRequestBitmap::GET_DEVICE,
            bRequest: UsbControlRequest::GET_DESCRIPTOR,
            wValue: (desc_type as u16) << 8 | index as u16,
            wIndex: 0,
            wLength: size_of::<T>() as u16,
        }) {
            Ok(v) => {
                let result = unsafe {
                    let p = result.as_mut_ptr();
                    let src = &v[0] as *const _ as *const T;
                    p.copy_from(src, 1);
                    result.assume_init()
                };
                if result.descriptor_type() == desc_type {
                    Ok(result)
                } else {
                    Err(UsbError::InvalidDescriptor)
                }
            }
            Err(err) => Err(err),
        }
    }

    /// Get string descriptor
    pub fn get_string(
        host: &Box<dyn UsbHostInterface>,
        index: NonZeroU8,
    ) -> Result<String, UsbError> {
        let mut setup = UsbControlSetupData {
            bmRequestType: UsbControlRequestBitmap::GET_DEVICE,
            bRequest: UsbControlRequest::GET_DESCRIPTOR,
            wValue: (UsbDescriptorType::String as u16) << 8 | index.get() as u16,
            wIndex: 0,
            wLength: 8,
        };

        let v = host.control(setup)?;
        let v = if v[0] > 8 {
            setup.wLength = v[0] as u16;
            host.control(setup)?
        } else {
            v
        };
        if v[1] != UsbDescriptorType::String as u8 {
            return Err(UsbError::InvalidDescriptor);
        }
        let len = v[0] as usize / 2 - 1;
        let v = unsafe { slice::from_raw_parts(&v[2] as *const _ as *const u16, len) };
        String::from_utf16(v).map_err(|_| UsbError::InvalidDescriptor)
    }

    /// set configuration
    pub fn set_configuration(
        host: &Box<dyn UsbHostInterface>,
        value: UsbConfigurationValue,
    ) -> Result<(), UsbError> {
        host.control(UsbControlSetupData {
            bmRequestType: UsbControlRequestBitmap::SET_DEVICE,
            bRequest: UsbControlRequest::SET_CONFIGURATION,
            wValue: value.0 as u16,
            wIndex: 0,
            wLength: 0,
        })
        .map(|_| ())
    }

    pub fn read<T: Sized>(
        &self,
        ep: UsbEndpointAddress,
        buffer: &mut T,
    ) -> Result<usize, UsbError> {
        let len = size_of::<T>();
        let buffer = buffer as *const _ as *mut u8;
        self.host.read(ep, buffer, len)
    }

    pub fn write<T: Sized>(&self, ep: UsbEndpointAddress, buffer: &T) -> Result<usize, UsbError> {
        let len = size_of::<T>();
        let buffer = buffer as *const _ as *const u8;
        self.host.write(ep, buffer, len)
    }
}

pub struct UsbConfiguration {
    descriptor: UsbConfigurationDescriptor,
    configuration_value: UsbConfigurationValue,
    interfaces: Vec<UsbInterface>,
}

impl UsbConfiguration {
    #[inline]
    pub const fn descriptor(&self) -> &UsbConfigurationDescriptor {
        &self.descriptor
    }

    #[inline]
    pub const fn configuration_value(&self) -> UsbConfigurationValue {
        self.configuration_value
    }

    #[inline]
    pub fn interfaces(&self) -> &[UsbInterface] {
        self.interfaces.as_slice()
    }

    #[inline]
    pub fn find_interface(
        &self,
        if_no: UsbInterfaceNumber,
        alt: Option<UsbAlternateSettingNumber>,
    ) -> Option<&UsbInterface> {
        for interface in self.interfaces.iter() {
            if interface.if_no() == if_no
                && (alt == None || alt == Some(interface.alternate_setting()))
            {
                return Some(interface);
            }
        }
        None
    }
}

pub struct UsbInterface {
    descriptor: UsbInterfaceDescriptor,
    class: UsbClass,
    if_no: UsbInterfaceNumber,
    alternate_setting: UsbAlternateSettingNumber,
    endpoints: Vec<UsbEndpoint>,
}

impl UsbInterface {
    #[inline]
    pub const fn descriptor(&self) -> &UsbInterfaceDescriptor {
        &self.descriptor
    }

    #[inline]
    pub const fn if_no(&self) -> UsbInterfaceNumber {
        self.if_no
    }

    #[inline]
    pub const fn alternate_setting(&self) -> UsbAlternateSettingNumber {
        self.alternate_setting
    }

    #[inline]
    pub const fn class(&self) -> UsbClass {
        self.class
    }

    #[inline]
    pub fn endpoints(&self) -> &[UsbEndpoint] {
        self.endpoints.as_slice()
    }
}

pub struct UsbEndpoint {
    descriptor: UsbEndpointDescriptor,
    address: UsbEndpointAddress,
    ep_type: UsbEndpointType,
}

impl UsbEndpoint {
    #[inline]
    pub const fn descriptor(&self) -> &UsbEndpointDescriptor {
        &self.descriptor
    }

    #[inline]
    pub const fn address(&self) -> UsbEndpointAddress {
        self.address
    }

    #[inline]
    pub const fn ep_type(&self) -> UsbEndpointType {
        self.ep_type
    }
}
