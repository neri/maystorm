//! Universal Serial Bus

use super::types::*;
use crate::{
    log::EventManager,
    sync::{fifo::AsyncEventQueue, RwLock},
    task::{scheduler::*, Task},
    *,
};
use alloc::{boxed::Box, collections::BTreeMap, string::String, sync::Arc, vec::Vec};
use core::{
    cell::UnsafeCell,
    mem::{size_of, MaybeUninit},
    num::NonZeroU8,
    ops::ControlFlow,
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

    unsafe fn enter_configuration(&self) -> Pin<Box<dyn Future<Output = Result<(), UsbError>>>>;

    unsafe fn leave_configuration(&self) -> Result<(), UsbError>;

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
    unsafe fn control<'a>(&self, setup: UsbControlSetupData) -> Result<&'a [u8], UsbError>;

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

        let mut specific_drivers = Self::shared().specific_driver_starters.write().unwrap();
        let mut class_drivers = Self::shared().class_driver_starters.write().unwrap();
        let mut interface_drivers = Self::shared().interface_driver_starters.write().unwrap();
        super::drivers::install_drivers(
            &mut specific_drivers,
            &mut class_drivers,
            &mut interface_drivers,
        );
    }

    #[inline]
    fn shared<'a>() -> &'a Self {
        unsafe { &*USB_MANAGER.get() }
    }

    /// Initialize the USB device and tie it to the appropriate class driver.
    pub fn instantiate(addr: UsbDeviceAddress, ctx: Arc<dyn UsbHostInterface>) -> bool {
        match UsbDevice::new(addr, ctx) {
            Ok(device) => {
                let shared = Self::shared();
                let device = Arc::new(device);

                // log!(
                //     "USB connected: {} {} {} {} {:?}",
                //     addr.0,
                //     device.vid(),
                //     device.pid(),
                //     device.class(),
                //     device.product_string(),
                // );

                shared.devices.write().unwrap().push(device.clone());

                let mut is_configured = false;
                for driver in shared.specific_driver_starters.read().unwrap().iter() {
                    if driver.instantiate(&device) {
                        is_configured = true;
                        break;
                    }
                }
                if !is_configured {
                    for driver in shared.class_driver_starters.read().unwrap().iter() {
                        if driver.instantiate(&device) {
                            is_configured = true;
                            break;
                        }
                    }
                }
                if !is_configured {
                    for interface in device.current_configuration().interfaces() {
                        for driver in shared.interface_driver_starters.read().unwrap().iter() {
                            if driver.instantiate(&device, interface) {
                                is_configured = true;
                                break;
                            }
                        }
                    }
                }

                if is_configured {
                    device.is_configured.store(true, Ordering::SeqCst);
                    if let Some(product_string) = device.product_string() {
                        notify!("USB Device \"{}\" has been configured.", product_string);
                    } else {
                        if let Some(name) = device.class().class_string(false) {
                            notify!("USB Device \"{}\" has been configured.", name);
                        } else {
                            notify!("A USB Device has been configured.");
                        }
                    }
                } else {
                    if let Some(product_string) = device.product_string() {
                        notify!("USB Device \"{}\" was found.", product_string);
                    } else {
                        if let Some(name) = device.class().class_string(false) {
                            notify!("USB Device \"{}\" was found.", name);
                        } else {
                            notify!("A USB Device was found.");
                        }
                    }
                }

                true
            }
            Err(err) => {
                log!("USB Device Initialize Error {:?}", err);
                false
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
    is_configured: AtomicBool,
    manufacturer_string: Option<String>,
    product_string: Option<String>,
    serial_number: Option<String>,
    descriptor: UsbDeviceDescriptor,

    config_blob: Vec<u8>,

    current_configuration: UsbConfigurationValue,
    configurations: Vec<UsbConfiguration>,
    bos: UsbBinaryObjectStore,
}

impl UsbDevice {
    #[inline]
    fn new(addr: UsbDeviceAddress, host: Arc<dyn UsbHostInterface>) -> Result<Self, UsbError> {
        let mut device_desc: Option<UsbDeviceDescriptor> = None;
        for _ in 0..5 {
            if host.speed() == PSIV::FS {
                // FullSpeed devices have to read the first 8 bytes of the device descriptor first and re-set the maximum packet size.
                let mut packet = [0; 8];
                match Self::_control_slice(
                    &host,
                    UsbControlSetupData::request(
                        UsbControlRequestBitmap::GET_DEVICE,
                        UsbControlRequest::GET_DESCRIPTOR,
                    )
                    .value((UsbDescriptorType::Device as u16) << 8)
                    .length(8),
                    &mut packet,
                ) {
                    Ok(_) => {
                        let max_packet_size = packet[7] as usize;
                        let _ = host.set_max_packet_size(max_packet_size);
                    }
                    Err(_) => (),
                }
                Timer::sleep(Duration::from_millis(10));
            }
            match Self::_get_device_descriptor(&host, UsbDescriptorType::Device, 0) {
                Ok(v) => {
                    device_desc = Some(v);
                    break;
                }
                Err(_err) => (),
            };
        }
        let device_desc = match device_desc {
            Some(v) => v,
            None => {
                log!("DEVICE DESCRIPTOR ERROR {}", addr.0.get());
                return Err(UsbError::InvalidDescriptor);
            }
        };

        // log!(
        //     "DEVICE {} {} {} {} v{}",
        //     addr.0.get(),
        //     device_desc.vid(),
        //     device_desc.pid(),
        //     device_desc.class(),
        //     device_desc.usb_version(),
        // );

        let manufacturer_string = device_desc
            .manufacturer_index()
            .and_then(|index| Self::_get_string(&host, index).ok());
        let product_string = device_desc
            .product_index()
            .and_then(|index| Self::_get_string(&host, index).ok());
        let serial_number = device_desc
            .serial_number_index()
            .and_then(|index| Self::_get_string(&host, index).ok());

        let prot_config_desc: UsbConfigurationDescriptor =
            match Self::_get_device_descriptor(&host, UsbDescriptorType::Configuration, 0) {
                Ok(v) => v,
                Err(err) => return Err(err),
            };

        let mut config = Vec::new();
        match Self::_control_vec(
            &host,
            UsbControlSetupData {
                bmRequestType: UsbControlRequestBitmap::GET_DEVICE,
                bRequest: UsbControlRequest::GET_DESCRIPTOR,
                wValue: (UsbDescriptorType::Configuration as u16) << 8,
                wIndex: 0,
                wLength: prot_config_desc.total_length(),
            },
            &mut config,
        ) {
            Ok(_) => (),
            Err(err) => {
                log!("CONFIG DESCRIPTOR FAILED {}", addr.0);
                return Err(err);
            }
        }

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
                    let name = descriptor
                        .configuration_index()
                        .and_then(|index| Self::_get_string(&host, index).ok());
                    current_configuration = Some(UsbConfiguration {
                        descriptor: *descriptor,
                        configuration_value: descriptor.configuration_value(),
                        interfaces: Vec::new(),
                        name,
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
                    let name = descriptor
                        .interface_index()
                        .and_then(|index| Self::_get_string(&host, index).ok());
                    current_interface = Some(UsbInterface {
                        descriptor: *descriptor,
                        endpoints: Vec::new(),
                        hid_reports: BTreeMap::new(),
                        name,
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
                    for (report_type, len) in descriptor.children() {
                        let mut vec = Vec::new();
                        match Self::_get_hid_descriptor(
                            &host,
                            current_interface.if_no(),
                            report_type,
                            0,
                            len as usize,
                            &mut vec,
                        ) {
                            Ok(_) => (),
                            Err(_err) => {
                                // log!("ERR HID {:02x} {:?}", report_type as usize, _err);
                                // return Err(UsbError::InvalidDescriptor);
                            }
                        };
                        current_interface.hid_reports.insert(report_type, vec);
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
        Self::_set_configuration(&host, current_configuration.configuration_value())?;

        // Binary Device Object Storage
        let mut bos = UsbBinaryObjectStore::empty();
        let mut uuid = [0u8; 16];
        if let Some(bos_desc) = (device_desc.usb_version() >= UsbVersion::BOS_MIN)
            .then(|| {
                Self::_get_device_descriptor::<UsbBinaryObjectStoreDescriptor>(
                    &host,
                    UsbDescriptorType::Bos,
                    0,
                )
                .ok()
            })
            .flatten()
        {
            let mut bos_blob = Vec::new();
            match Self::_control_vec(
                &host,
                UsbControlSetupData {
                    bmRequestType: UsbControlRequestBitmap::GET_DEVICE,
                    bRequest: UsbControlRequest::GET_DESCRIPTOR,
                    wValue: (UsbDescriptorType::Bos as u16) << 8,
                    wIndex: 0,
                    wLength: bos_desc.total_length(),
                },
                &mut bos_blob,
            ) {
                Ok(_) => (),
                Err(err) => {
                    log!("BOS DESCRIPTOR FAILED {}", addr.0);
                    return Err(err);
                }
            }

            let mut cursor = 0;
            while cursor < bos_blob.len() {
                let len = bos_blob[cursor] as usize;
                let cap_type: UsbDeviceCapabilityType = match (bos_blob[cursor + 1]
                    == UsbDescriptorType::DeviceCapability as u8)
                    .then(|| FromPrimitive::from_u8(bos_blob[cursor + 2]))
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
                            &*(&config[cursor] as *const _ as *const UsbSsDeviceCapability)
                        };
                        bos.ss_dev_cap = Some(*descriptor);
                    }
                    UsbDeviceCapabilityType::ContainerId => {
                        let descriptor = unsafe {
                            &*(&config[cursor] as *const _ as *const UsbContainerIdCapability)
                        };
                        uuid.copy_from_slice(descriptor.uuid());
                        bos.container_id = Some(*descriptor);
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

            bos.raw = bos_blob;
        }

        // if let Some(ss_dev_cap) = ss_dev_cap.as_ref() {
        //     if host.speed() == PSIV::SS {
        //         Self::set_sel(&host, &Usb3ExitLatencyValues::from_ss_dev_cap(ss_dev_cap)).unwrap();
        //     }
        // }

        let parent = host.parent_device_address();

        Ok(Self {
            host,
            addr,
            uuid,
            parent,
            descriptor: device_desc,
            is_configured: AtomicBool::new(false),
            manufacturer_string,
            product_string,
            serial_number,
            config_blob: config,
            current_configuration: current_configuration.configuration_value(),
            configurations,
            bos,
        })
    }

    /// Gets an instance of the USB host controller interface that implements the [UsbHostInterface] trait.
    #[inline]
    pub fn host(&self) -> &dyn UsbHostInterface {
        self.host.as_ref()
    }

    fn host_clone(&self) -> Arc<dyn UsbHostInterface> {
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

    /// Get the uuid of this device if possible.
    #[inline]
    pub const fn uuid(&self) -> &[u8; 16] {
        &self.uuid
    }

    /// Gets the vendor ID for this device.
    #[inline]
    pub const fn vid(&self) -> UsbVendorId {
        self.descriptor.vid()
    }

    /// Gets the product ID for this device.
    #[inline]
    pub const fn pid(&self) -> UsbProductId {
        self.descriptor.pid()
    }

    /// Get the device class of this device.
    #[inline]
    pub const fn class(&self) -> UsbClass {
        self.descriptor.class()
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

    /// Get the device descriptor for this device.
    #[inline]
    pub fn descriptor(&self) -> &UsbDeviceDescriptor {
        &self.descriptor
    }

    /// Get raw configuration descriptors.
    #[inline]
    pub fn config_raw(&self) -> &[u8] {
        self.config_blob.as_slice()
    }

    #[inline]
    pub fn bos(&self) -> &UsbBinaryObjectStore {
        &self.bos
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

    #[inline]
    pub fn configure_endpoint(&self, desc: &UsbEndpointDescriptor) -> Result<(), UsbError> {
        self.host().configure_endpoint(desc)
    }

    // pub async fn control_send(
    //     &self,
    //     request: u16,
    //     value: u16,
    //     index: u16,
    //     data: &[u8],
    // ) -> Result<usize, UsbError> {
    //     let setup = UsbControlSetupData {
    //         bmRequestType: UsbControlRequestBitmap(request as u8),
    //         bRequest: UsbControlRequest(request as u8),
    //         wValue: value,
    //         wIndex: index,
    //         wLength: data.len() as u16,
    //     };
    //     let data = unsafe { data.get_unchecked(0) as *const _ };
    //     unsafe { self.host().control_send(setup, data) }
    // }

    pub async fn read<T: Sized>(
        &self,
        ep: UsbEndpointAddress,
        buffer: &mut T,
    ) -> Result<(), UsbError> {
        let len = size_of::<T>();
        unsafe {
            self.host_clone()
                .read(ep, buffer as *const _ as *mut u8, len)
        }
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
        unsafe { self.host_clone().read(ep, raw_buffer, max_len) }
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
        unsafe {
            self.host_clone()
                .write(ep, buffer as *const _ as *const u8, len)
        }
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
        unsafe { self.host_clone().write(ep, raw_buffer, len) }
            .await
            .and_then(|result| {
                if result == len {
                    Ok(())
                } else {
                    Err(UsbError::ShortPacket)
                }
            })
    }

    pub fn control_nodata(&self, setup: UsbControlSetupData) -> Result<(), UsbError> {
        Self::_control_nodata(&self.host_clone(), setup)
    }

    pub fn control_var(
        &self,
        setup: UsbControlSetupData,
        vec: &mut Vec<u8>,
        min_len: usize,
        max_len: usize,
    ) -> Result<(), UsbError> {
        Self::_control_var(&self.host_clone(), setup, vec, min_len, max_len)
    }

    pub fn control_vec(
        &self,
        setup: UsbControlSetupData,
        vec: &mut Vec<u8>,
    ) -> Result<(), UsbError> {
        Self::_control_vec(&self.host_clone(), setup, vec)
    }

    pub fn control_slice(
        &self,
        setup: UsbControlSetupData,
        data: &mut [u8],
    ) -> Result<(), UsbError> {
        Self::_control_slice(&self.host_clone(), setup, data)
    }

    pub fn control_send(
        &self,
        mut setup: UsbControlSetupData,
        max_len: usize,
        data: &[u8],
    ) -> Result<(), UsbError> {
        if max_len > data.len() {
            return Err(UsbError::InvalidParameter);
        }
        setup.wLength = max_len as u16;
        let data = match data.get(0) {
            Some(v) => v as *const _ as *const u8,
            None => return Err(UsbError::InvalidParameter),
        };
        match unsafe { self.host().control_send(setup, data) } {
            Ok(_v) => Ok(()),
            Err(err) => Err(err),
        }
    }

    fn _control_nodata(
        host: &Arc<dyn UsbHostInterface>,
        setup: UsbControlSetupData,
    ) -> Result<(), UsbError> {
        if setup.wLength > 0 {
            return Err(UsbError::InvalidParameter);
        }
        unsafe { host.control(setup).map(|_| ()) }
    }

    fn _control_var(
        host: &Arc<dyn UsbHostInterface>,
        mut setup: UsbControlSetupData,
        vec: &mut Vec<u8>,
        min_len: usize,
        max_len: usize,
    ) -> Result<(), UsbError> {
        vec.resize(0, 0);
        setup.wLength = max_len as u16;
        unsafe { host.control(setup) }.and_then(|p| {
            if p.len() >= min_len {
                vec.extend_from_slice(p);
                Ok(())
            } else {
                Err(UsbError::ShortPacket)
            }
        })
    }

    fn _control_vec(
        host: &Arc<dyn UsbHostInterface>,
        setup: UsbControlSetupData,
        vec: &mut Vec<u8>,
    ) -> Result<(), UsbError> {
        vec.resize(0, 0);
        unsafe { host.control(setup) }.and_then(|p| {
            if p.len() == setup.wLength as usize {
                vec.extend_from_slice(p);
                Ok(())
            } else {
                Err(UsbError::ShortPacket)
            }
        })
    }

    fn _control_slice(
        host: &Arc<dyn UsbHostInterface>,
        mut setup: UsbControlSetupData,
        data: &mut [u8],
    ) -> Result<(), UsbError> {
        setup.wLength = data.len() as u16;
        unsafe { host.control(setup) }.and_then(|p| {
            if p.len() == data.len() {
                data.copy_from_slice(p);
                Ok(())
            } else {
                Err(UsbError::ShortPacket)
            }
        })
    }

    #[inline]
    fn _get_device_descriptor<T: UsbDescriptor>(
        host: &Arc<dyn UsbHostInterface>,
        desc_type: UsbDescriptorType,
        index: u8,
    ) -> Result<T, UsbError> {
        Self::_get_descriptor(host, UsbControlRequestBitmap::GET_DEVICE, desc_type, index)
    }

    /// Get the descriptor associated with a device
    pub fn get_descriptor<T: UsbDescriptor>(
        &self,
        request_type: UsbControlRequestBitmap,
        desc_type: UsbDescriptorType,
        index: u8,
    ) -> Result<T, UsbError> {
        Self::_get_descriptor(&self.host_clone(), request_type, desc_type, index)
    }

    fn _get_descriptor<T: UsbDescriptor>(
        host: &Arc<dyn UsbHostInterface>,
        request_type: UsbControlRequestBitmap,
        desc_type: UsbDescriptorType,
        index: u8,
    ) -> Result<T, UsbError> {
        let mut result = MaybeUninit::<T>::zeroed();
        match unsafe {
            host.control(UsbControlSetupData {
                bmRequestType: request_type,
                bRequest: UsbControlRequest::GET_DESCRIPTOR,
                wValue: (desc_type as u16) << 8 | index as u16,
                wIndex: 0,
                wLength: size_of::<T>() as u16,
            })
        } {
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
    pub fn get_string(&self, index: NonZeroU8) -> Result<String, UsbError> {
        Self::_get_string(&self.host_clone(), index)
    }

    fn _get_string(host: &Arc<dyn UsbHostInterface>, index: NonZeroU8) -> Result<String, UsbError> {
        let setup = UsbControlSetupData::request(
            UsbControlRequestBitmap::GET_DEVICE,
            UsbControlRequest::GET_DESCRIPTOR,
        )
        .value((UsbDescriptorType::String as u16) << 8 | index.get() as u16);

        let mut vec = Vec::new();
        Self::_control_var(host, setup, &mut vec, 4, 255)?;
        if vec[1] != UsbDescriptorType::String as u8 {
            return Err(UsbError::InvalidDescriptor);
        }

        let len = vec[0] as usize / 2 - 1;
        let vec = unsafe { slice::from_raw_parts(&vec[2] as *const _ as *const u16, len) };
        String::from_utf16(vec).map_err(|_| UsbError::InvalidDescriptor)
    }

    /// set configuration
    pub fn set_configuration(&self, value: UsbConfigurationValue) -> Result<(), UsbError> {
        Self::_set_configuration(&self.host_clone(), value)
    }

    fn _set_configuration(
        host: &Arc<dyn UsbHostInterface>,
        value: UsbConfigurationValue,
    ) -> Result<(), UsbError> {
        Self::_control_nodata(
            host,
            UsbControlSetupData {
                bmRequestType: UsbControlRequestBitmap::SET_DEVICE,
                bRequest: UsbControlRequest::SET_CONFIGURATION,
                wValue: value.0 as u16,
                wIndex: 0,
                wLength: 0,
            },
        )
    }

    #[inline]
    pub fn clear_device_feature(&self, feature_sel: UsbDeviceFeatureSel) -> Result<(), UsbError> {
        Self::_control_nodata(
            &self.host_clone(),
            UsbControlSetupData::request(
                UsbControlRequestBitmap::SET_DEVICE,
                UsbControlRequest::CLEAR_FEATURE,
            )
            .value(feature_sel as u16),
        )
    }

    #[inline]
    pub fn set_device_feature(&self, feature_sel: UsbDeviceFeatureSel) -> Result<(), UsbError> {
        Self::_control_nodata(
            &self.host_clone(),
            UsbControlSetupData::request(
                UsbControlRequestBitmap::SET_DEVICE,
                UsbControlRequest::SET_FEATURE,
            )
            .value(feature_sel as u16),
        )
    }

    /// Set exit latency values
    pub fn set_sel(&self, values: &Usb3ExitLatencyValues) -> Result<(), UsbError> {
        Self::_set_sel(&self.host_clone(), values)
    }

    fn _set_sel(
        host: &Arc<dyn UsbHostInterface>,
        values: &Usb3ExitLatencyValues,
    ) -> Result<(), UsbError> {
        let length = 6;
        let data = values as *const _ as *const u8;
        unsafe {
            host.control_send(
                UsbControlSetupData::request(
                    UsbControlRequestBitmap::SET_DEVICE,
                    UsbControlRequest::SET_SEL,
                )
                .length(length),
                data,
            )
        }
        .map(|_| ())
    }

    #[inline]
    pub fn get_hid_descriptor(
        &self,
        if_no: UsbInterfaceNumber,
        report_type: UsbDescriptorType,
        report_id: u8,
        len: usize,
        vec: &mut Vec<u8>,
    ) -> Result<(), UsbError> {
        Self::_get_hid_descriptor(&self.host_clone(), if_no, report_type, report_id, len, vec)
    }

    fn _get_hid_descriptor(
        host: &Arc<dyn UsbHostInterface>,
        if_no: UsbInterfaceNumber,
        report_type: UsbDescriptorType,
        report_id: u8,
        len: usize,
        vec: &mut Vec<u8>,
    ) -> Result<(), UsbError> {
        let setup = UsbControlSetupData {
            bmRequestType: UsbControlRequestBitmap::GET_INTERFACE,
            bRequest: UsbControlRequest::GET_DESCRIPTOR,
            wValue: (report_type as u16) << 8 | report_id as u16,
            wIndex: if_no.0 as u16,
            wLength: len as u16,
        };
        Self::_control_vec(host, setup, vec)
    }

    #[must_use]
    pub async fn enter_configuration<'a>(&'a self) -> Result<ConfigurationGuard<'a>, UsbError> {
        unsafe { self.host().enter_configuration() }
            .await
            .map(|_| ConfigurationGuard { device: self })
    }
}

pub struct ConfigurationGuard<'a> {
    device: &'a UsbDevice,
}

impl Drop for ConfigurationGuard<'_> {
    fn drop(&mut self) {
        let _ = unsafe { self.device.host().leave_configuration() };
    }
}

/// USB Binary Device Object Store instance type
pub struct UsbBinaryObjectStore {
    raw: Vec<u8>,
    ss_dev_cap: Option<UsbSsDeviceCapability>,
    container_id: Option<UsbContainerIdCapability>,
}

impl UsbBinaryObjectStore {
    #[inline]
    const fn empty() -> Self {
        Self {
            raw: Vec::new(),
            ss_dev_cap: None,
            container_id: None,
        }
    }

    #[inline]
    pub fn raw(&self) -> &[u8] {
        self.raw.as_slice()
    }

    #[inline]
    pub fn ss_dev_cap(&self) -> Option<&UsbSsDeviceCapability> {
        self.ss_dev_cap.as_ref()
    }

    #[inline]
    pub fn container_id(&self) -> Option<&UsbContainerIdCapability> {
        self.container_id.as_ref()
    }
}

/// USB configuration instance type
pub struct UsbConfiguration {
    descriptor: UsbConfigurationDescriptor,
    configuration_value: UsbConfigurationValue,
    interfaces: Vec<UsbInterface>,
    name: Option<String>,
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
    pub fn name(&self) -> Option<&String> {
        self.name.as_ref()
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
    endpoints: Vec<UsbEndpoint>,
    hid_reports: BTreeMap<UsbDescriptorType, Vec<u8>>,
    name: Option<String>,
}

impl UsbInterface {
    #[inline]
    pub const fn descriptor(&self) -> &UsbInterfaceDescriptor {
        &self.descriptor
    }

    #[inline]
    pub const fn if_no(&self) -> UsbInterfaceNumber {
        self.descriptor.if_no()
    }

    #[inline]
    pub const fn alternate_setting(&self) -> UsbAlternateSettingNumber {
        self.descriptor.alternate_setting()
    }

    #[inline]
    pub const fn class(&self) -> UsbClass {
        self.descriptor.class()
    }

    #[inline]
    pub fn endpoints(&self) -> &[UsbEndpoint] {
        self.endpoints.as_slice()
    }

    #[inline]
    pub fn hid_reports_by(&self, desc_type: UsbDescriptorType) -> Option<&[u8]> {
        self.hid_reports.get(&desc_type).map(|v| v.as_slice())
    }

    #[inline]
    pub fn name(&self) -> Option<&String> {
        self.name.as_ref()
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
