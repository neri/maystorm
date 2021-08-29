//! xHCI: Extensible Host Controller Interface

use super::{data::*, regs::*};
use crate::{
    arch::cpu::Cpu,
    arch::page::{NonNullPhysicalAddress, PageManager, PhysicalAddress},
    bus::pci::*,
    mem::mmio::*,
    mem::MemoryManager,
    sync::RwLock,
    sync::{fifo::EventQueue, semaphore::Semaphore},
    system::System,
    task::scheduler::*,
};
use alloc::{boxed::Box, format, string::String, sync::Arc};
use core::{
    cell::UnsafeCell, ffi::c_void, fmt::Write, mem::size_of, num::NonZeroU64, slice,
    sync::atomic::*, time::Duration,
};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

// for debug
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
#[allow(dead_code)]
pub struct Xhci {
    addr: PciConfigAddress,
    mmio: MmioSlice,

    cap: &'static CapabilityRegisters,
    opr: &'static OperationalRegisters,
    ports: &'static [UsbPort],
    doorbells: &'static [Doorbell],
    rts: &'static RuntimeRegisters,

    max_device_slots: usize,
    dcbaa_len: usize,
    min_page_size: usize,
    context_size: usize,
    ers: PhysicalAddress,

    ring_context: RwLock<[EpRingContext; Self::MAX_TR]>,
    event_cycle: CycleBit,
    port_status_change_queue: EventQueue<PortId>,
    port2slot: RwLock<[Option<SlotId>; 256]>,
    urbs: [UnsafeCell<UsbRequestBlock>; Self::MAX_URB],
    ics: [UnsafeCell<InputContext>; Self::MAX_DEVICE_SLOTS],
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
    const MAX_URB: usize = 256;

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
        let min_page_size = opr.page_size();
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
            min_page_size,
            context_size,
            ring_context: RwLock::new([EpRingContext::EMPTY; Self::MAX_TR]),
            event_cycle: CycleBit::from(true),
            ers,
            port_status_change_queue: EventQueue::new(Self::MAX_PORT_CHANGE),
            port2slot: RwLock::new([None; 256]),
            urbs: [UsbRequestBlock::EMPTY; Self::MAX_URB],
            ics: [InputContext::EMPTY; Self::MAX_DEVICE_SLOTS],
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

        println!("Starting XHCI...");

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
            let size = max_scratchpad_size * self.min_page_size;
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
    }

    fn dcbaa(&self) -> &'static mut [u64] {
        unsafe {
            slice::from_raw_parts_mut(
                PageManager::direct_map(self.opr.dcbaap() & !63) as *mut u64,
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

    pub fn doorbell(&self, slot_id: Option<SlotId>) -> &Doorbell {
        self.doorbells
            .get(slot_id.map(|v| v.0.get() as usize).unwrap_or_default())
            .unwrap()
    }

    pub fn port_by(&self, port_id: PortId) -> &UsbPort {
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

    pub fn find_ep_ring(&self, slot_id: Option<SlotId>, epno: Option<EpNo>) -> Option<usize> {
        let slot_id = slot_id.map(|v| v.0.get()).unwrap_or_default();
        let epno = epno.map(|v| v.0.get()).unwrap_or_default();
        for (index, ctx) in self.ring_context.read().unwrap().iter().enumerate() {
            let ctx_slot_id = ctx.slot_id().map(|v| v.0.get()).unwrap_or_default();
            let ctx_epno = ctx.epno().map(|v| v.0.get()).unwrap_or_default();
            if ctx.tr_base() != 0 && ctx_slot_id == slot_id && ctx_epno == epno {
                return Some(index);
            }
        }
        None
    }

    pub fn alloc_ep_ring(
        &self,
        slot_id: Option<SlotId>,
        epno: Option<EpNo>,
    ) -> Option<NonNullPhysicalAddress> {
        if let Some(index) = self.find_ep_ring(slot_id, epno) {
            let ctx = &mut self.ring_context.write().unwrap()[index];
            ctx.reset();
            return ctx.tr_value();
        }
        for ctx in self.ring_context.write().unwrap().iter_mut() {
            if ctx.tr_base() == 0 {
                ctx.alloc(slot_id, epno);
                return ctx.tr_value();
            }
        }
        None
    }

    pub fn allocate_urb<'a>(&'a self) -> Option<&'a mut UsbRequestBlock> {
        for urb in &self.urbs {
            let urb = unsafe { &mut *urb.get() };
            if urb.try_to_acquire() {
                return Some(urb);
            }
        }
        None
    }

    pub fn find_urb<'a>(
        &'a self,
        scheduled_trb: ScheduledTrb,
        state: Option<UrbState>,
    ) -> Option<&'a mut UsbRequestBlock> {
        for urb in &self.urbs {
            let urb = unsafe { &mut *urb.get() };
            let urb_state = urb.state();
            if urb_state != UrbState::Available && urb.scheduled_trb == scheduled_trb {
                match state {
                    Some(state) => {
                        if urb_state == state {
                            return Some(urb);
                        }
                    }
                    None => return Some(urb),
                }
            }
        }
        None
    }

    pub fn schedule_trb<T: TrbCommon>(
        &self,
        urb: Option<&mut UsbRequestBlock>,
        trb: &T,
        slot_id: Option<SlotId>,
        epno: Option<EpNo>,
        need_to_notify: bool,
    ) -> ScheduledTrb {
        let trb = trb.as_common_trb();
        let index = match self.find_ep_ring(slot_id, epno) {
            Some(index) => index,
            None => todo!(),
        };
        let ctx = &mut self.ring_context.write().unwrap()[index];

        let tr_base = ctx.tr_base();
        let tr = PageManager::direct_map(tr_base) as *const Trb as *mut Trb;
        let mut index = ctx.index;

        let scheduled_trb = ScheduledTrb(tr_base + (size_of::<Trb>() * index) as u64);
        if let Some(urb) = urb {
            urb.schedule(trb, scheduled_trb);
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
            self.doorbell(slot_id).set_target(epno);
        }

        scheduled_trb
    }

    pub fn execute_command<T: TrbCommon>(&self, trb: &T) -> Result<TrbCce, TrbCce> {
        let urb = self.allocate_urb().unwrap();
        self.schedule_trb(Some(urb), trb, None, None, true);
        urb.sem.wait();
        let result = urb.response.copied();
        let state = urb.state();
        urb.release();
        match state {
            UrbState::Completed => Ok(result),
            UrbState::Failed => Err(result),
            _ => unreachable!(),
        }
    }

    pub fn configure_endpoint(
        &self,
        slot_id: SlotId,
        epno: EpNo,
        ep_type: EpType,
        max_packet_size: usize,
        interval: usize,
        copy_dc: bool,
    ) {
        let input_context = self.input_context(slot_id);
        let control = input_context.control();
        let slot = input_context.slot();
        let endpoint = input_context.endpoint(epno);

        control.clear();
        control.set_add(1 | 1u32 << epno.0.get());

        if copy_dc {
            // TODO:
        }

        slot.set_context_entries(usize::max(epno.0.get() as usize, slot.context_entries()));

        endpoint.set_ep_type(ep_type);

        if max_packet_size > 0 {
            // TODO:
        } else {
            let speed: Option<PSIV> = FromPrimitive::from_usize(slot.speed_raw());
            match speed {
                Some(speed) => {
                    endpoint.set_max_packet_size(speed.max_packet_size());
                }
                None => {
                    endpoint.set_max_packet_size(512);
                }
            }
            endpoint.set_average_trb_len(8);
        }
        if interval > 0 {
            // TODO:
        }
        endpoint.set_error_count(3);

        let tr = self.alloc_ep_ring(Some(slot_id), Some(epno)).unwrap().get();
        endpoint.set_trdp(tr);
    }

    pub fn reset_port(&self, port: &UsbPort) -> PortSc {
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
                    println!("XHCI: PORT RESET TIMEDOUT {}", port_id.0.get());
                    return None;
                }

                let trb = Trb::new(TrbType::ENABLE_SLOT_COMMAND);
                let slot_id = match self.execute_command(&trb) {
                    Ok(result) => result.slot_id().unwrap(),
                    Err(err) => {
                        println!("ENABLE_SLOT ERROR {:?}", err.completion_code());
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

                self.configure_endpoint(slot_id, EpNo::CONTROL, EpType::Control, 0, 0, false);

                Timer::sleep(Duration::from_millis(100));

                let trb = TrbAddressDeviceCommand::new(slot_id, input_context_pa);
                match self.execute_command(&trb) {
                    Ok(_result) => {
                        println!("ADDRESS_DEVICE OK {:?}", slot_id);
                    }
                    Err(err) => {
                        println!("ADDRESS_DEVICE ERROR {:?}", err.completion_code());
                    }
                }

                return Some(slot_id);
            } else {
                // Detached USB device
            }
        }
        None
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
                    if let Some(urb) = self.find_urb(scheduled_trb, Some(UrbState::Scheduled)) {
                        urb.set_response(event);
                    }
                    println!(
                        "XHCI: COMMAND_COMPLETION {:?} {:?} {:016x}",
                        event.slot_id(),
                        event.completion_code(),
                        event.ptr(),
                    );
                }
                TrbEvent::PortStatusChange(event) => {
                    let port_id = event.port_id().unwrap();
                    println!("XHCI: PORT_STATUS_CHANGE {}", port_id.0.get());
                    self.port_status_change_queue.post(port_id).unwrap();
                }
                TrbEvent::TransferEvent(event) => {
                    println!(
                        "XHCI: TRANSFER {:?} {:?}",
                        event.slot_id(),
                        event.completion_code()
                    );
                }
            }
        }
    }

    /// xHCI Main event loop
    fn _event_thread(self: Arc<Self>) {
        self.initialize();

        let p = self.clone();
        SpawnOption::with_priority(Priority::High).spawn(
            move || {
                p._config_thread();
            },
            "xhci.config",
        );

        loop {
            self.process_event();
            Timer::sleep(Duration::from_millis(10));
        }
    }

    /// xHCI Configuration thread
    fn _config_thread(self: Arc<Self>) {
        for port in self.ports {
            self.reset_port(port);
        }

        loop {
            while let Some(port_id) = self.port_status_change_queue.wait_event() {
                self.port_initialize(port_id);
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
            "{} ports {} slots {}",
            status,
            self.cap.max_ports(),
            self.cap.max_device_slots(),
        )
    }
}

struct EpRingContext {
    tr_base: PhysicalAddress,
    response: Trb,
    slot_id: Option<SlotId>,
    epno: Option<EpNo>,
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
            epno: None,
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
    pub const fn epno(&self) -> Option<EpNo> {
        self.epno
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
                let p = PageManager::direct_map(self.tr_base) as *const c_void as *mut c_void;
                p.write_bytes(0, Self::size());
            }
        }
        self.response = Trb::new(TrbType::RESERVED);
        self.pcs.reset();
        self.index = 0;
    }

    #[inline]
    pub fn alloc(&mut self, slot_id: Option<SlotId>, epno: Option<EpNo>) {
        self.tr_base = unsafe { MemoryManager::alloc_pages(Self::size()) }
            .unwrap()
            .get() as PhysicalAddress;
        self.slot_id = slot_id;
        self.epno = epno;
        self.pcs.reset();
        self.index = 0;
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScheduledTrb(pub u64);

#[allow(dead_code)]
pub struct UsbRequestBlock {
    state: AtomicUsize,
    scheduled_trb: ScheduledTrb,
    sem: Semaphore,
    reuse_delay: Timer,
    request: Trb,
    response: TrbCce,
}

impl UsbRequestBlock {
    pub const EMPTY: UnsafeCell<Self> = UnsafeCell::new(Self::new());

    #[inline]
    pub const fn new() -> Self {
        Self {
            state: AtomicUsize::new(0),
            scheduled_trb: ScheduledTrb(0),
            sem: Semaphore::new(0),
            reuse_delay: Timer::JUST,
            request: Trb::empty(),
            response: TrbCce::empty(),
        }
    }

    #[inline]
    pub fn state(&self) -> UrbState {
        FromPrimitive::from_usize(self.state.load(Ordering::SeqCst)).unwrap_or_default()
    }

    #[inline]
    pub fn set_state(&self, val: UrbState) {
        self.state.store(val as usize, Ordering::SeqCst);
    }

    #[inline]
    pub fn compare_exchange_state(&self, current: UrbState, new: UrbState) -> Result<(), UrbState> {
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
        self.response = TrbCce::empty();
        fence(Ordering::SeqCst);
        self.set_state(UrbState::Scheduled);
    }

    #[inline]
    pub fn try_to_acquire(&self) -> bool {
        if self.reuse_delay.until() {
            return false;
        }
        self.compare_exchange_state(UrbState::Available, UrbState::Acquired)
            .is_ok()
    }

    #[inline]
    pub fn release(&mut self) {
        self.reuse_delay = Timer::new(Duration::from_millis(10));
        self.set_state(UrbState::Available);
    }

    #[inline]
    pub fn set_response(&mut self, response: &TrbCce) {
        self.response.raw_copy_from(response);
        self.set_state(
            if response.completion_code() == Some(TrbCompletionCode::SUCCESS) {
                UrbState::Completed
            } else {
                UrbState::Failed
            },
        );
        self.sem.signal();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
pub enum UrbState {
    Available = 0,
    Acquired,
    Scheduled,
    Completed,
    Failed,
}

impl Default for UrbState {
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
            &mut *(PageManager::direct_map(self.raw_data() as PhysicalAddress)
                as *mut InputControlContext)
        }
    }

    #[inline]
    pub fn slot<'a>(&self) -> &'a mut SlotContext {
        unsafe {
            &mut *((PageManager::direct_map(self.raw_data() as PhysicalAddress) + self.context_size)
                as *mut SlotContext)
        }
    }

    #[inline]
    pub fn endpoint<'a>(&self, dci: EpNo) -> &'a mut EndpointContext {
        unsafe {
            &mut *((PageManager::direct_map(self.raw_data() as PhysicalAddress)
                + self.context_size * (1 + dci.0.get() as usize))
                as *mut EndpointContext)
        }
    }
}
