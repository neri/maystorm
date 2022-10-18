use super::*;
use crate::{
    r,
    sync::{fifo::AsyncEventQueue, RwLock},
    task::{scheduler::*, Task},
    *,
};
use alloc::{boxed::Box, collections::BTreeMap, string::String, sync::Arc, vec::Vec};
use core::{
    cell::UnsafeCell,
    mem::{size_of, MaybeUninit},
    num::NonZeroU8,
    ops::Deref,
    pin::Pin,
    ptr::null_mut,
    slice,
    sync::atomic::*,
    time::Duration,
};
use futures_util::Future;
use megstd::uuid::Uuid;

/// USB Driver to Host interface
pub trait UsbHostInterface {
    fn parent_device_address(&self) -> Option<UsbAddress>;

    fn route_string(&self) -> UsbRouteString;

    fn speed(&self) -> PSIV;

    fn set_max_packet_size(&self, max_packet_size: usize) -> Result<(), UsbError>;

    fn configure_endpoint(&self, desc: &UsbEndpointDescriptor) -> Result<(), UsbError>;

    fn configure_hub2(&self, hub_desc: &Usb2HubDescriptor, is_mtt: bool) -> Result<(), UsbError>;

    fn configure_hub3(&self, hub_desc: &Usb3HubDescriptor) -> Result<(), UsbError>;

    fn focus_hub(&self) -> Result<(), UsbError>;

    fn unfocus_hub(&self) -> Result<(), UsbError>;

    fn attach_child_device(
        self: Arc<Self>,
        port_id: UsbHubPortNumber,
        speed: PSIV,
    ) -> Pin<Box<dyn Future<Output = Result<UsbAddress, UsbError>>>>;

    /// Performs a control transfer
    unsafe fn control_recv(
        self: Arc<Self>,
        setup: UsbControlSetupData,
        data: *mut u8,
    ) -> Pin<Box<dyn Future<Output = Result<usize, UsbError>>>>;

    unsafe fn control_send(
        self: Arc<Self>,
        setup: UsbControlSetupData,
        data: *const u8,
    ) -> Pin<Box<dyn Future<Output = Result<usize, UsbError>>>>;

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
    fn instantiate(
        &self,
        device: &Arc<UsbDeviceControl>,
    ) -> Option<Pin<Box<dyn Future<Output = Result<Task, UsbError>>>>>;
}

pub trait UsbInterfaceDriverStarter {
    fn instantiate(
        &self,
        device: &Arc<UsbDeviceControl>,
        if_no: UsbInterfaceNumber,
        class: UsbClass,
    ) -> Option<Pin<Box<dyn Future<Output = Result<Task, UsbError>>>>>;
}

static mut USB_MANAGER: MaybeUninit<UsbManager> = MaybeUninit::uninit();

pub struct UsbManager {
    devices: RwLock<Vec<Arc<UsbDeviceControl>>>,
    specific_driver_starters: RwLock<Vec<Box<dyn UsbClassDriverStarter>>>,
    class_driver_starters: RwLock<Vec<Box<dyn UsbClassDriverStarter>>>,
    interface_driver_starters: RwLock<Vec<Box<dyn UsbInterfaceDriverStarter>>>,
    request_queue: AsyncEventQueue<Task>,
}

impl UsbManager {
    /// USB notification does not appear for a certain period of time after startup
    const NOTIFICATION_BLOCK_TIME: Duration = Duration::from_millis(3000);

    pub unsafe fn init() {
        USB_MANAGER.write(Self {
            devices: RwLock::new(Vec::new()),
            specific_driver_starters: RwLock::new(Vec::new()),
            class_driver_starters: RwLock::new(Vec::new()),
            interface_driver_starters: RwLock::new(Vec::new()),
            request_queue: AsyncEventQueue::new(255),
        });

        SpawnOption::with_priority(Priority::High)
            .spawn(Self::_usb_xfer_task_thread, "USB xfer task");

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
        unsafe { &*USB_MANAGER.as_ptr() }
    }

    /// Initialize the USB device and tie it to the appropriate class driver.
    pub async fn instantiate(
        addr: UsbAddress,
        ctx: Arc<dyn UsbHostInterface>,
    ) -> Result<(), UsbError> {
        match UsbDeviceControl::new(addr, ctx).await {
            Ok(device) => {
                let shared = Self::shared();
                let device = Arc::new(device);

                if false {
                    let device = device.device();
                    // let uuid = device.uuid();
                    log!(
                        "USB {:03}:{:03} connected {} {} {} {:?}",
                        device.parent().map(|v| v.as_u8()).unwrap_or(0),
                        addr.as_u8(),
                        device.vid(),
                        device.pid(),
                        device.class(),
                        device.product_string(),
                    );
                }

                shared.devices.write().unwrap().push(device.clone());
                let mut tasks = Vec::new();

                let mut is_configured = false;
                for driver in shared.specific_driver_starters.read().unwrap().iter() {
                    let instantiator = match driver.instantiate(&device) {
                        Some(v) => v,
                        None => continue,
                    };
                    match instantiator.await {
                        Ok(task) => {
                            tasks.push(task);
                            is_configured = true;
                            break;
                        }
                        Err(UsbError::Unsupported) => (),
                        Err(_) => {
                            //
                        }
                    }
                }
                if !is_configured {
                    for driver in shared.class_driver_starters.read().unwrap().iter() {
                        let instantiator = match driver.instantiate(&device) {
                            Some(v) => v,
                            None => continue,
                        };
                        match instantiator.await {
                            Ok(task) => {
                                tasks.push(task);
                                is_configured = true;
                                break;
                            }
                            Err(UsbError::Unsupported) => (),
                            Err(_) => {
                                //
                            }
                        }
                    }
                }
                if !is_configured {
                    for interface in device.device().current_configuration().interfaces() {
                        for driver in shared.interface_driver_starters.read().unwrap().iter() {
                            let instantiator = match driver.instantiate(
                                &device,
                                interface.if_no(),
                                interface.class(),
                            ) {
                                Some(v) => v,
                                None => continue,
                            };
                            match instantiator.await {
                                Ok(task) => {
                                    tasks.push(task);
                                    is_configured = true;
                                    break;
                                }
                                Err(UsbError::Unsupported) => (),
                                Err(_) => {
                                    //
                                }
                            }
                        }
                    }
                }

                if is_configured {
                    device.device().is_configured.store(true, Ordering::SeqCst);
                    tasks
                        .into_iter()
                        .for_each(|task| UsbManager::register_xfer_task(task));

                    if Timer::monotonic() > Self::NOTIFICATION_BLOCK_TIME {
                        if let Some(device_name) = device.device().preferred_device_name() {
                            notify!(r::Icons::Usb, "\"{}\"\nhas been configured.", device_name);
                        } else {
                            notify!(r::Icons::Usb, "A USB Device has been configured.");
                        }
                    }
                } else {
                    if Timer::monotonic() > Self::NOTIFICATION_BLOCK_TIME {
                        if let Some(device_name) = device.device().preferred_device_name() {
                            notify!(r::Icons::Usb, "\"{}\" was found.", device_name);
                        } else {
                            notify!(r::Icons::Usb, "A USB Device was found.");
                        }
                    }
                }

                Ok(())
            }
            Err(err) => {
                log!("USB Device Initialize Error {:?}", err);
                Err(err)
            }
        }
    }

    pub fn detach_device(addr: UsbAddress) {
        let shared = Self::shared();
        let mut vec = shared.devices.write().unwrap();
        let index = match vec.iter().position(|v| v.device().addr() == addr) {
            Some(v) => v,
            None => return,
        };
        let _device = vec.remove(index);
        drop(vec);

        log!("USB disconnected: {}", addr.as_u8());
    }

    pub fn devices() -> impl Iterator<Item = UsbDeviceIterResult> {
        UsbDeviceIter { index: 0 }
    }

    pub fn device_by_addr<'a>(addr: UsbAddress) -> Option<UsbDeviceIterResult> {
        Self::devices().find(|v| v.addr() == addr)
    }

    /// Register a task for USB transfer.
    pub fn register_xfer_task(task: Task) {
        let shared = Self::shared();
        let _ = shared.request_queue.post(task);
    }

    fn _usb_xfer_task_thread() {
        Scheduler::spawn_async(Self::_usb_xfer_observer());
        Scheduler::perform_tasks();
    }

    async fn _usb_xfer_observer() {
        let shared = Self::shared();
        while let Some(new_task) = shared.request_queue.wait_event().await {
            Scheduler::spawn_task(new_task);
        }
    }
}

pub struct UsbDeviceIter {
    index: usize,
}

impl Iterator for UsbDeviceIter {
    type Item = UsbDeviceIterResult;

    fn next(&mut self) -> Option<Self::Item> {
        let devices = UsbManager::shared().devices.read().unwrap();
        if self.index < devices.len() {
            match devices.get(self.index) {
                Some(device) => {
                    self.index += 1;
                    Some(UsbDeviceIterResult {
                        device: device.clone(),
                    })
                }
                None => unreachable!(),
            }
        } else {
            None
        }
    }
}

pub struct UsbDeviceIterResult {
    device: Arc<UsbDeviceControl>,
}

impl Deref for UsbDeviceIterResult {
    type Target = UsbDevice;

    fn deref(&self) -> &Self::Target {
        self.device.device()
    }
}

pub struct UsbDeviceControl {
    host: Arc<dyn UsbHostInterface>,
    device: UnsafeCell<UsbDevice>,
}

impl UsbDeviceControl {
    #[inline]
    async fn new(addr: UsbAddress, host: Arc<dyn UsbHostInterface>) -> Result<Self, UsbError> {
        let mut device_desc: Option<UsbDeviceDescriptor> = None;
        for _ in 0..5 {
            if host.speed() == PSIV::FS {
                // FullSpeed devices have to read the first 8 bytes of the device descriptor first and re-set the maximum packet size.
                let mut packet = [0; 8];
                match Self::_control_slice(
                    &host,
                    UsbControlSetupData::get_descriptor(
                        UsbControlRequestBitmap::GET_DEVICE,
                        UsbDescriptorType::Device,
                        0,
                        8,
                    ),
                    &mut packet,
                )
                .await
                {
                    Ok(_) => {
                        let max_packet_size = packet[7] as usize;
                        let _ = host.set_max_packet_size(max_packet_size);
                    }
                    Err(_) => (),
                }
                Timer::sleep(Duration::from_millis(10));
            }
            match Self::_get_device_descriptor(&host, 0).await {
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
                log!("DEVICE DESCRIPTOR ERROR {}", addr.as_u8());
                return Err(UsbError::InvalidDescriptor);
            }
        };

        let lang_id = match Self::_get_device_descriptor::<UsbStringDescriptor>(&host, 0).await {
            Ok(v) => v.lang_id(),
            Err(_) => UsbLangId(0),
        };

        let manufacturer_string =
            Self::_get_string(&host, device_desc.manufacturer_index(), lang_id).await;
        let product_string = Self::_get_string(&host, device_desc.product_index(), lang_id).await;
        let serial_number =
            Self::_get_string(&host, device_desc.serial_number_index(), lang_id).await;

        // Binary Device Object Storage
        let mut bos = UsbBinaryObjectStore::empty();
        let mut uuid = [0u8; 16];
        if let Some(bos_blob) = if device_desc.usb_version() >= UsbVersion::BOS_MIN {
            match Self::_get_variable_length_device_descriptor(
                &host,
                0,
                |v: UsbBinaryObjectStoreDescriptor| v.total_length(),
            )
            .await
            {
                Ok(v) => Some(v),
                Err(err) => {
                    log!("BOS DESCRIPTOR FAILED {}", addr.as_u8());
                    return Err(err);
                }
            }
        } else {
            None
        } {
            let mut cursor = 0;
            while cursor < bos_blob.len() {
                let len = bos_blob[cursor] as usize;
                let cap_type: UsbDeviceCapabilityType = match (bos_blob[cursor + 1]
                    == UsbDescriptorType::DeviceCapability as u8)
                    .then(|| UsbDeviceCapabilityType::from_u8(bos_blob[cursor + 2]))
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
                            &*(&bos_blob[cursor] as *const _ as *const UsbSsDeviceCapability)
                        };
                        bos.ss_dev_cap = Some(*descriptor);
                    }
                    UsbDeviceCapabilityType::ContainerId => {
                        let descriptor = unsafe {
                            &*(&bos_blob[cursor] as *const _ as *const UsbContainerIdCapability)
                        };
                        uuid.copy_from_slice(descriptor.uuid());
                        bos.container_id = Some(*descriptor);
                    }
                    _ => {
                        // log!(
                        //     "USB {} UNKNOWN CAPABILITY {} {:02x}",
                        //     addr.as_u8(),
                        //     bos[cursor],
                        //     bos[cursor + 2],
                        // );
                    }
                }
                cursor += len;
            }

            bos.raw = bos_blob;
        }

        let config = match Self::_get_variable_length_device_descriptor(
            &host,
            0,
            |v: UsbConfigurationDescriptor| v.total_length(),
        )
        .await
        {
            Ok(v) => v,
            Err(err) => {
                log!("CONFIG DESCRIPTOR FAILED {}", addr.as_u8());
                return Err(err);
            }
        };

        let mut cursor = 0;
        let mut configurations = Vec::new();
        let mut current_configuration = None;
        let mut current_interface = None;
        while cursor < config.len() {
            let len = config[cursor] as usize;
            let desc_type_raw = config[cursor + 1];

            let desc_type: UsbDescriptorType = UsbDescriptorType::from_u8(desc_type_raw);

            match desc_type {
                UsbDescriptorType::Configuration => {
                    let descriptor = unsafe {
                        &*(&config[cursor] as *const _ as *const UsbConfigurationDescriptor)
                    };
                    if let Some(current_configuration) = current_configuration {
                        configurations.push(current_configuration);
                    }
                    let name =
                        Self::_get_string(&host, descriptor.configuration_index(), lang_id).await;
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
                            log!("BAD CONFIG Descriptor {:?}", addr);
                            return Err(UsbError::InvalidDescriptor);
                        }
                    };
                    if let Some(current_interface) = current_interface {
                        current_configuration.interfaces.push(current_interface);
                    }
                    let name =
                        Self::_get_string(&host, descriptor.interface_index(), lang_id).await;
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
                    let ep_type = descriptor.ep_type();
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
                        let mut vec = Vec::with_capacity(len);
                        vec.resize(len, 0);
                        match Self::_get_hid_descriptor(
                            &host,
                            current_interface.if_no(),
                            report_type,
                            0,
                            len,
                            &mut vec,
                        )
                        .await
                        {
                            Ok(_) => (),
                            Err(_err) => {
                                // log!("ERR HID {:02x} {:?}", report_type as usize, _err);
                                // return Err(UsbError::InvalidDescriptor);
                            }
                        };
                        current_interface
                            .hid_reports
                            .insert(report_type, vec.into_boxed_slice());
                    }
                }
                _ => {
                    // log!(
                    //     "USB {} UNKNOWN DESCRIPTOR {} {:02x}",
                    //     addr.as_u8(),
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
        Self::_set_configuration(&host, current_configuration.configuration_value()).await?;

        // if let Some(ss_dev_cap) = ss_dev_cap.as_ref() {
        //     if host.speed() == PSIV::SS {
        //         Self::set_sel(&host, &Usb3ExitLatencyValues::from_ss_dev_cap(ss_dev_cap)).unwrap();
        //     }
        // }

        let parent = host.parent_device_address();

        let device = UsbDevice {
            addr,
            route_string: host.route_string(),
            uuid: Uuid::from_raw(uuid),
            parent,
            children: Vec::new(),
            speed: host.speed(),
            lang_id,
            descriptor: device_desc,
            is_configured: AtomicBool::new(false),
            manufacturer_string,
            product_string,
            serial_number,
            bos,
            config_blob: config,
            current_configuration: current_configuration.configuration_value(),
            configurations,
        };

        Ok(Self {
            host,
            device: UnsafeCell::new(device),
        })
    }

    fn host(&self) -> &dyn UsbHostInterface {
        self.host.as_ref()
    }

    fn host_clone(&self) -> Arc<dyn UsbHostInterface> {
        self.host.clone()
    }

    #[inline]
    pub fn device(&self) -> &UsbDevice {
        unsafe { &*self.device.get() }
    }

    #[inline]
    pub fn device_mut(&self) -> &mut UsbDevice {
        unsafe { &mut *self.device.get() }
    }

    #[inline]
    pub fn configure_endpoint(&self, desc: &UsbEndpointDescriptor) -> Result<(), UsbError> {
        self.host().configure_endpoint(desc)
    }

    #[inline]
    pub fn configure_hub2(
        &self,
        hub_desc: &Usb2HubDescriptor,
        is_mtt: bool,
    ) -> Result<(), UsbError> {
        self.host().configure_hub2(hub_desc, is_mtt)
    }

    #[inline]
    pub fn configure_hub3(&self, hub_desc: &Usb3HubDescriptor) -> Result<(), UsbError> {
        self.host().configure_hub3(hub_desc)
    }

    #[inline]
    pub async fn attach_child_device(
        &self,
        port_id: UsbHubPortNumber,
        speed: PSIV,
    ) -> Result<UsbAddress, UsbError> {
        self.host_clone()
            .attach_child_device(port_id, speed)
            .await
            .map(|addr| {
                self.device_mut().children.push(addr);
                addr
            })
    }

    #[inline]
    pub async fn detach_child_device(&self, child: UsbAddress) {
        let index = match self.device().children.iter().position(|v| *v == child) {
            Some(v) => v,
            None => todo!(),
        };
        self.device_mut().children.remove(index);

        UsbManager::detach_device(child);
    }

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
        if max_len > buffer.len() || min_len > max_len {
            return Err(UsbError::InvalidParameter);
        }
        unsafe { self.host_clone().read(ep, buffer.as_mut_ptr(), max_len) }
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
        let len = buffer.len();
        unsafe { self.host_clone().write(ep, buffer.as_ptr(), len) }
            .await
            .and_then(|result| {
                if result == len {
                    Ok(())
                } else {
                    Err(UsbError::ShortPacket)
                }
            })
    }

    pub async fn control_nodata(&self, setup: UsbControlSetupData) -> Result<(), UsbError> {
        Self::_control_nodata(&self.host_clone(), setup).await
    }

    pub async fn control_vec(
        &self,
        setup: UsbControlSetupData,
        vec: &mut Vec<u8>,
        min_len: usize,
        max_len: usize,
    ) -> Result<(), UsbError> {
        Self::_control_vec(&self.host_clone(), setup, vec, min_len, max_len).await
    }

    pub async fn control_slice(
        &self,
        setup: UsbControlSetupData,
        data: &mut [u8],
    ) -> Result<(), UsbError> {
        Self::_control_slice(&self.host_clone(), setup, data).await
    }

    pub async fn control_send(
        &self,
        mut setup: UsbControlSetupData,
        max_len: usize,
        data: &[u8],
    ) -> Result<(), UsbError> {
        if max_len > data.len() {
            return Err(UsbError::InvalidParameter);
        }
        setup.wLength = max_len as u16;
        match unsafe { self.host_clone().control_send(setup, data.as_ptr()) }.await {
            Ok(_v) => Ok(()),
            Err(err) => Err(err),
        }
    }

    async fn _control_nodata(
        host: &Arc<dyn UsbHostInterface>,
        setup: UsbControlSetupData,
    ) -> Result<(), UsbError> {
        if setup.wLength > 0 {
            return Err(UsbError::InvalidParameter);
        }
        unsafe { host.clone().control_recv(setup, null_mut()) }
            .await
            .map(|_| ())
    }

    async fn _control_vec(
        host: &Arc<dyn UsbHostInterface>,
        mut setup: UsbControlSetupData,
        vec: &mut Vec<u8>,
        min_len: usize,
        max_len: usize,
    ) -> Result<(), UsbError> {
        vec.resize(max_len, 0);
        setup.wLength = max_len as u16;
        unsafe { host.clone().control_recv(setup, vec.as_mut_ptr()) }
            .await
            .and_then(|len| {
                if len >= min_len {
                    vec.resize(len, 0);
                    Ok(())
                } else {
                    Err(UsbError::ShortPacket)
                }
            })
    }

    async fn _control_slice(
        host: &Arc<dyn UsbHostInterface>,
        mut setup: UsbControlSetupData,
        data: &mut [u8],
    ) -> Result<(), UsbError> {
        setup.wLength = data.len() as u16;
        unsafe { host.clone().control_recv(setup, data.as_mut_ptr()) }
            .await
            .and_then(|len| {
                if len == data.len() {
                    Ok(())
                } else {
                    Err(UsbError::ShortPacket)
                }
            })
    }

    #[inline]
    async fn _get_variable_length_device_descriptor<F, T: UsbDescriptor>(
        host: &Arc<dyn UsbHostInterface>,
        index: u8,
        get_length: F,
    ) -> Result<Vec<u8>, UsbError>
    where
        F: FnOnce(T) -> usize,
    {
        let prot_desc: T = Self::_get_device_descriptor(host, index).await?;
        let total_length = get_length(prot_desc);

        let mut vec = Vec::with_capacity(total_length);
        vec.resize(total_length, 0);

        Self::_control_slice(
            host,
            UsbControlSetupData::get_descriptor(
                UsbControlRequestBitmap::GET_DEVICE,
                T::DESCRIPTOR_TYPE,
                index,
                total_length,
            ),
            vec.as_mut_slice(),
        )
        .await?;

        Ok(vec)
    }

    #[inline]
    async fn _get_device_descriptor<T: UsbDescriptor>(
        host: &Arc<dyn UsbHostInterface>,
        index: u8,
    ) -> Result<T, UsbError> {
        Self::_get_descriptor(host, UsbControlRequestBitmap::GET_DEVICE, index).await
    }

    /// Get the descriptor associated with a device
    pub async fn get_descriptor<T: UsbDescriptor>(
        &self,
        request_type: UsbControlRequestBitmap,
        index: u8,
    ) -> Result<T, UsbError> {
        Self::_get_descriptor(&self.host_clone(), request_type, index).await
    }

    async fn _get_descriptor<T: UsbDescriptor>(
        host: &Arc<dyn UsbHostInterface>,
        request_type: UsbControlRequestBitmap,
        index: u8,
    ) -> Result<T, UsbError> {
        let size_of_t = size_of::<T>();
        let mut result = MaybeUninit::<T>::zeroed();
        match unsafe {
            host.clone()
                .control_recv(
                    UsbControlSetupData::get_descriptor(
                        request_type,
                        T::DESCRIPTOR_TYPE,
                        index,
                        size_of_t,
                    ),
                    result.as_mut_ptr() as *mut u8,
                )
                .await
        } {
            Ok(len) => {
                if len != size_of_t {
                    return Err(UsbError::InvalidDescriptor);
                }
                let result = unsafe { result.assume_init() };
                if result.descriptor_type() == T::DESCRIPTOR_TYPE {
                    Ok(result)
                } else {
                    Err(UsbError::InvalidDescriptor)
                }
            }
            Err(err) => Err(err),
        }
    }

    /// Get string descriptor
    pub async fn get_string(&self, index: Option<NonZeroU8>, lang_id: UsbLangId) -> Option<String> {
        Self::_get_string(&self.host_clone(), index, lang_id).await
    }

    async fn _get_string(
        host: &Arc<dyn UsbHostInterface>,
        index: Option<NonZeroU8>,
        lang_id: UsbLangId,
    ) -> Option<String> {
        let index = match index {
            Some(v) => v.get(),
            None => return None,
        };
        let setup = UsbControlSetupData::get_descriptor(
            UsbControlRequestBitmap::GET_DEVICE,
            UsbDescriptorType::String,
            index,
            0,
        )
        .index(lang_id.0);

        let mut vec = Vec::new();
        match Self::_control_vec(host, setup, &mut vec, 4, 255).await {
            Ok(_) => (),
            Err(_) => return None,
        }
        if vec[1] != UsbDescriptorType::String as u8 {
            return None;
        }

        let len = vec[0] as usize / 2 - 1;
        let vec = unsafe { slice::from_raw_parts(&vec[2] as *const _ as *const u16, len) };
        String::from_utf16(vec).ok()
    }

    /// set configuration
    pub async fn set_configuration(&self, value: UsbConfigurationValue) -> Result<(), UsbError> {
        Self::_set_configuration(&self.host_clone(), value).await
    }

    async fn _set_configuration(
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
        .await
    }

    #[inline]
    pub async fn clear_device_feature(
        &self,
        feature_sel: UsbDeviceFeatureSel,
    ) -> Result<(), UsbError> {
        Self::_control_nodata(
            &self.host_clone(),
            UsbControlSetupData::request(
                UsbControlRequestBitmap::SET_DEVICE,
                UsbControlRequest::CLEAR_FEATURE,
            )
            .value(feature_sel as u16),
        )
        .await
    }

    #[inline]
    pub async fn set_device_feature(
        &self,
        feature_sel: UsbDeviceFeatureSel,
    ) -> Result<(), UsbError> {
        Self::_control_nodata(
            &self.host_clone(),
            UsbControlSetupData::request(
                UsbControlRequestBitmap::SET_DEVICE,
                UsbControlRequest::SET_FEATURE,
            )
            .value(feature_sel as u16),
        )
        .await
    }

    /// Set exit latency values (USB3)
    pub async fn set_sel(&self, values: &Usb3ExitLatencyValues) -> Result<(), UsbError> {
        Self::_set_sel(&self.host_clone(), values).await
    }

    async fn _set_sel(
        host: &Arc<dyn UsbHostInterface>,
        values: &Usb3ExitLatencyValues,
    ) -> Result<(), UsbError> {
        let length = 6;
        let data = values as *const _ as *const u8;
        unsafe {
            host.clone().control_send(
                UsbControlSetupData::request(
                    UsbControlRequestBitmap::SET_DEVICE,
                    UsbControlRequest::SET_SEL,
                )
                .length(length),
                data,
            )
        }
        .await
        .map(|_| ())
    }

    async fn _get_hid_descriptor(
        host: &Arc<dyn UsbHostInterface>,
        if_no: UsbInterfaceNumber,
        report_type: UsbDescriptorType,
        report_id: u8,
        len: usize,
        slice: &mut [u8],
    ) -> Result<(), UsbError> {
        Self::_control_slice(
            host,
            UsbControlSetupData::get_descriptor(
                UsbControlRequestBitmap::GET_INTERFACE,
                report_type,
                report_id,
                len,
            )
            .index_if(if_no),
            slice,
        )
        .await
    }

    /// Focuses on packets from the specified hub and delays tasks on other devices.
    #[must_use]
    pub fn focus_device(&self) -> UsbDeviceFocusedScope {
        self.host().focus_hub().unwrap();
        UsbDeviceFocusedScope(self)
    }
}

#[repr(transparent)]
pub struct UsbDeviceFocusedScope<'a>(&'a UsbDeviceControl);

impl Drop for UsbDeviceFocusedScope<'_> {
    #[inline]
    fn drop(&mut self) {
        let _ = self.0.host().unfocus_hub();
    }
}

/// USB device instance type
pub struct UsbDevice {
    addr: UsbAddress,
    route_string: UsbRouteString,
    parent: Option<UsbAddress>,
    children: Vec<UsbAddress>,
    speed: PSIV,

    uuid: Uuid,

    is_configured: AtomicBool,
    manufacturer_string: Option<String>,
    product_string: Option<String>,
    serial_number: Option<String>,
    descriptor: UsbDeviceDescriptor,
    lang_id: UsbLangId,

    config_blob: Vec<u8>,

    bos: UsbBinaryObjectStore,
    current_configuration: UsbConfigurationValue,
    configurations: Vec<UsbConfiguration>,
}

impl UsbDevice {
    /// Gets the USB address of this device.
    #[inline]
    pub fn addr(&self) -> UsbAddress {
        self.addr
    }

    /// Gets the USB address of the parent device.
    #[inline]
    pub fn parent(&self) -> Option<UsbAddress> {
        self.parent
    }

    #[inline]
    pub fn route_string(&self) -> UsbRouteString {
        self.route_string
    }

    #[inline]
    pub fn children(&self) -> &[UsbAddress] {
        self.children.as_slice()
    }

    /// Gets the uuid of this device, if available.
    #[inline]
    pub fn uuid(&self) -> &Uuid {
        &self.uuid
    }

    #[inline]
    pub fn protocol_speed(&self) -> usize {
        self.speed.protocol_speed()
    }

    #[inline]
    pub fn usb_version(&self) -> UsbVersion {
        self.descriptor.usb_version()
    }

    /// Gets the vendor ID for this device.
    #[inline]
    pub fn vid(&self) -> UsbVendorId {
        self.descriptor.vid()
    }

    /// Gets the product ID for this device.
    #[inline]
    pub fn pid(&self) -> UsbProductId {
        self.descriptor.pid()
    }

    /// Gets the device class of this device.
    #[inline]
    pub fn class(&self) -> UsbClass {
        self.descriptor.class()
    }

    #[inline]
    pub fn preferred_lang_id(&self) -> UsbLangId {
        self.lang_id
    }

    #[inline]
    pub fn is_configured(&self) -> bool {
        self.is_configured.load(Ordering::Relaxed)
    }

    /// Gets the manufacturer's string for this device if possible.
    #[inline]
    pub fn manufacturer_string(&self) -> Option<&str> {
        self.manufacturer_string.as_ref().map(|v| v.as_str())
    }

    /// Gets the preferred name of the device. First, use the product name if available. Next, use the class name if available.
    pub fn preferred_device_name(&self) -> Option<&str> {
        if let Some(v) = self.product_string.as_ref() {
            return Some(v);
        }
        if self.class() == UsbClass::COMPOSITE
            && self.current_configuration().interfaces().len() == 1
        {
            if let Some(v) = self
                .current_configuration()
                .interfaces()
                .first()
                .unwrap()
                .class()
                .class_string(true)
            {
                return Some(v);
            }
        } else {
            if let Some(v) = self.class().class_string(false) {
                return Some(v);
            }
        }
        return None;
    }

    /// Gets the product name string for this device, if possible.
    #[inline]
    pub fn product_string(&self) -> Option<&str> {
        self.product_string.as_ref().map(|v| v.as_str())
    }

    /// Gets the serial number string of this device, if possible.
    #[inline]
    pub fn serial_number(&self) -> Option<&str> {
        self.serial_number.as_ref().map(|v| v.as_str())
    }

    /// Gets the device descriptor for this device.
    #[inline]
    pub fn descriptor(&self) -> &UsbDeviceDescriptor {
        &self.descriptor
    }

    /// Gets raw configuration descriptors.
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
            .iter()
            .find(|v| v.configuration_value == self.current_configuration)
            .unwrap()
    }

    #[inline]
    pub fn configurations(&self) -> &[UsbConfiguration] {
        self.configurations.as_slice()
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
    pub fn name(&self) -> Option<&str> {
        self.name.as_ref().map(|v| v.as_str())
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
    hid_reports: BTreeMap<UsbDescriptorType, Box<[u8]>>,
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
        self.hid_reports.get(&desc_type).map(|v| v.as_ref())
    }

    #[inline]
    pub fn name(&self) -> Option<&str> {
        self.name.as_ref().map(|v| v.as_str())
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
    pub const fn is_dir_in(&self) -> bool {
        self.address().is_dir_in()
    }

    #[inline]
    pub const fn ep_type(&self) -> UsbEndpointType {
        self.ep_type
    }
}
