//! Universal Serial Bus

use super::types::*;
use crate::{
    sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard},
    task::scheduler::{Priority, SpawnOption, Timer},
};
use alloc::{boxed::Box, collections::binary_heap::Iter, sync::Arc, vec::Vec};
use core::{
    cell::UnsafeCell,
    marker::PhantomData,
    mem::{size_of, MaybeUninit},
    time::Duration,
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

    pub fn instantiate(addr: UsbDeviceAddress, ctx: Box<dyn UsbHostInterface>) {
        let device = UsbDevice::new(addr, ctx);
        match device.initialize() {
            Ok(_) => {
                let device = Arc::new(device);
                Self::shared().devices.write().unwrap().push(device.clone());

                // dummy driver
                let props = device.props();
                for interface in props.interfaces() {
                    if interface.class().base_class() == UsbBaseClassCode::HID {
                        super::class::usb_hid::UsbHid::start(device.clone(), interface.if_no());
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
    addr: UsbDeviceAddress,
    ctx: Box<dyn UsbHostInterface>,
    props: RwLock<UsbDeviceProps>,
}

unsafe impl Sync for UsbDevice {}

unsafe impl Send for UsbDevice {}

impl UsbDevice {
    #[inline]
    const fn new(addr: UsbDeviceAddress, ctx: Box<dyn UsbHostInterface>) -> Self {
        Self {
            addr,
            ctx,
            props: RwLock::new(UsbDeviceProps::empty()),
        }
    }

    #[inline]
    pub fn host(&self) -> &Box<dyn UsbHostInterface> {
        &self.ctx
    }

    #[inline]
    pub fn addr(&self) -> UsbDeviceAddress {
        self.addr
    }

    #[inline]
    pub fn props(&self) -> RwLockReadGuard<UsbDeviceProps> {
        self.props.read().unwrap()
    }

    fn initialize(&self) -> Result<(), UsbError> {
        let device_desc: UsbDeviceDescriptor =
            match self.get_descriptor(UsbDescriptorType::Device, 0) {
                Ok(v) => v,
                Err(err) => return Err(err),
            };

        let config_desc: UsbConfigurationDescriptor =
            match self.get_descriptor(UsbDescriptorType::Configuration, 0) {
                Ok(v) => v,
                Err(err) => return Err(err),
            };

        let blob = match self.ctx.control(UsbControlSetupData {
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
        let mut current_configuration = None;
        let mut configurations = Vec::new();
        let mut interfaces = Vec::new();
        let mut endpoints = Vec::new();
        let mut ep_bmps = Vec::new();
        let mut ep_bitmap = 0;
        let mut cursor = 0;
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
                    let desc = unsafe {
                        &*(&blob[cursor] as *const _ as *const UsbConfigurationDescriptor)
                    };
                    configurations.push(*desc);
                }
                UsbDescriptorType::Interface => {
                    let desc =
                        unsafe { &*(&blob[cursor] as *const _ as *const UsbInterfaceDescriptor) };
                    if interfaces.len() > 0 {
                        ep_bmps.push(ep_bitmap);
                    }
                    interfaces.push(*desc);
                    ep_bitmap = 0;
                }
                UsbDescriptorType::Endpoint => {
                    let desc =
                        unsafe { &*(&blob[cursor] as *const _ as *const UsbEndpointDescriptor) };
                    ep_bitmap |= 1u32 << desc.endpoint_address().unwrap().compact();
                    endpoints.push(*desc);
                }
                _ => (),
            }

            cursor += len;
        }
        ep_bmps.push(ep_bitmap);

        if let Some(configuration) = configurations.first() {
            self.set_configuration(configuration.configuration_value())?;
            current_configuration = Some(*configuration);
        }

        for endpoint in &endpoints {
            Timer::sleep(Duration::from_millis(10));
            self.ctx.configure_endpoint(endpoint)?;
        }

        let mut props = self.props.write().unwrap();
        props.device.write(device_desc);
        props.current_configuration = current_configuration;
        props.configurations = configurations;
        props.interfaces = interfaces;
        props.endpoints = endpoints;
        props.ep_bmps = ep_bmps;

        Ok(())
    }

    /// Get descriptor
    pub fn get_descriptor<T: UsbDescriptor>(
        &self,
        desc_type: UsbDescriptorType,
        index: u8,
    ) -> Result<T, UsbError> {
        let mut result = MaybeUninit::<T>::zeroed();
        match self.ctx.control(UsbControlSetupData {
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

    pub fn set_configuration(&self, index: u8) -> Result<(), UsbError> {
        self.ctx
            .control(UsbControlSetupData {
                bmRequestType: UsbControlRequestBitmap::SET_DEVICE,
                bRequest: UsbControlRequest::SET_CONFIGURATION,
                wValue: index as u16,
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
        self.ctx.read(ep, buffer, len)
    }
}

pub struct UsbDeviceProps {
    device: MaybeUninit<UsbDeviceDescriptor>,
    current_configuration: Option<UsbConfigurationDescriptor>,
    configurations: Vec<UsbConfigurationDescriptor>,
    interfaces: Vec<UsbInterfaceDescriptor>,
    endpoints: Vec<UsbEndpointDescriptor>,
    ep_bmps: Vec<u32>,
}

impl UsbDeviceProps {
    #[inline]
    const fn empty() -> Self {
        Self {
            device: MaybeUninit::uninit(),
            current_configuration: None,
            configurations: Vec::new(),
            interfaces: Vec::new(),
            endpoints: Vec::new(),
            ep_bmps: Vec::new(),
        }
    }

    #[inline]
    pub fn device<'a>(&self) -> &'a UsbDeviceDescriptor {
        unsafe { &*self.device.as_ptr() }
    }

    #[inline]
    pub const fn current_configuration(&self) -> Option<&UsbConfigurationDescriptor> {
        self.current_configuration.as_ref()
    }

    #[inline]
    pub fn configurations(&self) -> &[UsbConfigurationDescriptor] {
        self.configurations.as_slice()
    }

    #[inline]
    pub fn interface<'a>(&'a self, if_no: u8) -> Option<UsbInterface<'a>> {
        for (desc, ep_bitmap) in self.interfaces.iter().zip(self.ep_bmps.iter()) {
            if desc.if_no() == if_no {
                return Some(UsbInterface {
                    desc,
                    ep_bitmap: *ep_bitmap,
                });
            }
        }
        None
    }

    #[inline]
    pub fn interfaces(&self) -> &[UsbInterfaceDescriptor] {
        self.interfaces.as_slice()
    }

    #[inline]
    pub fn endpoints(&self) -> &[UsbEndpointDescriptor] {
        self.endpoints.as_slice()
    }

    #[inline]
    pub fn endpoint_by_bitmap(
        &self,
        bitmap: u32,
        is_dir_in: bool,
    ) -> Option<&UsbEndpointDescriptor> {
        let mask = if is_dir_in { 0xFFFF_0000 } else { 0x0000_FFFF };
        let ep_addr =
            UsbEndpointAddress::new((bitmap & mask).trailing_zeros() as u8, is_dir_in).unwrap();
        match self
            .endpoints
            .binary_search_by_key(&ep_addr, |v| v.endpoint_address().unwrap())
        {
            Ok(index) => self.endpoints.get(index),
            Err(_) => None,
        }
    }
}

pub struct UsbInterface<'a> {
    desc: &'a UsbInterfaceDescriptor,
    ep_bitmap: u32,
}

impl UsbInterface<'_> {
    #[inline]
    pub const fn descriptor(&self) -> &UsbInterfaceDescriptor {
        self.desc
    }

    #[inline]
    pub const fn endpoint_bitmap(&self) -> u32 {
        self.ep_bitmap
    }
}
