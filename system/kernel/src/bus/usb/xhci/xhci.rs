//! xHCI: Extensible Host Controller Interface

use super::*;
use crate::{
    arch::cpu::Cpu,
    bus::{pci::*, usb::*},
    mem::mmio::*,
    mem::{MemoryManager, NonNullPhysicalAddress, PhysicalAddress},
    sync::{fifo::AsyncEventQueue, semaphore::*, RwLock},
    task::{scheduler::*, Task},
    *,
};
use alloc::{
    boxed::Box,
    format,
    string::String,
    sync::{Arc, Weak},
    vec::Vec,
};
use core::{
    cell::UnsafeCell,
    ffi::c_void,
    fmt::Write,
    marker::PhantomData,
    mem::transmute,
    mem::{size_of, MaybeUninit},
    num::{NonZeroU64, NonZeroU8},
    pin::Pin,
    slice,
    sync::atomic::*,
    task::Poll,
    time::Duration,
};
use futures_util::Future;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

pub struct XhciRegistrar {}

impl XhciRegistrar {
    const PREFERRED_CLASS: PciClass = PciClass::code(0x0C).sub(0x03).interface(0x30);

    pub fn init() -> Box<dyn PciDriverRegistrar> {
        Box::new(Self {}) as Box<dyn PciDriverRegistrar>
    }
}

impl PciDriverRegistrar for XhciRegistrar {
    fn instantiate(&self, device: &PciDevice) -> Option<Arc<dyn PciDriver>> {
        if device.class_code().matches(Self::PREFERRED_CLASS) {
            unsafe { Xhci::new(device) }
        } else {
            None
        }
    }
}

/// Extensible Host Controller Interface
///
/// Many methods are made public for documentation purposes, but are not intended to be called from the outside.
pub struct Xhci {
    addr: PciConfigAddress,
    #[allow(dead_code)]
    mmio: MmioSlice,

    cap: &'static CapabilityRegisters,
    opr: &'static OperationalRegisters,
    ports: &'static [PortRegisters],
    doorbells: &'static [DoorbellRegister],
    rts: &'static RuntimeRegisters,

    max_device_slots: usize,
    dcbaa_len: usize,
    context_size: usize,
    ers: PhysicalAddress,

    ring_context: RwLock<[MaybeUninit<EpRingContext>; Self::MAX_TR]>,
    event_cycle: CycleBit,
    port_status_change_queue: AsyncEventQueue<PortId>,
    port2slot: RwLock<[Option<SlotId>; 256]>,
    slot2port: RwLock<[Option<PortId>; 256]>,
    xrbs: [UnsafeCell<XhciRequestBlock>; Self::MAX_XRB],
    ics: [UnsafeCell<InputContext>; Self::MAX_DEVICE_SLOTS],

    sem_event_thread: Semaphore,

    root_hub_lock: Pin<Arc<AsyncSharedLockTemp>>,
}

unsafe impl Send for Xhci {}
unsafe impl Sync for Xhci {}

impl Xhci {
    const DRIVER_NAME: &'static str = "xhci";

    /// The maximum number of device slots allowed.
    /// This means the maximum number of USB devices that can be connected to this controller.
    const MAX_DEVICE_SLOTS: usize = 64;
    const MAX_TR: usize = 256;
    const MAX_TR_INDEX: usize = 64;
    const MAX_XRB: usize = 1024;

    const MAX_PORT_CHANGE: usize = 64;

    unsafe fn new(device: &PciDevice) -> Option<Arc<dyn PciDriver>> {
        let bar = match device.bars().first() {
            Some(v) => v,
            None => return None,
        };
        let mmio = match MmioSlice::from_bar(*bar) {
            Some(v) => v,
            None => return None,
        };

        let cap: &CapabilityRegisters = mmio.transmute(0);
        let opr = cap.opr();
        let ports = cap.ports();
        let doorbells = cap.doorbells();
        let rts = cap.rts();

        let max_device_slots = usize::min(Self::MAX_DEVICE_SLOTS, cap.max_device_slots());
        let dcbaa_len = 1 + max_device_slots;
        let hcc_params1 = cap.hcc_params1();
        let context_size = if hcc_params1.contains(HccParams1::CSZ) {
            64
        } else {
            32
        };

        let ers =
            MemoryManager::alloc_pages(InterrupterRegisterSet::SIZE_EVENT_RING * size_of::<Trb>())
                .unwrap()
                .get() as PhysicalAddress;

        let driver = Arc::new(Self {
            addr: device.address(),
            mmio,
            cap,
            opr,
            ports,
            doorbells,
            rts,
            max_device_slots,
            dcbaa_len,
            context_size,
            ring_context: RwLock::new(MaybeUninit::uninit_array()),
            event_cycle: CycleBit::from(true),
            ers,
            port_status_change_queue: AsyncEventQueue::new(Self::MAX_PORT_CHANGE),
            port2slot: RwLock::new([None; 256]),
            slot2port: RwLock::new([None; 256]),
            xrbs: [XhciRequestBlock::EMPTY; Self::MAX_XRB],
            ics: [InputContext::EMPTY; Self::MAX_DEVICE_SLOTS],
            sem_event_thread: Semaphore::new(0),
            root_hub_lock: AsyncSharedLockTemp::new(),
        });

        for ctx in driver.ring_context.write().unwrap().iter_mut() {
            *ctx = MaybeUninit::zeroed();
        }

        driver.clone().initialize(device);

        let p = driver.clone();
        SpawnOption::with_priority(Priority::Realtime).spawn(
            move || {
                p._event_thread();
            },
            Self::DRIVER_NAME,
        );

        UsbManager::register_xfer_task(Task::new(driver.clone()._root_hub_task()));

        Some(driver as Arc<dyn PciDriver>)
    }

    ///  xHCI Initialize
    unsafe fn initialize(self: Arc<Self>, pci: &PciDevice) {
        if let Some(xecp) = self.cap.xecp() {
            let mut xecp_base = xecp.get() as *mut u32;
            loop {
                let xecp = xecp_base.read_volatile();
                match xecp & 0xFF {
                    0x01 => {
                        // USB Legacy Support
                        const USBLEGSUP_BIOS_OWNED: u32 = 0x0001_0000;
                        const USBLEGSUP_OS_OWNED: u32 = 0x0100_0000;
                        let usb_leg_sup = xecp_base;
                        let usb_leg_ctl_sts = xecp_base.add(1);

                        // Hand over ownership from BIOS to OS
                        usb_leg_sup.write_volatile(xecp | USBLEGSUP_OS_OWNED);

                        if (usb_leg_sup.read_volatile() & USBLEGSUP_BIOS_OWNED) != 0 {
                            for _ in 0..20 {
                                if (usb_leg_sup.read_volatile() & USBLEGSUP_BIOS_OWNED) == 0 {
                                    break;
                                }
                                Timer::sleep(Duration::from_millis(50));
                            }
                            // Force BIOS ownership to be disabled.
                            usb_leg_sup.write_volatile(
                                usb_leg_sup.read_volatile() & !USBLEGSUP_BIOS_OWNED,
                            );
                        }

                        // Adjusting SMI settings
                        usb_leg_ctl_sts.write_volatile(
                            (usb_leg_ctl_sts.read_volatile() & 0x000E_1FEE) | 0xE000_0000,
                        );
                    }
                    _ => (),
                }
                match ((xecp >> 8) & 0xFF) as usize {
                    0 => break,
                    xecp_ptr => {
                        xecp_base = xecp_base.add(xecp_ptr);
                    }
                }
            }
        }

        // reset xHC
        self.opr.write_cmd(UsbCmd::HCRST);
        // The xHC shall halt within 16ms.
        Timer::sleep(Duration::from_millis(20));
        while self.opr.read_cmd().contains(UsbCmd::HCRST) || self.opr.status().contains(UsbSts::CNR)
        {
            Timer::sleep(Duration::from_millis(10));
        }

        self.opr.set_config(self.max_device_slots, false, false);

        // make Device Context Base Address Array
        let dcbaa_size = self.dcbaa_len * 8;
        let pa_dcbaa = MemoryManager::alloc_pages(dcbaa_size).unwrap().get() as u64;
        self.opr.set_dcbaap(NonZeroU64::new(pa_dcbaa).unwrap());

        // make Scratchpad
        let max_scratchpad_size = self.cap.max_scratchpad_size();
        if max_scratchpad_size > 0 {
            let array_size = max_scratchpad_size * 8;
            let sp_array = MemoryManager::alloc_pages(array_size).unwrap().get() as u64;
            let sp_size = max_scratchpad_size * self.opr.page_size();
            let scratchpad = MemoryManager::alloc_pages(sp_size).unwrap().get() as u64;
            let spava = MemoryManager::direct_map(sp_array) as *mut u64;
            for i in 0..max_scratchpad_size {
                spava
                    .add(i)
                    .write_volatile(scratchpad + (i * self.opr.page_size()) as u64);
            }
            self.dcbaa()[0] = sp_array;
        }

        // Command Ring Control Register
        self.opr.set_crcr(self.alloc_ep_ring(None, None).unwrap());

        // Event Ring Segment Table
        self.rts.irs0().init(self.ers);

        // Interrupt
        self.rts.irs0().set_iman(3);
        self.opr.set_cmd(UsbCmd::INTE);
        let p = Arc::as_ptr(&self);
        Arc::increment_strong_count(p);
        pci.register_msi(Self::_msi_handler, p as usize).unwrap();

        // self.opr
        //     .set_device_notification_bitmap(DeviceNotificationBitmap::FUNCTION_WAKE);

        // start xHC
        self.wait_cnr(0);
        self.opr.set_cmd(UsbCmd::RUN);
        while self.opr.status().contains(UsbSts::HCH) {
            Timer::sleep(Duration::from_millis(10));
        }
    }

    fn dcbaa(&self) -> &'static mut [u64] {
        unsafe {
            slice::from_raw_parts_mut(
                MemoryManager::direct_map(self.opr.dcbaap() & !63) as *mut u64,
                self.dcbaa_len,
            )
        }
    }

    fn _msi_handler(p: usize) {
        let this = unsafe { &*(p as *const Self) };
        this.sem_event_thread.signal();
    }

    /// xHCI Main event loop
    fn _event_thread(self: Arc<Self>) {
        loop {
            self.sem_event_thread.wait();
            self.process_event();
        }
    }

    pub fn get_device_context(&self, slot_id: SlotId) -> u64 {
        *self.dcbaa().get(slot_id.0.get() as usize).unwrap()
    }

    pub fn set_device_context(&self, slot_id: SlotId, value: u64) {
        *self.dcbaa().get_mut(slot_id.0.get() as usize).unwrap() = value;
    }

    pub fn ring_a_doorbell(&self, slot_id: Option<SlotId>, dci: Option<DCI>) {
        self.doorbells
            .get(slot_id.map(|v| v.0.get() as usize).unwrap_or_default())
            .unwrap()
            .set_target(dci);
    }

    #[inline]
    pub fn port_by_slot(&self, slot_id: SlotId) -> Option<PortId> {
        unsafe {
            *self
                .slot2port
                .read()
                .unwrap()
                .get_unchecked(slot_id.0.get() as usize)
        }
    }

    #[inline]
    pub fn slot_by_port(&self, port_id: PortId) -> Option<SlotId> {
        unsafe {
            *self
                .port2slot
                .read()
                .unwrap()
                .get_unchecked(port_id.0.get() as usize)
        }
    }

    #[inline]
    pub fn port_by(&self, port_id: PortId) -> &PortRegisters {
        self.ports.get(port_id.0.get() as usize - 1).unwrap()
    }

    pub fn input_context<'a>(&self, slot_id: SlotId) -> &'a mut InputContext {
        self.ics
            .get(slot_id.0.get() as usize)
            .map(|v| unsafe { &mut *v.get() })
            .unwrap()
    }

    /// wait for CNR (Controller Not Ready)
    #[inline]
    pub fn wait_cnr(&self, _: usize) {
        while self.opr.status().contains(UsbSts::CNR) {
            Cpu::spin_loop_hint();
        }
    }

    pub fn ep_ring_index(&self, slot_id: Option<SlotId>, dci: Option<DCI>) -> Option<usize> {
        let slot_id = slot_id.map(|v| v.0.get()).unwrap_or_default();
        let dci = dci.map(|v| v.0.get()).unwrap_or_default();
        for (index, ctx) in self.ring_context.read().unwrap().iter().enumerate() {
            let ctx = unsafe { &*ctx.as_ptr() };
            if ctx.tr_base().is_some()
                && ctx.slot_id().map(|v| v.0.get()).unwrap_or_default() == slot_id
                && ctx.dci().map(|v| v.0.get()).unwrap_or_default() == dci
            {
                return Some(index);
            }
        }
        None
    }

    pub fn alloc_ep_ring(
        &self,
        slot_id: Option<SlotId>,
        dci: Option<DCI>,
    ) -> Option<NonNullPhysicalAddress> {
        if let Some(index) = self.ep_ring_index(slot_id, dci) {
            let ctx = &mut self.ring_context.write().unwrap()[index];
            let ctx = unsafe { &mut *ctx.as_mut_ptr() };
            ctx.clear();
            return ctx.tr_value();
        }
        for ctx in self.ring_context.write().unwrap().iter_mut() {
            let ctx = unsafe { &mut *ctx.as_mut_ptr() };
            if ctx.tr_base().is_none() {
                ctx.alloc(slot_id, dci);
                return ctx.tr_value();
            }
        }
        None
    }

    pub fn schedule_ep_ring(
        &self,
        slot_id: Option<SlotId>,
        dci: Option<DCI>,
    ) -> Option<Pin<Arc<AsyncSemaphore>>> {
        if let Some(index) = self.ep_ring_index(slot_id, dci) {
            let ctx = &mut self.ring_context.write().unwrap()[index];
            let ctx = unsafe { &mut *ctx.as_mut_ptr() };
            return ctx.asem().map(|v| v.clone());
        }
        return None;
    }

    pub fn allocate_xrb<'a>(&'a self) -> Option<&'a mut XhciRequestBlock> {
        for xrb in &self.xrbs {
            let xrb = unsafe { &mut *xrb.get() };
            if xrb.try_to_acquire() {
                return Some(xrb);
            }
        }
        None
    }

    pub fn find_xrb<'a>(
        &'a self,
        scheduled_trb: ScheduledTrb,
        state: Option<RequestState>,
    ) -> Option<&'a mut XhciRequestBlock> {
        for xrb in &self.xrbs {
            let xrb = unsafe { &mut *xrb.get() };
            let xrb_state = xrb.state();
            if xrb_state != RequestState::Available && xrb.scheduled_trb == scheduled_trb {
                match state {
                    Some(state) => {
                        if xrb_state == state {
                            return Some(xrb);
                        }
                    }
                    None => return Some(xrb),
                }
            }
        }
        None
    }

    pub fn issue_trb<T: TrbCommon>(
        &self,
        xrb: Option<&mut XhciRequestBlock>,
        trb: &T,
        slot_id: Option<SlotId>,
        dci: Option<DCI>,
        need_to_notify: bool,
    ) -> ScheduledTrb {
        let trb = trb.as_common_trb();
        let index = match self.ep_ring_index(slot_id, dci) {
            Some(index) => index,
            None => todo!(),
        };
        let ctx = &mut self.ring_context.write().unwrap()[index];
        let ctx = unsafe { &mut *ctx.as_mut_ptr() };

        let tr_base = ctx.tr_base().unwrap().get();
        let tr = MemoryManager::direct_map(tr_base) as *mut Trb;
        let mut index = ctx.index;

        let scheduled_trb = ScheduledTrb(tr_base + (size_of::<Trb>() * index) as u64);
        if let Some(xrb) = xrb {
            xrb.schedule(trb, scheduled_trb);
        }

        unsafe {
            let p = tr.add(index);
            (&*p).copy_from(trb, ctx.pcs());
        }
        index += 1;
        if index == Xhci::MAX_TR_INDEX - 1 {
            let trb_link = TrbLink::new(tr_base, true);
            unsafe {
                let p = tr.add(index);
                (&*p).copy_from(&trb_link, ctx.pcs());
            }

            index = 0;
            ctx.pcs().toggle();
        }
        ctx.index = index;

        if need_to_notify {
            self.wait_cnr(0);
            self.ring_a_doorbell(slot_id, dci);
        }

        scheduled_trb
    }

    /// Issue trb command
    pub fn execute_command<T: TrbCommon>(&self, trb: &T) -> Result<TrbCce, TrbCce> {
        let xrb = self.allocate_xrb().unwrap();
        self.issue_trb(Some(xrb), trb, None, None, true);
        xrb.wait();
        let result = match xrb.response.as_event() {
            Some(TrbEvent::CommandCompletion(v)) => Some(v.copied()),
            _ => None,
        };
        xrb.dispose();
        match result {
            Some(result) => {
                if result.completion_code() == Some(TrbCompletionCode::SUCCESS) {
                    Ok(result)
                } else {
                    Err(result)
                }
            }
            None => Err(TrbCce::empty()),
        }
    }

    pub async unsafe fn control_async(
        self: Arc<Self>,
        slot_id: SlotId,
        setup: UsbControlSetupData,
        buffer: u64,
    ) -> Result<usize, UsbError> {
        let trt = if setup.wLength > 0 {
            if setup.bmRequestType.is_device_to_host() {
                TrbTranfserType::ControlIn
            } else {
                TrbTranfserType::ControlOut
            }
        } else {
            TrbTranfserType::NoData
        };
        let dir = trt == TrbTranfserType::ControlIn;
        let dci = Some(DCI::CONTROL);
        let slot_id = Some(slot_id);

        let setup_trb = TrbSetupStage::new(trt, setup);
        self.issue_trb(None, &setup_trb, slot_id, dci, false);

        if setup.wLength > 0 {
            let data_trb = TrbDataStage::new(buffer, setup.wLength as usize, dir, false);
            self.issue_trb(None, &data_trb, slot_id, dci, false);
        }

        let ctx = match self.ep_ring_index(slot_id, dci) {
            Some(index) => {
                let ctx = &mut self.ring_context.write().unwrap()[index];
                unsafe { &mut *ctx.as_mut_ptr() }
            }
            None => todo!(),
        };
        ctx.set_scheduled();

        let status_trb = TrbStatusStage::new(!dir);
        self.issue_trb(None, &status_trb, slot_id, dci, true);

        ctx.asem().unwrap().clone().wait().await;

        let result = match ctx.response.as_event() {
            Some(TrbEvent::Transfer(v)) => Some(v.copied()),
            _ => None,
        };
        let state = ctx.state();
        ctx.dispose();
        match state {
            RequestState::Available | RequestState::Acquired | RequestState::Scheduled => {
                unreachable!()
            }
            RequestState::Completed => match result {
                Some(result) => match result.completion_code() {
                    Some(TrbCompletionCode::SUCCESS) => {
                        Ok(setup.wLength as usize - result.transfer_length())
                    }
                    Some(TrbCompletionCode::STALL) => {
                        let _ = self.reset_endpoint(slot_id.unwrap(), dci.unwrap());
                        Err(UsbError::General)
                    }
                    Some(_err) => Err(UsbError::General),
                    None => Err(UsbError::ControllerError),
                },
                None => Err(UsbError::ControllerError),
            },
            RequestState::Aborted => Err(UsbError::Aborted),
        }
    }

    pub async unsafe fn control_async2(
        self: Arc<Self>,
        slot_id: SlotId,
        setup: UsbControlSetupData,
        buffer: u64,
    ) -> Result<(*const u8, usize), UsbError> {
        self.control_async(slot_id, setup, buffer).await.map(|len| {
            (
                MemoryManager::direct_map(buffer as PhysicalAddress) as *const u8,
                len,
            )
        })
    }

    pub async unsafe fn transfer_async(
        self: Arc<Self>,
        slot_id: SlotId,
        dci: DCI,
        buffer: u64,
        len: usize,
    ) -> Result<usize, UsbError> {
        let slot_id = Some(slot_id);
        let dci = Some(dci);

        let trb = TrbNormal::new(buffer, len, true, true);

        let ctx = match self.ep_ring_index(slot_id, dci) {
            Some(index) => {
                let ctx = &mut self.ring_context.write().unwrap()[index];
                unsafe { &mut *ctx.as_mut_ptr() }
            }
            None => todo!(),
        };
        ctx.set_scheduled();

        self.issue_trb(None, &trb, slot_id, dci, true);

        ctx.asem().unwrap().clone().wait().await;

        let result = match ctx.response.as_event() {
            Some(TrbEvent::Transfer(v)) => Some(v.copied()),
            _ => None,
        };
        let state = ctx.state();
        ctx.dispose();
        match state {
            RequestState::Available | RequestState::Acquired | RequestState::Scheduled => {
                unreachable!()
            }
            RequestState::Completed => match result {
                Some(result) => match result.completion_code() {
                    Some(TrbCompletionCode::SUCCESS) | Some(TrbCompletionCode::SHORT_PACKET) => {
                        Ok(len - result.transfer_length())
                    }
                    Some(_err) => Err(UsbError::General),
                    None => Err(UsbError::ControllerError),
                },
                None => Err(UsbError::UnexpectedToken),
            },
            RequestState::Aborted => Err(UsbError::Aborted),
        }
    }

    pub async unsafe fn read_async(
        self: Arc<Self>,
        slot_id: SlotId,
        dci: DCI,
        device_buffer: u64,
        len: usize,
        xfer_buffer: *mut u8,
    ) -> Result<usize, UsbError> {
        self.transfer_async(slot_id, dci, device_buffer, len)
            .await
            .map(|len| {
                unsafe {
                    let p =
                        MemoryManager::direct_map(device_buffer as PhysicalAddress) as *const u8;
                    let q = xfer_buffer;
                    q.copy_from(p, len);
                }
                len
            })
    }

    #[inline]
    pub fn reset_endpoint(&self, slot_id: SlotId, dci: DCI) -> Result<TrbCce, TrbCce> {
        let trb = TrbResetEndpointCommand::new(slot_id, dci);
        self.execute_command(&trb)
    }

    pub fn configure_endpoint(
        &self,
        slot_id: SlotId,
        dci: DCI,
        ep_type: EpType,
        max_packet_size: usize,
        interval: u8,
        copy_dc: bool,
    ) {
        let input_context = self.input_context(slot_id);
        let control = input_context.control();
        let slot = input_context.slot();
        let endpoint = input_context.endpoint(dci);
        let psiv: PSIV = FromPrimitive::from_usize(slot.speed_raw()).unwrap_or(PSIV::SS);

        control.clear();
        control.set_add(1 | (1u32 << dci.0.get()));

        if copy_dc {
            unsafe {
                let slot = slot as *const _ as *mut u8;
                let dc = MemoryManager::direct_map(self.get_device_context(slot_id)) as *const u8;
                slot.copy_from(dc, self.context_size);
            }
        }

        slot.set_context_entries(usize::max(dci.0.get() as usize, slot.context_entries()));
        // let psiv = FromPrimitive::from_usize(slot.speed_raw()).unwrap_or(PSIV::SS);

        endpoint.set_ep_type(ep_type);

        if max_packet_size > 0 {
            endpoint.set_max_packet_size(max_packet_size & 0x07FF);
            endpoint.set_max_burst_size((max_packet_size & 0x1800) >> 11)
        } else {
            endpoint.set_max_packet_size(psiv.max_packet_size());
            endpoint.set_average_trb_len(8);
        }
        if interval > 0 {
            match psiv {
                PSIV::LS | PSIV::FS => {
                    if ep_type.is_interrupt() {
                        endpoint
                            .set_interval(interval.next_power_of_two().trailing_zeros() as u8 + 3);
                    } else {
                        endpoint.set_interval(interval + 2);
                    }
                }
                _ => {
                    endpoint.set_interval(interval - 1);
                }
            }
        }
        if !ep_type.is_isochronous() {
            endpoint.set_error_count(3);
        }

        let tr = self.alloc_ep_ring(Some(slot_id), Some(dci)).unwrap().get();
        endpoint.set_trdp(tr);
    }

    pub fn configure_hub2(
        &self,
        slot_id: SlotId,
        hub_desc: &Usb2HubDescriptor,
        is_mtt: bool,
    ) -> Result<(), UsbError> {
        let input_context = self.input_context(slot_id);
        input_context.control().set_add(1);

        unsafe {
            let slot = input_context.slot() as *const _ as *mut u8;
            let dc = MemoryManager::direct_map(self.get_device_context(slot_id)) as *const u8;
            slot.copy_from(dc, self.context_size * 2);
        }

        let slot = input_context.slot();
        slot.set_is_hub(true);
        slot.set_num_ports(hub_desc.num_ports());
        slot.set_is_mtt(is_mtt);
        slot.set_ttt(hub_desc.characteristics().ttt());

        let trb = TrbEvaluateContextCommand::new(slot_id, input_context.raw_data());
        match self.execute_command(&trb) {
            Ok(_) => Ok(()),
            Err(_) => Err(UsbError::General),
        }
    }

    pub fn configure_hub3(
        &self,
        slot_id: SlotId,
        hub_desc: &Usb3HubDescriptor,
    ) -> Result<(), UsbError> {
        let input_context = self.input_context(slot_id);
        input_context.control().set_add(1);

        unsafe {
            let slot = input_context.slot() as *const _ as *mut u8;
            let dc = MemoryManager::direct_map(self.get_device_context(slot_id)) as *const u8;
            slot.copy_from(dc, self.context_size * 2);
        }

        let slot = input_context.slot();
        slot.set_is_hub(true);
        slot.set_num_ports(hub_desc.num_ports());
        // slot.set_max_exit_latency(max_exit_latency);

        let trb = TrbEvaluateContextCommand::new(slot_id, input_context.raw_data());
        match self.execute_command(&trb) {
            Ok(_) => Ok(()),
            Err(_) => Err(UsbError::General),
        }
    }

    pub async fn attach_child_device(
        self: Arc<Self>,
        hub: Arc<HciContext>,
        port_id: UsbHubPortNumber,
        speed: PSIV,
    ) -> Result<UsbAddress, UsbError> {
        let device = hub.device();

        let new_route = match device.route_string.appending(port_id) {
            Ok(v) => v,
            Err(_) => return Err(UsbError::InvalidParameter),
        };

        let trb = Trb::new(TrbType::ENABLE_SLOT_COMMAND);
        let slot_id = match self.execute_command(&trb) {
            Ok(result) => result.slot_id().unwrap(),
            Err(_err) => {
                // TODO:
                return Err(UsbError::ControllerError);
            }
        };

        let device_context_size = self.context_size * 32;
        let device_context = unsafe { MemoryManager::alloc_pages(device_context_size) }
            .unwrap()
            .get() as u64;
        self.set_device_context(slot_id, device_context);

        let input_context_size = self.context_size * 33;
        let input_context_pa = unsafe { MemoryManager::alloc_pages(input_context_size) }
            .unwrap()
            .get() as u64;
        let input_context = self.input_context(slot_id);
        input_context.init(input_context_pa, self.context_size);

        let slot = input_context.slot();
        slot.set_root_hub_port(device.root_port_id);
        slot.set_context_entries(1);
        slot.set_route_string(new_route);
        slot.set_speed(speed as usize);

        if hub.device().psiv > speed {
            slot.set_parent_hub_slot_id(device.slot_id);
            slot.set_parent_port_id(port_id);
        }

        self.configure_endpoint(slot_id, DCI::CONTROL, EpType::Control, 0, 0, false);

        Timer::sleep(Duration::from_millis(100));

        // log!(
        //     "ATTACH HUB DEVICE: SLOT {} ROOT {} ROUTE {:05x} PSIV {:?}",
        //     slot_id.0.get(),
        //     device.root_port_id.0.get(),
        //     new_route.as_u32(),
        //     speed,
        // );

        let trb = TrbAddressDeviceCommand::new(slot_id, input_context_pa);
        match self.execute_command(&trb) {
            Ok(_) => (),
            Err(err) => {
                log!("ADDRESS_DEVICE ERROR {:?}", err.completion_code());
                return Err(UsbError::UsbTransactionError);
            }
        }

        let buffer = unsafe { MemoryManager::alloc_pages(MemoryManager::PAGE_SIZE_MIN) }
            .unwrap()
            .get() as u64;
        let device = HciDeviceContext {
            root_port_id: device.root_port_id,
            port_id: PortId(port_id.0),
            slot_id,
            parent_slot_id: Some(device.slot_id),
            route_string: new_route,
            psiv: speed,
            buffer,
        };
        let ctx = Arc::new(HciContext::new(Arc::downgrade(&self), device));
        if UsbManager::instantiate(UsbAddress(slot_id.0), ctx as Arc<dyn UsbHostInterface>).await {
            Ok(UsbAddress(slot_id.0))
        } else {
            todo!()
        }
    }

    pub async fn attach_root_device(self: Arc<Self>, port_id: PortId) -> Option<UsbAddress> {
        let port = self.port_by(port_id);
        self.wait_cnr(0);

        let trb = Trb::new(TrbType::ENABLE_SLOT_COMMAND);
        let slot_id = match self.execute_command(&trb) {
            Ok(result) => result.slot_id().unwrap(),
            Err(err) => {
                log!("ENABLE_SLOT ERROR {:?}", err.completion_code());
                return None;
            }
        };

        self.port2slot.write().unwrap()[port_id.0.get() as usize] = Some(slot_id);
        self.slot2port.write().unwrap()[slot_id.0.get() as usize] = Some(port_id);

        let device_context_size = self.context_size * 32;
        let device_context = unsafe { MemoryManager::alloc_pages(device_context_size) }
            .unwrap()
            .get() as u64;
        self.set_device_context(slot_id, device_context);

        let input_context_size = self.context_size * 33;
        let input_context_pa = unsafe { MemoryManager::alloc_pages(input_context_size) }
            .unwrap()
            .get() as u64;
        let input_context = self.input_context(slot_id);
        input_context.init(input_context_pa, self.context_size);

        let slot = input_context.slot();
        let speed_raw = port.status().speed_raw();
        slot.set_root_hub_port(port_id);
        slot.set_speed(speed_raw);
        slot.set_context_entries(1);
        let psiv = FromPrimitive::from_usize(speed_raw).unwrap_or(PSIV::SS);

        self.configure_endpoint(slot_id, DCI::CONTROL, EpType::Control, 0, 0, false);

        Timer::sleep(Duration::from_millis(10));

        let trb = TrbAddressDeviceCommand::new(slot_id, input_context_pa);
        match self.execute_command(&trb) {
            Ok(_result) => (),
            Err(err) => {
                log!("ADDRESS_DEVICE ERROR {:?}", err.completion_code());
            }
        }

        let buffer = unsafe { MemoryManager::alloc_pages(MemoryManager::PAGE_SIZE_MIN) }
            .unwrap()
            .get() as u64;
        let device = HciDeviceContext {
            root_port_id: port_id,
            port_id,
            slot_id,
            parent_slot_id: None,
            route_string: UsbRouteString::EMPTY,
            psiv,
            buffer,
        };
        let ctx = Arc::new(HciContext::new(Arc::downgrade(&self), device));

        if UsbManager::instantiate(UsbAddress(slot_id.0), ctx as Arc<dyn UsbHostInterface>).await {
            Timer::sleep_async(Duration::from_millis(10)).await;
        } else {
            let port = self.port_by(port_id);
            let status = port.status();
            port.write(status & PortSc::PRESERVE_MASK | PortSc::PRC | PortSc::PR);
        }

        Some(UsbAddress(slot_id.0))
    }

    pub fn set_max_packet_size(&self, slot_id: SlotId, max_packet_size: usize) -> Result<(), ()> {
        let input_context = self.input_context(slot_id);
        input_context.control().set_add(3);

        unsafe {
            let slot = input_context.slot() as *const _ as *mut u8;
            let dc = MemoryManager::direct_map(self.get_device_context(slot_id)) as *const u8;
            slot.copy_from(dc, self.context_size * 2);
        }

        let endpoint = input_context.endpoint(DCI::CONTROL);
        endpoint.set_max_packet_size(max_packet_size);

        let trb = TrbEvaluateContextCommand::new(slot_id, input_context.raw_data());
        match self.execute_command(&trb) {
            Ok(_) => Ok(()),
            Err(_) => todo!(),
        }
    }

    pub fn process_event(&self) {
        while let Some(event) = self.rts.irs0().dequeue_event(&self.event_cycle) {
            let event = match event.as_event() {
                Some(v) => v,
                None => {
                    panic!("XHCI: UNHANDLED EVENT TRB {:?}", event.trb_type(),);
                }
            };
            match event {
                TrbEvent::Transfer(event) => {
                    let event_trb = ScheduledTrb(event.ptr());
                    // log!(
                    //     "TRANSFER EVENT {:?} {:?}",
                    // unsafe { event_trb.peek().trb_type() },
                    // event.completion_code(),
                    // );

                    if event.completion_code() != Some(TrbCompletionCode::SUCCESS) {
                        unsafe {
                            let next_trb = {
                                let next_trb = event_trb.next();
                                if next_trb.peek().trb_type() != Some(TrbType::LINK) {
                                    next_trb
                                } else {
                                    let link_trb: &TrbLink = transmute(next_trb.peek());
                                    ScheduledTrb(link_trb.ptr() & !0x0F)
                                }
                            };

                            if event_trb.peek().trb_type() == Some(TrbType::DATA)
                                && next_trb.peek().trb_type() == Some(TrbType::STATUS)
                            {
                                // ex. STALL ERROR in DATA STAGE
                                event_trb.peek().set_trb_type(TrbType::NOP);
                                next_trb.peek().set_trb_type(TrbType::NOP);
                            } else if event_trb.peek().trb_type() == Some(TrbType::SETUP) {
                                let last_trb = {
                                    let last_trb = next_trb.next();
                                    if last_trb.peek().trb_type() != Some(TrbType::LINK) {
                                        last_trb
                                    } else {
                                        let link_trb: &TrbLink = transmute(next_trb.peek());
                                        ScheduledTrb(link_trb.ptr() & !0x0F)
                                    }
                                };
                                // ex. USB TRANSACTION ERROR in SETUP STAGE
                                if next_trb.peek().trb_type() == Some(TrbType::STATUS) {
                                    // SETUP - STATUS
                                    event_trb.peek().set_trb_type(TrbType::NOP);
                                    next_trb.peek().set_trb_type(TrbType::NOP);
                                } else if next_trb.peek().trb_type() == Some(TrbType::DATA)
                                    && last_trb.peek().trb_type() == Some(TrbType::STATUS)
                                {
                                    // SETUP - DATA - STATUS
                                    event_trb.peek().set_trb_type(TrbType::NOP);
                                    next_trb.peek().set_trb_type(TrbType::NOP);
                                    last_trb.peek().set_trb_type(TrbType::NOP);
                                } else {
                                    todo!()
                                }
                            }
                        }
                    }

                    let slot_id = event.slot_id();
                    let dci = event.dci();
                    let index = self.ep_ring_index(slot_id, dci).unwrap();
                    let ctx = &mut self.ring_context.write().unwrap()[index];
                    let ctx = unsafe { &mut *ctx.as_mut_ptr() };
                    ctx.set_response(event.as_common_trb());
                }
                TrbEvent::CommandCompletion(event) => {
                    let event_trb = ScheduledTrb(event.ptr());
                    if let Some(xrb) = self.find_xrb(event_trb, Some(RequestState::Scheduled)) {
                        xrb.set_response(event.as_common_trb());
                    } else {
                        todo!()
                    }
                }
                TrbEvent::PortStatusChange(event) => {
                    let port_id = event.port_id().unwrap();
                    self.port_status_change_queue.post(port_id).unwrap();
                }
                TrbEvent::DeviceNotification(event) => {
                    log!(
                        "DEVICE_NOTIFICATION {} {:?}",
                        event.slot_id().unwrap().0.get(),
                        event.completion_code()
                    );
                }
            }
            drop(event)
        }
    }

    /// xHCI Root hub task
    async fn _root_hub_task(self: Arc<Self>) {
        UsbManager::focus_root_hub();

        // Timer::sleep_async(Duration::from_millis(100)).await;

        for (index, port) in self.ports.iter().enumerate() {
            let port_id = PortId(NonZeroU8::new(index as u8 + 1).unwrap());
            self.wait_cnr(0);
            let status = port.status();

            if status.is_connected() {
                // if status.is_usb2() {
                port.set(PortSc::PR);
                // }

                let deadline = Timer::new(Duration::from_millis(200));
                loop {
                    if port.status().is_enabled() || deadline.is_expired() {
                        break;
                    }
                    Timer::sleep_async(Duration::from_millis(10)).await;
                }

                self.wait_cnr(0);
                let status = port.status();
                port.set(PortSc::ALL_CHANGE_BITS);
                if status.is_connected() && status.is_enabled() {
                    let _addr = self.clone().attach_root_device(port_id).await.unwrap();
                } else {
                    log!(
                        "ROOT PORT TIMED OUT {} {:08x} {:?} {:?}",
                        port_id.0.get(),
                        status.bits(),
                        status.speed(),
                        status.link_state()
                    );
                }
            }
        }

        while {
            let mut count = 0;
            for (index, port) in self.ports.iter().enumerate() {
                let port_id = PortId(NonZeroU8::new(index as u8 + 1).unwrap());
                self.wait_cnr(0);
                let status = port.status();
                if status.is_connected_status_changed() && status.is_connected() && status.is_usb2()
                {
                    self.root_hub_lock.lock_shared();
                    self.clone()._process_port_change(port_id).await;
                    count += 1;
                }
                port.set(PortSc::ALL_CHANGE_BITS);
            }
            self.root_hub_lock.wait().await;
            count > 0
        } {}
        UsbManager::unfocus_root_hub();

        while let Some(port_id) = self.port_status_change_queue.wait_event().await {
            let mut ports = Vec::new();
            ports.push(port_id);
            while let Some(port_id) = self.port_status_change_queue.get_event() {
                ports.push(port_id);
            }
            UsbManager::focus_root_hub();
            for port_id in ports {
                self.root_hub_lock.lock_shared();
                UsbManager::schedule_config_task(
                    None,
                    Box::pin(self.clone()._process_port_change(port_id)),
                );
            }
            self.root_hub_lock.wait().await;
            UsbManager::unfocus_root_hub();
        }
    }

    pub async fn _process_port_change(self: Arc<Self>, port_id: PortId) {
        let defer = self.root_hub_lock.unlock_shared();

        let port = self.port_by(port_id);
        self.wait_cnr(0);
        let status = port.status();

        if status.is_connected_status_changed() {
            if status.is_connected() {
                // Attached USB device
                port.set(PortSc::CSC | PortSc::PR);

                let deadline = Timer::new(Duration::from_millis(200));
                loop {
                    if port.status().is_enabled() || deadline.is_expired() {
                        break;
                    }
                    Timer::sleep_async(Duration::from_millis(10)).await;
                }

                port.set(PortSc::PRC);

                let status = port.status();
                if status.is_connected() && status.is_enabled() {
                    self.attach_root_device(port_id).await;
                } else {
                    port.write(
                        status & PortSc::PRESERVE_MASK & !PortSc::PP | PortSc::ALL_CHANGE_BITS,
                    );
                    Timer::sleep_async(Duration::from_millis(100)).await;
                    port.set(PortSc::PP | PortSc::PR);
                    log!("XHCI: PORT RESET TIMED OUT {}", port_id.0.get());
                }

                return;
            } else {
                // Detached USB device
                port.set(PortSc::CSC);

                let mut slice = self.port2slot.write().unwrap();
                let slot = slice.get_mut(port_id.0.get() as usize).unwrap();
                if let Some(slot_id) = slot.take() {
                    UsbManager::detach_device(UsbAddress(slot_id.0));
                }
            }
        }

        port.set(PortSc::ALL_CHANGE_BITS);

        drop(defer);
    }

    pub async fn focus_slot(self: Arc<Self>, hub: Option<SlotId>) -> Result<(), UsbError> {
        let _ = hub;
        todo!();
    }

    pub async fn unfocus_slot(self: Arc<Self>, hub: Option<SlotId>) -> Result<(), UsbError> {
        let _ = hub;
        todo!();
    }
}

impl Drop for Xhci {
    fn drop(&mut self) {
        todo!()
    }
}

impl PciDriver for Xhci {
    fn address(&self) -> PciConfigAddress {
        self.addr
    }

    fn name<'a>(&self) -> &'a str {
        Self::DRIVER_NAME
    }

    fn current_status(&self) -> String {
        let cmd = self.opr.read_cmd();
        let sts = self.opr.status();
        let status = if !cmd.contains(UsbCmd::RUN) {
            "STOPPED"
        } else if sts.contains(UsbSts::HCH) {
            "HALTED"
        } else {
            "RUNNING"
        };

        format!(
            "{} ports {} slots {} ctx {}",
            status,
            self.cap.max_ports(),
            self.cap.max_device_slots(),
            self.context_size,
        )
    }
}

struct EpRingContext {
    tr_base: Option<NonNullPhysicalAddress>,
    state: AtomicUsize,
    slot_id: Option<SlotId>,
    dci: Option<DCI>,
    index: usize,
    pcs: CycleBit,
    response: Trb,
    signal: Option<Pin<Arc<AsyncSemaphore>>>,
}

impl EpRingContext {
    #[inline]
    pub fn state(&self) -> RequestState {
        FromPrimitive::from_usize(self.state.load(Ordering::SeqCst)).unwrap_or_default()
    }

    #[inline]
    pub fn set_state(&self, val: RequestState) {
        self.state.store(val as usize, Ordering::SeqCst);
    }

    #[inline]
    pub fn compare_exchange_state(
        &self,
        current: RequestState,
        new: RequestState,
    ) -> Result<(), RequestState> {
        match self.state.compare_exchange_weak(
            current as usize,
            new as usize,
            Ordering::SeqCst,
            Ordering::Relaxed,
        ) {
            Ok(_) => Ok(()),
            Err(v) => Err(FromPrimitive::from_usize(v).unwrap_or_default()),
        }
    }

    #[inline]
    pub const fn tr_base(&self) -> Option<NonNullPhysicalAddress> {
        self.tr_base
    }

    #[inline]
    pub fn tr_value(&self) -> Option<NonNullPhysicalAddress> {
        self.tr_base
            .and_then(|v| NonNullPhysicalAddress::new(v.get() | self.pcs.tr_value()))
    }

    #[inline]
    pub const fn slot_id(&self) -> Option<SlotId> {
        self.slot_id
    }

    #[inline]
    pub const fn dci(&self) -> Option<DCI> {
        self.dci
    }

    #[inline]
    pub const fn pcs(&self) -> &CycleBit {
        &self.pcs
    }

    #[inline]
    pub const fn size() -> usize {
        (Xhci::MAX_TR_INDEX + 1) * size_of::<Trb>()
    }

    #[inline]
    pub fn asem(&self) -> Option<&Pin<Arc<AsyncSemaphore>>> {
        self.signal.as_ref()
    }

    #[inline]
    pub fn clear(&mut self) {
        if let Some(tr_base) = self.tr_base {
            unsafe {
                let p = MemoryManager::direct_map(tr_base.get()) as *const c_void as *mut c_void;
                p.write_bytes(0, Self::size());
            }
        }
        self.pcs.reset();
        self.index = 0;
        self.response = Trb::empty();
    }

    #[inline]
    pub fn alloc(&mut self, slot_id: Option<SlotId>, dci: Option<DCI>) {
        self.tr_base = NonNullPhysicalAddress::new(
            unsafe { MemoryManager::alloc_pages(Self::size()) }
                .unwrap()
                .get() as PhysicalAddress,
        );
        self.slot_id = slot_id;
        self.dci = dci;
        self.pcs.reset();
        self.index = 0;
        self.response = Trb::empty();
        self.signal = Some(AsyncSemaphore::new(0));
    }

    #[inline]
    pub fn set_response<T: TrbCommon>(&self, response: &T) {
        self.response.raw_copy_from(response);
        match self.compare_exchange_state(RequestState::Scheduled, RequestState::Completed) {
            Ok(_) => self.signal.as_ref().unwrap().signal(),
            Err(_err) => {
                todo!()
            }
        }
    }

    #[inline]
    pub fn set_scheduled(&self) {
        self.set_state(RequestState::Scheduled);
    }

    #[inline]
    pub fn dispose(&self) {
        self.set_state(RequestState::Available);
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScheduledTrb(pub u64);

impl ScheduledTrb {
    #[inline]
    pub fn next(&self) -> Self {
        Self(self.0 + size_of::<Trb>() as u64)
    }

    #[inline]
    pub unsafe fn peek(&self) -> &mut Trb {
        let p = MemoryManager::direct_map(self.0) as usize as *mut Trb;
        &mut *p
    }
}

pub struct XhciRequestBlock {
    state: AtomicUsize,
    scheduled_trb: ScheduledTrb,
    signal: XrbSignalObject,
    reuse_delay: Timer,
    request: Trb,
    response: Trb,
}

pub enum XrbSignalObject {
    Sync(Semaphore),
    Async(Pin<Arc<AsyncSemaphore>>),
}

impl XhciRequestBlock {
    pub const EMPTY: UnsafeCell<Self> = UnsafeCell::new(Self::new());

    #[inline]
    pub const fn new() -> Self {
        Self {
            state: AtomicUsize::new(0),
            scheduled_trb: ScheduledTrb(0),
            signal: XrbSignalObject::Sync(Semaphore::new(0)),
            reuse_delay: Timer::JUST,
            request: Trb::empty(),
            response: Trb::empty(),
        }
    }

    #[inline]
    pub fn state(&self) -> RequestState {
        FromPrimitive::from_usize(self.state.load(Ordering::SeqCst)).unwrap_or_default()
    }

    #[inline]
    pub fn set_state(&self, val: RequestState) {
        self.state.store(val as usize, Ordering::SeqCst);
    }

    #[inline]
    pub fn compare_exchange_state(
        &self,
        current: RequestState,
        new: RequestState,
    ) -> Result<(), RequestState> {
        match self.state.compare_exchange_weak(
            current as usize,
            new as usize,
            Ordering::SeqCst,
            Ordering::Relaxed,
        ) {
            Ok(_) => Ok(()),
            Err(v) => Err(FromPrimitive::from_usize(v).unwrap_or_default()),
        }
    }

    #[inline]
    pub fn schedule(&mut self, request: &Trb, scheduled_trb: ScheduledTrb) {
        self.scheduled_trb = scheduled_trb;
        self.request.raw_copy_from(request);
        self.response = Trb::empty();
        fence(Ordering::SeqCst);
        self.set_state(RequestState::Scheduled);
    }

    #[inline]
    pub fn try_to_acquire(&self) -> bool {
        if self.reuse_delay.until() {
            return false;
        }
        self.compare_exchange_state(RequestState::Available, RequestState::Acquired)
            .is_ok()
    }

    #[inline]
    pub fn dispose(&mut self) {
        self.reuse_delay = Timer::new(Duration::from_millis(10));
        self.signal = XrbSignalObject::Sync(Semaphore::new(0));
        self.set_state(RequestState::Available);
    }

    #[inline]
    pub fn wait(&self) {
        match self.signal {
            XrbSignalObject::Sync(ref sem) => sem.wait(),
            XrbSignalObject::Async(_) => unreachable!(),
        }
    }

    #[inline]
    pub async fn wait_async(&self) {
        match self.signal {
            XrbSignalObject::Sync(_) => unreachable!(),
            XrbSignalObject::Async(ref asem) => asem.clone().wait().await,
        }
    }

    #[inline]
    pub fn prepare_async(&mut self) {
        self.signal = XrbSignalObject::Async(AsyncSemaphore::new(0));
    }

    #[inline]
    pub fn set_response(&mut self, response: &Trb) {
        self.response.raw_copy_from(response);
        match self.compare_exchange_state(RequestState::Scheduled, RequestState::Completed) {
            Ok(_) => match self.signal {
                XrbSignalObject::Sync(ref sem) => sem.signal(),
                XrbSignalObject::Async(ref asem) => asem.signal(),
            },
            Err(_err) => {
                // TODO:
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
pub enum RequestState {
    Available = 0,
    Acquired,
    Scheduled,
    Completed,
    Aborted,
}

impl Default for RequestState {
    #[inline]
    fn default() -> Self {
        Self::Available
    }
}

pub struct InputContext {
    pa: PhysicalAddress,
    context_size: usize,
}

impl InputContext {
    pub const EMPTY: UnsafeCell<Self> = UnsafeCell::new(Self::empty());

    #[inline]
    pub const fn empty() -> Self {
        Self {
            pa: 0,
            context_size: 0,
        }
    }

    #[inline]
    pub fn init(&mut self, pa: PhysicalAddress, context_size: usize) {
        self.pa = pa;
        self.context_size = context_size;
    }

    #[inline]
    pub fn raw_data(&self) -> u64 {
        self.pa as u64
    }

    #[inline]
    pub fn control<'a>(&self) -> &'a mut InputControlContext {
        unsafe {
            &mut *(MemoryManager::direct_map(self.raw_data() as PhysicalAddress)
                as *mut InputControlContext)
        }
    }

    #[inline]
    pub fn slot<'a>(&self) -> &'a mut SlotContext {
        unsafe {
            &mut *((MemoryManager::direct_map(self.raw_data() as PhysicalAddress)
                + self.context_size) as *mut SlotContext)
        }
    }

    #[inline]
    pub fn endpoint<'a>(&self, dci: DCI) -> &'a mut EndpointContext {
        unsafe {
            &mut *((MemoryManager::direct_map(self.raw_data() as PhysicalAddress)
                + self.context_size * (1 + dci.0.get() as usize))
                as *mut EndpointContext)
        }
    }
}

/// Host Controller Interface Device Context
pub struct HciContext {
    host: Weak<Xhci>,
    device: UnsafeCell<HciDeviceContext>,
}

pub struct HciDeviceContext {
    root_port_id: PortId,
    #[allow(dead_code)]
    port_id: PortId,
    slot_id: SlotId,
    parent_slot_id: Option<SlotId>,
    route_string: UsbRouteString,
    psiv: PSIV,
    buffer: u64,
}

impl HciContext {
    #[inline]
    pub const fn new(host: Weak<Xhci>, device: HciDeviceContext) -> Self {
        Self {
            host,
            device: UnsafeCell::new(device),
        }
    }

    #[inline]
    fn device<'a>(&self) -> &'a mut HciDeviceContext {
        unsafe { &mut *self.device.get() }
    }
}

impl UsbHostInterface for HciContext {
    fn parent_device_address(&self) -> Option<UsbAddress> {
        self.device().parent_slot_id.map(|v| UsbAddress(v.0))
    }

    fn route_string(&self) -> UsbRouteString {
        self.device().route_string
    }

    fn speed(&self) -> PSIV {
        self.device().psiv
    }

    fn set_max_packet_size(&self, max_packet_size: usize) -> Result<(), UsbError> {
        let host = match self.host.upgrade() {
            Some(v) => v.clone(),
            None => return Err(UsbError::HostUnavailable),
        };
        let device = self.device();
        let slot_id = device.slot_id;
        host.set_max_packet_size(slot_id, max_packet_size)
            .map_err(|_| UsbError::General)
    }

    fn configure_hub2(&self, hub_desc: &Usb2HubDescriptor, is_mtt: bool) -> Result<(), UsbError> {
        let host = match self.host.upgrade() {
            Some(v) => v.clone(),
            None => return Err(UsbError::HostUnavailable),
        };
        let device = self.device();
        let slot_id = device.slot_id;

        host.configure_hub2(slot_id, hub_desc, is_mtt)
    }

    fn configure_hub3(&self, hub_desc: &Usb3HubDescriptor) -> Result<(), UsbError> {
        let host = match self.host.upgrade() {
            Some(v) => v.clone(),
            None => return Err(UsbError::HostUnavailable),
        };
        let device = self.device();
        let slot_id = device.slot_id;

        host.configure_hub3(slot_id, hub_desc)
    }

    fn focus_hub(self: Arc<Self>) -> Pin<Box<dyn Future<Output = Result<(), UsbError>>>> {
        let host = match self.host.upgrade() {
            Some(v) => v.clone(),
            None => return Box::pin(AsyncUsbError::new(UsbError::HostUnavailable)),
        };
        Box::pin(host.focus_slot(Some(self.device().slot_id)))
    }

    fn unfocus_hub(self: Arc<Self>) -> Pin<Box<dyn Future<Output = Result<(), UsbError>>>> {
        let host = match self.host.upgrade() {
            Some(v) => v.clone(),
            None => return Box::pin(AsyncUsbError::new(UsbError::HostUnavailable)),
        };
        Box::pin(host.unfocus_slot(Some(self.device().slot_id)))
    }

    fn attach_child_device(
        self: Arc<Self>,
        port_id: UsbHubPortNumber,
        speed: PSIV,
    ) -> Pin<Box<dyn Future<Output = Result<UsbAddress, UsbError>>>> {
        let host = match self.host.upgrade() {
            Some(v) => v.clone(),
            None => return Box::pin(AsyncUsbError::new(UsbError::HostUnavailable)),
        };
        Box::pin(host.attach_child_device(self.clone(), port_id, speed))
    }

    fn configure_endpoint(&self, desc: &UsbEndpointDescriptor) -> Result<(), UsbError> {
        let host = match self.host.upgrade() {
            Some(v) => v.clone(),
            None => return Err(UsbError::HostUnavailable),
        };
        let device = self.device();
        let slot_id = device.slot_id;

        let ep = match desc.endpoint_address() {
            Some(v) => v,
            None => return Err(UsbError::InvalidParameter),
        };
        let ep_type = match desc.ep_type() {
            Some(v) => v,
            None => return Err(UsbError::InvalidParameter),
        };
        let ep_type = EpType::from_usb_ep_type(ep_type, ep.is_dir_in());
        let dci = ep.into();

        host.configure_endpoint(
            slot_id,
            dci,
            ep_type,
            desc.max_packet_size() as usize,
            desc.interval(),
            true,
        );

        let trb = TrbConfigureEndpointCommand::new(slot_id, host.input_context(slot_id).raw_data());
        match host.execute_command(&trb) {
            Ok(_) => Ok(()),
            Err(_err) => Err(UsbError::General),
        }
    }

    unsafe fn control(
        self: Arc<Self>,
        setup: UsbControlSetupData,
    ) -> Pin<Box<dyn Future<Output = Result<(*const u8, usize), UsbError>>>> {
        let host = match self.host.upgrade() {
            Some(ref v) => v.clone(),
            None => return Box::pin(AsyncUsbError::new(UsbError::HostUnavailable)),
        };
        let device = self.device();

        Box::pin(
            host.clone()
                .control_async2(device.slot_id, setup, device.buffer),
        )
    }

    unsafe fn control_send(
        self: Arc<Self>,
        setup: UsbControlSetupData,
        data: *const u8,
    ) -> Pin<Box<dyn Future<Output = Result<usize, UsbError>>>> {
        let host = match self.host.upgrade() {
            Some(ref v) => v.clone(),
            None => return Box::pin(AsyncUsbError::new(UsbError::HostUnavailable)),
        };
        let len = setup.wLength as usize;
        if len == 0 {
            return Box::pin(AsyncUsbError::new(UsbError::InvalidParameter));
        }
        let device = self.device();

        let p = MemoryManager::direct_map(device.buffer) as *mut u8;
        p.copy_from(data, len);

        Box::pin(
            host.clone()
                .control_async(device.slot_id, setup, device.buffer),
        )
    }

    unsafe fn read(
        self: Arc<Self>,
        ep: UsbEndpointAddress,
        buffer: *mut u8,
        len: usize,
    ) -> Pin<Box<dyn Future<Output = Result<usize, UsbError>>>> {
        let host = match self.host.upgrade() {
            Some(ref v) => v.clone(),
            None => return Box::pin(AsyncUsbError::new(UsbError::HostUnavailable)),
        };
        let dci = DCI::from(ep);
        if !dci.can_read() {
            return Box::pin(AsyncUsbError::new(UsbError::InvalidParameter));
        }
        let device = self.device();

        let p = MemoryManager::direct_map(device.buffer) as *mut u8;
        p.write_bytes(0, len);

        let slot_id = device.slot_id;

        Box::pin(host.read_async(slot_id, dci, device.buffer, len, buffer))
    }

    unsafe fn write(
        self: Arc<Self>,
        ep: UsbEndpointAddress,
        buffer: *const u8,
        len: usize,
    ) -> Pin<Box<dyn Future<Output = Result<usize, UsbError>>>> {
        let host = match self.host.upgrade() {
            Some(ref v) => v.clone(),
            None => return Box::pin(AsyncUsbError::new(UsbError::HostUnavailable)),
        };
        let dci = DCI::from(ep);
        if !dci.can_write() {
            return Box::pin(AsyncUsbError::new(UsbError::InvalidParameter));
        }
        let device = self.device();
        let slot_id = device.slot_id;

        let p = MemoryManager::direct_map(device.buffer) as *mut u8;
        p.copy_from(buffer, len);

        Box::pin(host.transfer_async(slot_id, dci, device.buffer, len))
    }
}

#[derive(Debug, Clone, Copy)]
struct AsyncUsbError<T> {
    error: UsbError,
    _phantom: PhantomData<T>,
}

impl<T> AsyncUsbError<T> {
    pub const fn new(error: UsbError) -> Self {
        Self {
            error,
            _phantom: PhantomData,
        }
    }
}

impl<T> Future for AsyncUsbError<T> {
    type Output = Result<T, UsbError>;

    fn poll(
        self: Pin<&mut Self>,
        _cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        Poll::Ready(Err(self.error))
    }
}
