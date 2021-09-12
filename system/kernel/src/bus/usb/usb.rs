//! Universal Serial Bus
//!
//!   ┏━○
//! ○┻┳━|＞
//! ┗■

use super::types::*;
use crate::{
    sync::{fifo::AsyncEventQueue, RwLock},
    task::{scheduler::*, Task},
    *,
};
use alloc::{boxed::Box, string::String, sync::Arc, vec::Vec};
use core::{
    cell::UnsafeCell,
    mem::{size_of, MaybeUninit},
    num::NonZeroU8,
    pin::Pin,
    slice,
    sync::atomic::*,
    time::Duration,
};
use futures_util::Future;
use num_traits::FromPrimitive;

/// USB Driver to Host interface
pub trait UsbHostInterface {
    fn parent_device_address(&self) -> Option<UsbDeviceAddress>;

    fn speed(&self) -> PSIV;

    fn set_max_packet_size(&self, max_packet_size: usize) -> Result<(), UsbError>;

    fn enter_configuration(&self) -> Pin<Box<dyn Future<Output = Result<(), UsbError>>>>;

    fn leave_configuration(&self) -> Result<(), UsbError>;

    fn configure_endpoint(&self, desc: &UsbEndpointDescriptor) -> Result<(), UsbError>;

    fn configure_hub2(&self, hub_desc: &UsbHub2Descriptor, is_mtt: bool) -> Result<(), UsbError>;

    fn configure_hub3(
        &self,
        hub_desc: &UsbHub3Descriptor,
        max_exit_latency: usize,
    ) -> Result<(), UsbError>;

    fn attach_device(
        &self,
        port_id: UsbHubPortNumber,
        speed: PSIV,
        max_exit_latency: usize,
    ) -> Result<UsbDeviceAddress, UsbError>;

    /// Performs a control transfer
    fn control<'a>(&self, setup: UsbControlSetupData) -> Result<&'a [u8], UsbError>;

    unsafe fn control_send(
        &self,
        setup: UsbControlSetupData,
        data: *const u8,
    ) -> Result<usize, UsbError>;

    unsafe fn read(
        self: Arc<Self>,
        ep: UsbEndpointAddress,
        buffer: *mut u8,
        len: usize,
    ) -> Pin<Box<dyn Future<Output = Result<usize, UsbError>>>>;

    unsafe fn write(
        self: Arc<Self>,
        ep: UsbEndpointAddress,
        buffer: *const u8,
        len: usize,
    ) -> Pin<Box<dyn Future<Output = Result<usize, UsbError>>>>;
}

pub trait UsbClassDriverStarter {
    fn instantiate(&self, device: &UsbDevice) -> bool;
}

pub trait UsbInterfaceDriverStarter {
    fn instantiate(&self, device: &UsbDevice, interface: &UsbInterface) -> bool;
}

static mut USB_MANAGER: UnsafeCell<UsbManager> = UnsafeCell::new(UsbManager::new());

pub struct UsbManager {
    devices: RwLock<Vec<Arc<UsbDevice>>>,
    specific_driver_starters: RwLock<Vec<Arc<dyn UsbClassDriverStarter>>>,
    class_driver_starters: RwLock<Vec<Arc<dyn UsbClassDriverStarter>>>,
    interface_driver_starters: RwLock<Vec<Arc<dyn UsbInterfaceDriverStarter>>>,
    request_queue: MaybeUninit<AsyncEventQueue<Task>>,
}

impl UsbManager {
    const fn new() -> Self {
        Self {
            devices: RwLock::new(Vec::new()),
            specific_driver_starters: RwLock::new(Vec::new()),
            class_driver_starters: RwLock::new(Vec::new()),
            interface_driver_starters: RwLock::new(Vec::new()),
            request_queue: MaybeUninit::uninit(),
        }
    }

    pub unsafe fn init() {
        let shared = &mut *USB_MANAGER.get();
        shared.request_queue.write(AsyncEventQueue::new(100));

        SpawnOption::with_priority(Priority::High).spawn(Self::_usb_xfer_task_thread, "usb.xfer");

        // let mut vec = Self::shared().specific_driver_starters.write().unwrap();

        let mut vec = Self::shared().class_driver_starters.write().unwrap();
        vec.push(super::drivers::usb_hub::UsbHubStarter::new());

        let mut vec = Self::shared().interface_driver_starters.write().unwrap();
        vec.push(super::drivers::usb_hid::UsbHidStarter::new());
        vec.push(super::drivers::xinput::XInputStarter::new());
    }

    #[inline]
    fn shared<'a>() -> &'a Self {
        unsafe { &*USB_MANAGER.get() }
    }

    /// Initialize the USB device and tie it to the appropriate class driver.
    pub fn instantiate(addr: UsbDeviceAddress, ctx: Arc<dyn UsbHostInterface>) {
        match UsbDevice::new(addr, ctx) {
            Ok(device) => {
                let shared = Self::shared();
                let device = Arc::new(device);
                // let uuid = device.uuid();
                log!(
                    "USB connected: {} {:04x} {:04x} {:06x} {:?}",
                    addr.0,
                    device.vid().0,
                    device.pid().0,
                    device.class().0,
                    device.product_string(),
                );
                shared.devices.write().unwrap().push(device.clone());

                let mut issued = false;
                for driver in shared.specific_driver_starters.read().unwrap().iter() {
                    if driver.instantiate(&device) {
                        issued = true;
                        break;
                    }
                }
                if !issued {
                    for driver in shared.class_driver_starters.read().unwrap().iter() {
                        if driver.instantiate(&device) {
                            issued = true;
                            break;
                        }
                    }
                }
                if !issued {
                    for interface in device.current_configuration().interfaces() {
                        for driver in shared.interface_driver_starters.read().unwrap().iter() {
                            if driver.instantiate(&device, interface) {
                                issued = true;
                                break;
                            }
                        }
                    }
                }
                if issued {
                    device.is_configured.store(true, Ordering::SeqCst);
                }
            }
            Err(err) => {
                log!("USB Device Initialize Error {:?}", err);
            }
        }
    }

    pub fn detach_device(addr: UsbDeviceAddress) {
        let shared = Self::shared();
        let mut vec = shared.devices.write().unwrap();
        let index = match vec.binary_search_by_key(&addr, |v| v.addr()) {
            Ok(v) => v,
            Err(_) => return,
        };
        let _device = vec.remove(index);
        drop(vec);

        log!("USB disconnected: {}", addr.0);
    }

    pub fn devices() -> impl Iterator<Item = Arc<UsbDevice>> {
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

    /// Register a task for USB transfer.
    pub fn register_xfer_task(task: Task) {
        let shared = Self::shared();
        let queue = unsafe { shared.request_queue.as_ptr().as_ref().unwrap() };
        let _ = queue.post(task);
    }

    fn _usb_xfer_task_thread() {
        Scheduler::spawn_async(Task::new(Self::_usb_xfer_observer()));
        Scheduler::perform_tasks();
    }

    async fn _usb_xfer_observer() {
        let shared = Self::shared();
        let queue = unsafe { shared.request_queue.as_ptr().as_ref().unwrap() };
        loop {
            while let Some(new_task) = queue.wait_event().await {
                Scheduler::spawn_async(new_task);
            }
        }
    }
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

/// USB device instance type
pub struct UsbDevice {
    host: Arc<dyn UsbHostInterface>,
    addr: UsbDeviceAddress,
    uuid: [u8; 16],

    parent: Option<UsbDeviceAddress>,
    // children: Vec<UsbDeviceAddress>,
    vid: UsbVendorId,
    pid: UsbProductId,
    class: UsbClass,
    is_configured: AtomicBool,
    manufacturer_string: Option<String>,
    product_string: Option<String>,
    serial_number: Option<String>,
    descriptor: UsbDeviceDescriptor,

    #[allow(dead_code)]
    config_blob: Vec<u8>,
    #[allow(dead_code)]
    bos_blob: Option<Vec<u8>>,
    current_configuration: UsbConfigurationValue,
    configurations: Vec<UsbConfiguration>,
    ss_dev_cap: Option<UsbSuperspeedUsbDeviceCapability>,
}

impl UsbDevice {
    #[inline]
    fn new(addr: UsbDeviceAddress, host: Arc<dyn UsbHostInterface>) -> Result<Self, UsbError> {
        if host.speed() == PSIV::FS {
            // FullSpeed devices have to read the first 8 bytes of the device descriptor first and re-set the maximum packet size.
            let mut max_packet_size = 0;
            for _ in 0..5 {
                Timer::sleep(Duration::from_millis(50));
                match host.control(UsbControlSetupData {
                    bmRequestType: UsbControlRequestBitmap::GET_DEVICE,
                    bRequest: UsbControlRequest::GET_DESCRIPTOR,
                    wValue: (UsbDescriptorType::Device as u16) << 8,
                    wIndex: 0,
                    wLength: 8,
                }) {
                    Ok(v) => {
                        max_packet_size = v[7] as usize;
                        break;
                    }
                    Err(_) => (),
                }
            }
            if max_packet_size > 0 {
                let _ = host.set_max_packet_size(max_packet_size);
            }
            Timer::sleep(Duration::from_millis(10));
        }

        let device_desc: UsbDeviceDescriptor =
            match Self::get_device_descriptor(&host, UsbDescriptorType::Device, 0) {
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
            match Self::get_device_descriptor(&host, UsbDescriptorType::Configuration, 0) {
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
                log!("CONFIG DESCRIPTOR FAILED {}", addr.0);
                return Err(err);
            }
        };
        let config = {
            let mut vec = Vec::with_capacity(blob.len());
            vec.extend_from_slice(blob);
            vec
        };

        let mut cursor = 0;
        let mut configurations = Vec::new();
        let mut current_configuration = None;
        let mut current_interface = None;
        while cursor < config.len() {
            let len = config[cursor] as usize;
            let desc_type_raw = config[cursor + 1];

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
                        &*(&config[cursor] as *const _ as *const UsbConfigurationDescriptor)
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
                        unsafe { &*(&config[cursor] as *const _ as *const UsbInterfaceDescriptor) };
                    let current_configuration = match current_configuration {
                        Some(ref mut v) => v,
                        None => {
                            log!("BAD USB Descriptor {:?}", addr);
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
                        hid_reports: Vec::new(),
                    });
                }
                UsbDescriptorType::Endpoint => {
                    let descriptor =
                        unsafe { &*(&config[cursor] as *const _ as *const UsbEndpointDescriptor) };
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
                    let ep_type = match descriptor.ep_type() {
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
                UsbDescriptorType::HidClass => {
                    let descriptor =
                        unsafe { &*(&config[cursor] as *const _ as *const UsbHidClassDescriptor) };
                    let current_interface = match current_interface {
                        Some(ref mut v) => v,
                        None => {
                            return Err(UsbError::InvalidDescriptor);
                        }
                    };
                    for report in descriptor.children() {
                        current_interface.hid_reports.push(report);
                    }
                }
                _ => {
                    // log!(
                    //     "USB {} UNKNOWN DESCRIPTOR {} {:02x}",
                    //     addr.0,
                    //     config[cursor],
                    //     config[cursor + 1]
                    // );
                }
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

        // Binary Device Object Storage
        let mut uuid = [0u8; 16];
        let mut ss_dev_cap = None;
        let mut bos_blob = None;
        if let Some(bos_desc) = (device_desc.usb_version() >= UsbVersion(0x0201))
            .then(|| {
                Self::get_device_descriptor::<UsbBinaryObjectStoreDescriptor>(
                    &host,
                    UsbDescriptorType::Bos,
                    0,
                )
                .ok()
            })
            .flatten()
        {
            let blob = match host.control(UsbControlSetupData {
                bmRequestType: UsbControlRequestBitmap::GET_DEVICE,
                bRequest: UsbControlRequest::GET_DESCRIPTOR,
                wValue: (UsbDescriptorType::Bos as u16) << 8,
                wIndex: 0,
                wLength: bos_desc.total_length(),
            }) {
                Ok(v) => v,
                Err(err) => {
                    log!("BOS DESCRIPTOR FAILED {}", addr.0);
                    return Err(err);
                }
            };
            let bos = {
                let mut vec = Vec::with_capacity(blob.len());
                vec.extend_from_slice(blob);
                vec
            };

            let mut cursor = 0;
            while cursor < bos.len() {
                let len = bos[cursor] as usize;
                let cap_type: UsbDeviceCapabilityType = match (bos[cursor + 1]
                    == UsbDescriptorType::DeviceCapability as u8)
                    .then(|| FromPrimitive::from_u8(bos[cursor + 2]))
                    .flatten()
                {
                    Some(v) => v,
                    None => {
                        cursor += len;
                        continue;
                    }
                };

                match cap_type {
                    UsbDeviceCapabilityType::SuperspeedUsb => {
                        let descriptor = unsafe {
                            &*(&config[cursor] as *const _
                                as *const UsbSuperspeedUsbDeviceCapability)
                        };
                        ss_dev_cap = Some(*descriptor);
                    }
                    UsbDeviceCapabilityType::ContainerId => {
                        let descriptor = unsafe {
                            &*(&config[cursor] as *const _ as *const UsbContainerIdCapability)
                        };
                        uuid.copy_from_slice(descriptor.uuid());
                    }
                    _ => {
                        // log!(
                        //     "USB {} UNKNOWN CAPABILITY {} {:02x}",
                        //     addr.0,
                        //     bos[cursor],
                        //     bos[cursor + 2],
                        // );
                    }
                }
                cursor += len;
            }

            bos_blob = Some(bos);
        }

        if let Some(ss_dev_cap) = ss_dev_cap.as_ref() {
            if host.speed() == PSIV::SS {
                Self::set_sel(&host, &Usb3ExitLatencyValues::from_ss_dev_cap(ss_dev_cap)).unwrap();
            }
        }

        let parent = host.parent_device_address();

        Ok(Self {
            host,
            addr,
            uuid,
            parent,
            descriptor: device_desc,
            vid: device_desc.vid(),
            pid: device_desc.pid(),
            class: device_desc.class(),
            is_configured: AtomicBool::new(false),
            manufacturer_string,
            product_string,
            serial_number,
            config_blob: config,
            bos_blob,
            current_configuration: current_configuration.configuration_value(),
            configurations,
            ss_dev_cap,
        })
    }

    /// Gets an instance of the USB host controller interface that implements the [UsbHostInterface] trait.
    #[inline]
    pub fn host(&self) -> Arc<dyn UsbHostInterface> {
        self.host.clone()
    }

    /// Gets the USB device address of the parent device.
    #[inline]
    pub const fn parent_device_address(&self) -> Option<UsbDeviceAddress> {
        self.parent
    }

    /// Gets the USB address of this device.
    #[inline]
    pub const fn addr(&self) -> UsbDeviceAddress {
        self.addr
    }

    #[inline]
    pub const fn uuid(&self) -> &[u8; 16] {
        &self.uuid
    }

    /// Gets the vendor ID for this device.
    #[inline]
    pub const fn vid(&self) -> UsbVendorId {
        self.vid
    }

    /// Gets the product ID for this device.
    #[inline]
    pub const fn pid(&self) -> UsbProductId {
        self.pid
    }

    #[inline]
    pub const fn vid_pid_raw(&self) -> (u16, u16) {
        (self.vid().0, self.pid().0)
    }

    /// Get the device class of this device.
    #[inline]
    pub const fn class(&self) -> UsbClass {
        self.class
    }

    #[inline]
    pub fn is_configured(&self) -> bool {
        self.is_configured.load(Ordering::Relaxed)
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
    pub fn descriptor(&self) -> &UsbDeviceDescriptor {
        &self.descriptor
    }

    #[inline]
    pub fn ss_dev_cap(&self) -> Option<&UsbSuperspeedUsbDeviceCapability> {
        self.ss_dev_cap.as_ref()
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

    // pub async fn control_recv(
    //     &self,
    //     mut setup: UsbControlSetupData,
    //     data: &mut [u8],
    // ) -> Result<(), UsbError> {
    //     setup.wLength = data.len() as u16;
    //     host.control()
    // }

    // pub async fn control_send(&self, request: u16, value: u16, index: u16, data: &[u8]) {
    //     todo!()
    // }

    pub async fn read<T: Sized>(
        &self,
        ep: UsbEndpointAddress,
        buffer: &mut T,
    ) -> Result<(), UsbError> {
        let len = size_of::<T>();
        unsafe { self.host().read(ep, buffer as *const _ as *mut u8, len) }
            .await
            .and_then(|result| {
                if result == len {
                    Ok(())
                } else {
                    Err(UsbError::ShortPacket)
                }
            })
    }

    pub async fn read_vec(
        &self,
        ep: UsbEndpointAddress,
        buffer: &mut Vec<u8>,
        min_len: usize,
        max_len: usize,
    ) -> Result<(), UsbError> {
        buffer.resize(max_len, 0);
        self.read_slice(ep, buffer.as_mut_slice(), min_len, max_len)
            .await
            .map(|new_len| {
                buffer.resize(new_len, 0);
                ()
            })
    }

    pub async fn read_slice(
        &self,
        ep: UsbEndpointAddress,
        buffer: &mut [u8],
        min_len: usize,
        max_len: usize,
    ) -> Result<usize, UsbError> {
        let raw_buffer = match buffer.get(0) {
            Some(v) => v as *const _ as *mut u8,
            None => return Err(UsbError::InvalidParameter),
        };
        if max_len > buffer.len() || min_len > max_len {
            return Err(UsbError::InvalidParameter);
        }
        unsafe { self.host().read(ep, raw_buffer, max_len) }
            .await
            .and_then(|result| {
                if result >= min_len {
                    Ok(result)
                } else {
                    Err(UsbError::ShortPacket)
                }
            })
    }

    pub async fn write<T: Sized>(
        &self,
        ep: UsbEndpointAddress,
        buffer: &T,
    ) -> Result<(), UsbError> {
        let len = size_of::<T>();
        unsafe { self.host().write(ep, buffer as *const _ as *const u8, len) }
            .await
            .and_then(|result| {
                if result == len {
                    Ok(())
                } else {
                    Err(UsbError::ShortPacket)
                }
            })
    }

    pub async fn write_slice(&self, ep: UsbEndpointAddress, buffer: &[u8]) -> Result<(), UsbError> {
        let raw_buffer = match buffer.get(0) {
            Some(v) => v as *const _ as *mut u8,
            None => return Err(UsbError::InvalidParameter),
        };
        let len = buffer.len();
        unsafe { self.host().write(ep, raw_buffer, len) }
            .await
            .and_then(|result| {
                if result == len {
                    Ok(())
                } else {
                    Err(UsbError::ShortPacket)
                }
            })
    }

    /// Get the descriptor associated with a device
    #[inline]
    pub fn get_device_descriptor<T: UsbDescriptor>(
        host: &Arc<dyn UsbHostInterface>,
        desc_type: UsbDescriptorType,
        index: u8,
    ) -> Result<T, UsbError> {
        Self::get_descriptor(host, UsbControlRequestBitmap::GET_DEVICE, desc_type, index)
    }

    /// Get the descriptor associated with a device
    pub fn get_descriptor<T: UsbDescriptor>(
        host: &Arc<dyn UsbHostInterface>,
        request_type: UsbControlRequestBitmap,
        desc_type: UsbDescriptorType,
        index: u8,
    ) -> Result<T, UsbError> {
        let mut result = MaybeUninit::<T>::zeroed();
        match host.control(UsbControlSetupData {
            bmRequestType: request_type,
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
        host: &Arc<dyn UsbHostInterface>,
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
        host: &Arc<dyn UsbHostInterface>,
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

    /// Set exit latency values
    pub fn set_sel(
        host: &Arc<dyn UsbHostInterface>,
        values: &Usb3ExitLatencyValues,
    ) -> Result<(), UsbError> {
        let length = 6;
        let data = values as *const _ as *const u8;
        unsafe {
            host.control_send(
                UsbControlSetupData {
                    bmRequestType: UsbControlRequestBitmap(0x00),
                    bRequest: UsbControlRequest::SET_SEL,
                    wValue: 0,
                    wIndex: 0,
                    wLength: length,
                },
                data,
            )
        }
        .map(|_| ())
    }
}

/// USB configuration instance type
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

/// USB interface instance type
pub struct UsbInterface {
    descriptor: UsbInterfaceDescriptor,
    class: UsbClass,
    if_no: UsbInterfaceNumber,
    alternate_setting: UsbAlternateSettingNumber,
    endpoints: Vec<UsbEndpoint>,
    hid_reports: Vec<(UsbDescriptorType, u16)>,
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

    #[inline]
    pub fn hid_reports(&self) -> &[(UsbDescriptorType, u16)] {
        self.hid_reports.as_slice()
    }
}

/// USB endpoint instance type
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
