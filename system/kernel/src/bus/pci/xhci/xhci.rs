//! xHCI: Extensible Host Controller Interface

use super::{data::*, regs::*};
use crate::{
    arch::cpu::Cpu,
    bus::pci::*,
    bus::usb::*,
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
};
use core::{
    cell::UnsafeCell, ffi::c_void, fmt::Write, marker::PhantomData, mem::size_of, num::NonZeroU64,
    pin::Pin, slice, sync::atomic::*, task::Poll, time::Duration,
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

    ring_context: RwLock<[EpRingContext; Self::MAX_TR]>,
    event_cycle: CycleBit,
    port_status_change_queue: AsyncEventQueue<PortId>,
    port2slot: RwLock<[Option<SlotId>; 256]>,
    xrbs: [UnsafeCell<XhciRequestBlock>; Self::MAX_XRB],
    ics: [UnsafeCell<InputContext>; Self::MAX_DEVICE_SLOTS],

    lock_config: Pin<Arc<AsyncSemaphore>>,
    lock_control: BinarySemaphore,
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
            ring_context: RwLock::new([EpRingContext::EMPTY; Self::MAX_TR]),
            event_cycle: CycleBit::from(true),
            ers,
            port_status_change_queue: AsyncEventQueue::new(Self::MAX_PORT_CHANGE),
            port2slot: RwLock::new([None; 256]),
            xrbs: [XhciRequestBlock::EMPTY; Self::MAX_XRB],
            ics: [InputContext::EMPTY; Self::MAX_DEVICE_SLOTS],
            lock_config: AsyncSemaphore::new(1),
            lock_control: BinarySemaphore::new(),
        });

        let p = driver.clone();
        SpawnOption::with_priority(Priority::Realtime).spawn(
            move || {
                p._event_thread();
            },
            "xhci.event",
        );

        Some(driver as Arc<dyn PciDriver>)
    }

    ///  xHCI Initialize
    fn initialize(&self) {
        if let Some(xecp) = self.cap.xecp() {
            unsafe {
                let mut xecp_base = xecp.get() as *mut u32;
                loop {
                    let xecp = xecp_base.read_volatile();
                    match xecp & 0xFF {
                        0x01 => {
                            // USB Legacy
                            const USBLEGSUP_BIOS_OWNED: u32 = 0x00010000;
                            const USBLEGSUP_OS_OWNED: u32 = 0x01000000;
                            xecp_base.write_volatile(xecp | USBLEGSUP_OS_OWNED);
                            while (xecp_base.read_volatile() & USBLEGSUP_BIOS_OWNED) != 0 {
                                Timer::sleep(Duration::from_millis(10));
                            }
                            let data = xecp_base.add(1);
                            data.write_volatile((data.read_volatile() & 0x000E1FEE) | 0xE0000000);
                        }
                        _ => (),
                    }
                    let xecp_ptr = ((xecp >> 8) & 0xFF) as usize;
                    if xecp_ptr == 0 {
                        break;
                    } else {
                        xecp_base = xecp_base.add(xecp_ptr as usize);
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
        unsafe {
            let dcbaa_size = self.dcbaa_len * 8;
            let pa_dcbaa = MemoryManager::alloc_pages(dcbaa_size).unwrap().get() as u64;
            self.opr.set_dcbaap(NonZeroU64::new(pa_dcbaa).unwrap());
        };

        // make Scratchpad
        let max_scratchpad_size = self.cap.max_scratchpad_size();
        if max_scratchpad_size > 0 {
            let size = max_scratchpad_size * self.opr.page_size();
            unsafe {
                let scratchpad = MemoryManager::alloc_pages(size).unwrap().get() as u64;
                self.dcbaa()[0] = scratchpad;
            }
        }

        // Command Ring Control Register
        self.opr.set_crcr(self.alloc_ep_ring(None, None).unwrap());

        // Event Ring Segment Table
        self.rts.irs0().init(self.ers);

        // start xHC
        self.wait_cnr(0);
        self.opr.set_cmd(UsbCmd::RUN);
        while self.opr.status().contains(UsbSts::HCH) {
            Timer::sleep(Duration::from_millis(10));
        }

        log!("XHCI Started");
    }

    fn dcbaa(&self) -> &'static mut [u64] {
        unsafe {
            slice::from_raw_parts_mut(
                MemoryManager::direct_map(self.opr.dcbaap() & !63) as *mut u64,
                self.dcbaa_len,
            )
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

    pub fn find_ep_ring(&self, slot_id: Option<SlotId>, dci: Option<DCI>) -> Option<usize> {
        let slot_id = slot_id.map(|v| v.0.get()).unwrap_or_default();
        let dci = dci.map(|v| v.0.get()).unwrap_or_default();
        for (index, ctx) in self.ring_context.read().unwrap().iter().enumerate() {
            let ctx_slot_id = ctx.slot_id().map(|v| v.0.get()).unwrap_or_default();
            let ctx_dci = ctx.dci().map(|v| v.0.get()).unwrap_or_default();
            if ctx.tr_base() != 0 && ctx_slot_id == slot_id && ctx_dci == dci {
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
        if let Some(index) = self.find_ep_ring(slot_id, dci) {
            let ctx = &mut self.ring_context.write().unwrap()[index];
            ctx.reset();
            return ctx.tr_value();
        }
        for ctx in self.ring_context.write().unwrap().iter_mut() {
            if ctx.tr_base() == 0 {
                ctx.alloc(slot_id, dci);
                return ctx.tr_value();
            }
        }
        None
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
        state: Option<XrbState>,
    ) -> Option<&'a mut XhciRequestBlock> {
        for xrb in &self.xrbs {
            let xrb = unsafe { &mut *xrb.get() };
            let xrb_state = xrb.state();
            if xrb_state != XrbState::Available && xrb.scheduled_trb == scheduled_trb {
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
        let index = match self.find_ep_ring(slot_id, dci) {
            Some(index) => index,
            None => todo!(),
        };
        let ctx = &mut self.ring_context.write().unwrap()[index];

        let tr_base = ctx.tr_base();
        let tr = MemoryManager::direct_map(tr_base) as *const Trb as *mut Trb;
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

    /// Execute control transfer
    pub fn execute_control(
        &self,
        slot_id: SlotId,
        setup: UsbControlSetupData,
        buffer: u64,
    ) -> Result<usize, TrbTxe> {
        self.lock_control.synchronized(|| {
            let trt = if setup.wLength > 0 {
                if setup.bmRequestType.is_device_to_host() {
                    TrbTranfserType::ControlIn
                } else {
                    TrbTranfserType::ControlOut
                }
            } else {
                TrbTranfserType::NoData
            };
            let dci = Some(DCI::CONTROL);
            let slot_id = Some(slot_id);
            let setup_trb = TrbSetupStage::new(trt, setup);
            self.issue_trb(None, &setup_trb, slot_id, dci, false);

            let dir = trt == TrbTranfserType::ControlIn;
            if setup.wLength > 0 {
                let data_trb = TrbDataStage::new(buffer, setup.wLength as usize, dir);
                self.issue_trb(None, &data_trb, slot_id, dci, false);
            }

            let xrb = self.allocate_xrb().unwrap();
            let status_trb = TrbStatusStage::new(!dir, true);
            self.issue_trb(Some(xrb), &status_trb, slot_id, dci, true);

            xrb.wait();

            let result = match xrb.response.as_event() {
                Some(TrbEvent::TransferEvent(v)) => Some(v.copied()),
                _ => None,
            };
            xrb.dispose();
            match result {
                Some(result) => {
                    if result.completion_code() == Some(TrbCompletionCode::SUCCESS) {
                        Ok(setup.wLength as usize - result.transfer_length())
                    } else {
                        Err(result)
                    }
                }
                None => Err(TrbTxe::empty()),
            }
        })
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
        hub_desc: &UsbHub2Descriptor,
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

    pub fn attach_device(
        self: Arc<Self>,
        hub: &HciContext,
        port_id: UsbHubPortNumber,
        speed: PSIV,
    ) -> Result<UsbDeviceAddress, UsbError> {
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
                return Err(UsbError::General);
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
        slot.set_route_string(new_route);
        slot.set_speed(speed as usize);
        slot.set_context_entries(1);

        match speed {
            PSIV::FS | PSIV::LS => {
                slot.set_parent_hub_slot_id(device.slot_id);
                slot.set_parent_port_id(port_id);
            }
            _ => (),
        }

        self.configure_endpoint(slot_id, DCI::CONTROL, EpType::Control, 0, 0, false);

        Timer::sleep(Duration::from_millis(100));

        log!(
            "ATTACH HUB DEVICE: ROOT {} ROUTE {:05x} SLOT {}",
            device.root_port_id.0.get(),
            new_route.as_u32(),
            slot_id.0.get()
        );

        let trb = TrbAddressDeviceCommand::new(slot_id, input_context_pa);
        match self.execute_command(&trb) {
            Ok(_result) => {
                //
            }
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
        UsbManager::instantiate(
            UsbDeviceAddress(slot_id.0),
            ctx as Arc<dyn UsbHostInterface>,
        );

        Ok(UsbDeviceAddress(slot_id.0))
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

    pub fn reset_port(&self, port: &PortRegisters) -> PortSc {
        self.wait_cnr(0);
        let status = port.portsc();
        let ccs_csc = PortSc::CCS | PortSc::CSC;
        if (status & ccs_csc) == ccs_csc {
            port.write_portsc(status & PortSc::PRESERVE_MASK | PortSc::CSC | PortSc::PR);
            self.wait_cnr(0);
            while port.portsc().contains(PortSc::PR) {
                Cpu::spin_loop_hint();
            }
        }
        port.portsc()
    }

    pub fn port_initialize(&self, port_id: PortId) -> Option<SlotId> {
        self.lock_control.synchronized(|| {
            let port = self.port_by(port_id);
            self.wait_cnr(0);
            let status = port.portsc();
            if status.contains(PortSc::CSC) {
                if status.contains(PortSc::CCS) {
                    // Attached USB device
                    port.write_portsc(status & PortSc::PRESERVE_MASK | PortSc::CSC | PortSc::PR);
                    while !port.portsc().contains(PortSc::PED) {
                        Timer::sleep(Duration::from_millis(10));
                    }
                    let status = port.portsc();
                    if status.contains(PortSc::PRC) {
                        port.write_portsc(status & PortSc::PRESERVE_MASK | PortSc::PRC);
                    }
                    if status.contains(PortSc::PR) || !status.contains(PortSc::PED) {
                        log!("XHCI: PORT RESET TIMEDOUT {}", port_id.0.get());
                        return None;
                    }

                    let trb = Trb::new(TrbType::ENABLE_SLOT_COMMAND);
                    let slot_id = match self.execute_command(&trb) {
                        Ok(result) => result.slot_id().unwrap(),
                        Err(err) => {
                            log!("ENABLE_SLOT ERROR {:?}", err.completion_code());
                            return None;
                        }
                    };

                    self.port2slot.write().unwrap()[port_id.0.get() as usize] = Some(slot_id);

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
                    slot.set_root_hub_port(port_id);
                    slot.set_speed(port.portsc().speed_raw());
                    slot.set_context_entries(1);

                    self.configure_endpoint(slot_id, DCI::CONTROL, EpType::Control, 0, 0, false);

                    Timer::sleep(Duration::from_millis(10));

                    let trb = TrbAddressDeviceCommand::new(slot_id, input_context_pa);
                    match self.execute_command(&trb) {
                        Ok(_result) => {
                            // log!(
                            //     "ADDRESS DEVICE {:?} {:?} DC {:016x}",
                            //     port_id, slot_id, device_context,
                            // );
                        }
                        Err(err) => {
                            log!("ADDRESS_DEVICE ERROR {:?}", err.completion_code());
                        }
                    }

                    return Some(slot_id);
                } else {
                    // Detached USB device
                    port.write_portsc(status & PortSc::PRESERVE_MASK | PortSc::CSC);

                    let mut slice = self.port2slot.write().unwrap();
                    let slot = slice.get_mut(port_id.0.get() as usize).unwrap();
                    let slot_id = match *slot {
                        Some(v) => v,
                        None => return None,
                    };
                    *slot = None;

                    UsbManager::detach_device(UsbDeviceAddress(slot_id.0));
                }
            }
            None
        })
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
                TrbEvent::CommandCompletion(event) => {
                    let scheduled_trb = ScheduledTrb(event.ptr());
                    if let Some(xrb) = self.find_xrb(scheduled_trb, Some(XrbState::Scheduled)) {
                        xrb.set_response(event.as_common_trb());
                    }
                }
                TrbEvent::PortStatusChange(event) => {
                    let port_id = event.port_id().unwrap();
                    self.port_status_change_queue.post(port_id).unwrap();
                }
                TrbEvent::TransferEvent(event) => {
                    let scheduled_trb = ScheduledTrb(event.ptr());
                    if let Some(xrb) = self.find_xrb(scheduled_trb, Some(XrbState::Scheduled)) {
                        xrb.set_response(event.as_common_trb());
                    }
                }
            }
        }
    }

    /// xHCI Main event loop
    fn _event_thread(self: Arc<Self>) {
        self.initialize();

        UsbManager::register_xfer_task(Task::new(Self::_config_task(self.clone())));

        loop {
            self.process_event();
            Timer::sleep(Duration::from_millis(10));
        }
    }

    /// xHCI Configuration thread
    async fn _config_task(self: Arc<Self>) {
        // Because QEMU does not generate the PORT_STATUS_CHANGE event without a dummy reset for each port.
        for port in self.ports {
            self.reset_port(port);
        }

        loop {
            while let Some(port_id) = self.port_status_change_queue.wait_event().await {
                self.lock_config.clone().wait().await;
                if let Some(slot_id) = self.port_initialize(port_id) {
                    let buffer = unsafe { MemoryManager::alloc_pages(MemoryManager::PAGE_SIZE_MIN) }
                        .unwrap()
                        .get() as u64;
                    let device = HciDeviceContext {
                        root_port_id: port_id,
                        port_id,
                        slot_id,
                        parent_slot_id: None,
                        route_string: UsbRouteString::EMPTY,
                        psiv: self.port_by(port_id).portsc().speed().unwrap(),
                        buffer,
                    };
                    let ctx = Arc::new(HciContext::new(Arc::downgrade(&self), device));

                    UsbManager::instantiate(
                        UsbDeviceAddress(slot_id.0),
                        ctx as Arc<dyn UsbHostInterface>,
                    );
                    Timer::sleep_async(Duration::from_millis(10)).await;
                }
                self.lock_config.clone().signal();
            }
        }
    }
}

impl Drop for Xhci {
    fn drop(&mut self) {
        unreachable!()
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
    tr_base: PhysicalAddress,
    response: Trb,
    slot_id: Option<SlotId>,
    dci: Option<DCI>,
    index: usize,
    pcs: CycleBit,
}

impl EpRingContext {
    pub const EMPTY: Self = Self::new();

    #[inline]
    const fn new() -> Self {
        Self {
            tr_base: 0,
            response: Trb::new(TrbType::RESERVED),
            slot_id: None,
            dci: None,
            index: 0,
            pcs: CycleBit::new(),
        }
    }

    #[inline]
    pub const fn tr_base(&self) -> PhysicalAddress {
        self.tr_base
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
    pub fn tr_value(&self) -> Option<NonNullPhysicalAddress> {
        NonNullPhysicalAddress::new(self.tr_base | self.pcs.tr_value())
    }

    #[inline]
    pub const fn size() -> usize {
        (Xhci::MAX_TR_INDEX + 1) * size_of::<Trb>()
    }

    #[inline]
    pub fn reset(&mut self) {
        if self.tr_base != 0 {
            unsafe {
                let p = MemoryManager::direct_map(self.tr_base) as *const c_void as *mut c_void;
                p.write_bytes(0, Self::size());
            }
        }
        self.response = Trb::new(TrbType::RESERVED);
        self.pcs.reset();
        self.index = 0;
    }

    #[inline]
    pub fn alloc(&mut self, slot_id: Option<SlotId>, dci: Option<DCI>) {
        self.tr_base = unsafe { MemoryManager::alloc_pages(Self::size()) }
            .unwrap()
            .get() as PhysicalAddress;
        self.slot_id = slot_id;
        self.dci = dci;
        self.pcs.reset();
        self.index = 0;
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScheduledTrb(pub u64);

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
    pub fn state(&self) -> XrbState {
        FromPrimitive::from_usize(self.state.load(Ordering::SeqCst)).unwrap_or_default()
    }

    #[inline]
    pub fn set_state(&self, val: XrbState) {
        self.state.store(val as usize, Ordering::SeqCst);
    }

    #[inline]
    pub fn compare_exchange_state(&self, current: XrbState, new: XrbState) -> Result<(), XrbState> {
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
        self.set_state(XrbState::Scheduled);
    }

    #[inline]
    pub fn try_to_acquire(&self) -> bool {
        if self.reuse_delay.until() {
            return false;
        }
        self.compare_exchange_state(XrbState::Available, XrbState::Acquired)
            .is_ok()
    }

    #[inline]
    pub fn dispose(&mut self) {
        self.reuse_delay = Timer::new(Duration::from_millis(10));
        self.signal = XrbSignalObject::Sync(Semaphore::new(0));
        self.set_state(XrbState::Available);
    }

    #[inline]
    pub fn wait(&self) {
        match self.signal {
            XrbSignalObject::Sync(ref sem) => sem.wait(),
            XrbSignalObject::Async(_) => unreachable!(),
        }
    }

    #[inline]
    pub fn prepare_async(&mut self) {
        self.signal = XrbSignalObject::Async(AsyncSemaphore::new(0));
    }

    #[inline]
    pub fn set_response(&mut self, response: &Trb) {
        self.response.raw_copy_from(response);
        match self.compare_exchange_state(XrbState::Scheduled, XrbState::Completed) {
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

/// XHCI Request Block State
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
pub enum XrbState {
    Available = 0,
    Acquired,
    Scheduled,
    Completed,
    Aborted,
}

impl Default for XrbState {
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
    fn parent_device_address(&self) -> Option<UsbDeviceAddress> {
        self.device().parent_slot_id.map(|v| UsbDeviceAddress(v.0))
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

    fn enter_configuration(&self) -> Pin<Box<dyn Future<Output = Result<(), UsbError>>>> {
        let host = match self.host.upgrade() {
            Some(v) => v.clone(),
            None => return Box::pin(AsyncUsbError::new(UsbError::HostUnavailable)),
        };
        host.lock_config.clone().wait_ok()
    }

    fn leave_configuration(&self) -> Result<(), UsbError> {
        let host = match self.host.upgrade() {
            Some(v) => v.clone(),
            None => return Err(UsbError::HostUnavailable),
        };
        host.lock_config.signal();
        Ok(())
    }

    fn configure_hub2(&self, hub_desc: &UsbHub2Descriptor, is_mtt: bool) -> Result<(), UsbError> {
        let host = match self.host.upgrade() {
            Some(v) => v.clone(),
            None => return Err(UsbError::HostUnavailable),
        };
        let device = self.device();
        let slot_id = device.slot_id;

        host.configure_hub2(slot_id, hub_desc, is_mtt)
    }

    fn attach_device(
        &self,
        port_id: UsbHubPortNumber,
        speed: PSIV,
    ) -> Result<UsbDeviceAddress, UsbError> {
        let host = match self.host.upgrade() {
            Some(v) => v.clone(),
            None => return Err(UsbError::HostUnavailable),
        };
        // let device = self.device();
        host.attach_device(self, port_id, speed)
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

    fn control<'a>(&self, setup: UsbControlSetupData) -> Result<&'a [u8], UsbError> {
        let host = match self.host.upgrade() {
            Some(v) => v.clone(),
            None => return Err(UsbError::HostUnavailable),
        };
        let device = self.device();

        match host.execute_control(device.slot_id, setup, device.buffer) {
            Ok(result) => {
                let result = unsafe {
                    slice::from_raw_parts(
                        MemoryManager::direct_map(device.buffer as PhysicalAddress) as *const u8,
                        result,
                    )
                };
                Ok(result)
            }
            Err(_err) => Err(UsbError::General),
        }
    }

    fn control_send(&self, setup: UsbControlSetupData, buffer: &[u8]) -> Result<usize, UsbError> {
        let host = match self.host.upgrade() {
            Some(v) => v.clone(),
            None => return Err(UsbError::HostUnavailable),
        };
        let len = setup.wLength as usize;
        if len == 0 || buffer.len() < len {
            return Err(UsbError::InvalidParameter);
        }
        let device = self.device();

        unsafe {
            let p = MemoryManager::direct_map(device.buffer) as *mut u8;
            p.copy_from(&buffer[0] as *const u8, len);
        }

        match host.execute_control(device.slot_id, setup, device.buffer) {
            Ok(result) => Ok(result),
            Err(_err) => Err(UsbError::General),
        }
    }

    unsafe fn read(
        self: Arc<Self>,
        ep: UsbEndpointAddress,
        buffer: *mut u8,
        len: usize,
    ) -> Pin<Box<dyn Future<Output = Result<usize, UsbError>>>> {
        let host = match self.host.upgrade() {
            Some(v) => v.clone(),
            None => return Box::pin(AsyncUsbError::new(UsbError::HostUnavailable)),
        };
        let dci = DCI::from(ep);
        if !dci.is_dir_in() {
            return Box::pin(AsyncUsbError::new(UsbError::InvalidParameter));
        }
        let device = self.device();
        let slot_id = device.slot_id;
        let trb = TrbNormal::new(device.buffer, len, true, true);
        let xrb = host.allocate_xrb().unwrap();
        xrb.prepare_async();
        let scheduled_trb = host.issue_trb(Some(xrb), &trb, Some(slot_id), Some(dci), true);

        Box::pin(AsyncUsbReader {
            ctx: self.clone(),
            scheduled_trb,
            xfer_buffer: buffer as usize,
            xfer_len: len,
        })
    }

    unsafe fn write(
        self: Arc<Self>,
        ep: UsbEndpointAddress,
        buffer: *const u8,
        len: usize,
    ) -> Pin<Box<dyn Future<Output = Result<usize, UsbError>>>> {
        let host = match self.host.upgrade() {
            Some(v) => v.clone(),
            None => return Box::pin(AsyncUsbError::new(UsbError::HostUnavailable)),
        };
        let dci = DCI::from(ep);
        if dci.is_dir_in() {
            return Box::pin(AsyncUsbError::new(UsbError::InvalidParameter));
        }
        let device = self.device();
        let slot_id = device.slot_id;

        // unsafe {
        let p = MemoryManager::direct_map(device.buffer) as *mut u8;
        p.copy_from(buffer, len);
        // }

        let trb = TrbNormal::new(device.buffer, len, true, true);
        let xrb = host.allocate_xrb().unwrap();
        xrb.prepare_async();
        let scheduled_trb = host.issue_trb(Some(xrb), &trb, Some(slot_id), Some(dci), true);

        Box::pin(AsyncUsbWriter {
            ctx: self.clone(),
            scheduled_trb,
            xfer_len: len,
        })
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

struct AsyncUsbReader {
    ctx: Arc<HciContext>,
    scheduled_trb: ScheduledTrb,
    xfer_buffer: usize,
    xfer_len: usize,
}

impl Future for AsyncUsbReader {
    type Output = Result<usize, UsbError>;

    fn poll(self: Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> Poll<Self::Output> {
        let host = match self.ctx.clone().host.upgrade() {
            Some(v) => v.clone(),
            None => return Poll::Ready(Err(UsbError::HostUnavailable)),
        };
        let xrb = match host.find_xrb(self.scheduled_trb, None) {
            Some(v) => v,
            None => return Poll::Ready(Err(UsbError::InvalidParameter)),
        };
        let asem = match xrb.signal {
            XrbSignalObject::Sync(_) => unreachable!(),
            XrbSignalObject::Async(ref asem) => asem,
        };
        if asem.poll(cx) {
            match xrb.state() {
                XrbState::Available | XrbState::Acquired | XrbState::Scheduled => unreachable!(),
                XrbState::Completed => {
                    let result = match xrb.response.as_event() {
                        Some(TrbEvent::TransferEvent(v)) => v.copied(),
                        _ => {
                            xrb.dispose();
                            return Poll::Ready(Err(UsbError::UnexpectedToken));
                        }
                    };
                    xrb.dispose();
                    match result.completion_code() {
                        Some(TrbCompletionCode::SUCCESS)
                        | Some(TrbCompletionCode::SHORT_PACKET) => {
                            let len = self.xfer_len - result.transfer_length();
                            unsafe {
                                let p = MemoryManager::direct_map(
                                    self.ctx.device().buffer as PhysicalAddress,
                                ) as *const u8;
                                let q = self.xfer_buffer as *mut u8;
                                q.copy_from(p, len);
                            }
                            Poll::Ready(Ok(len))
                        }
                        _ => Poll::Ready(Err(UsbError::General)),
                    }
                }
                XrbState::Aborted => Poll::Ready(Err(UsbError::Aborted)),
            }
        } else {
            Poll::Pending
        }
    }
}

struct AsyncUsbWriter {
    ctx: Arc<HciContext>,
    scheduled_trb: ScheduledTrb,
    xfer_len: usize,
}

impl Future for AsyncUsbWriter {
    type Output = Result<usize, UsbError>;

    fn poll(self: Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> Poll<Self::Output> {
        let host = match self.ctx.clone().host.upgrade() {
            Some(v) => v.clone(),
            None => return Poll::Ready(Err(UsbError::HostUnavailable)),
        };
        let xrb = match host.find_xrb(self.scheduled_trb, None) {
            Some(v) => v,
            None => return Poll::Ready(Err(UsbError::InvalidParameter)),
        };
        let asem = match xrb.signal {
            XrbSignalObject::Sync(_) => unreachable!(),
            XrbSignalObject::Async(ref asem) => asem,
        };
        if asem.poll(cx) {
            match xrb.state() {
                XrbState::Available | XrbState::Acquired | XrbState::Scheduled => unreachable!(),
                XrbState::Completed => {
                    let result = match xrb.response.as_event() {
                        Some(TrbEvent::TransferEvent(v)) => v.copied(),
                        _ => {
                            xrb.dispose();
                            return Poll::Ready(Err(UsbError::UnexpectedToken));
                        }
                    };
                    xrb.dispose();
                    match result.completion_code() {
                        Some(TrbCompletionCode::SUCCESS)
                        | Some(TrbCompletionCode::SHORT_PACKET) => {
                            let len = self.xfer_len - result.transfer_length();
                            Poll::Ready(Ok(len))
                        }
                        _ => Poll::Ready(Err(UsbError::General)),
                    }
                }
                XrbState::Aborted => Poll::Ready(Err(UsbError::Aborted)),
            }
        } else {
            Poll::Pending
        }
    }
}
