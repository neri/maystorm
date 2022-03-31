//! xHCI MMIO Registers

use super::*;
use crate::{
    drivers::usb::*,
    mem::{MemoryManager, NonNullPhysicalAddress, PhysicalAddress},
};
use bitflags::*;
use core::{
    ffi::c_void,
    mem::size_of,
    mem::transmute,
    num::{NonZeroU8, NonZeroUsize},
    slice,
    sync::atomic::*,
};
use num_traits::FromPrimitive;

/// xHC Capability Registers
#[repr(C)]
#[allow(dead_code)]
pub struct CapabilityRegisters {
    caplength: AtomicU32,
    hcsparams1: AtomicU32,
    hcsparams2: AtomicU32,
    hcsparams3: AtomicU32,
    hccparams1: AtomicU32,
    dboff: AtomicU32,
    rtsoff: AtomicU32,
    hccparams2: AtomicU32,
}

impl CapabilityRegisters {
    #[inline]
    pub fn length(&self) -> usize {
        (self.caplength.load(Ordering::Relaxed) & 0xFF) as usize
    }

    #[inline]
    pub fn version(&self) -> (usize, usize, usize) {
        let ver = self.caplength.load(Ordering::Relaxed) >> 16;
        let ver2 = (ver & 0x0F) as usize;
        let ver1 = ((ver >> 4) & 0x0F) as usize;
        let ver0 = (ver >> 8) as usize;
        (ver0, ver1, ver2)
    }

    #[inline]
    pub fn hcs_params1(&self) -> u32 {
        self.hcsparams1.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn hcs_params2(&self) -> u32 {
        self.hcsparams2.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn hcs_params3(&self) -> u32 {
        self.hcsparams3.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn hcc_params1(&self) -> HccParams1 {
        HccParams1::from_bits_truncate(self.hccparams1.load(Ordering::Relaxed))
    }

    #[inline]
    pub fn hcc_params2(&self) -> u32 {
        self.hccparams2.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn db_off(&self) -> usize {
        (self.dboff.load(Ordering::Relaxed) & !0x03) as usize
    }

    #[inline]
    pub fn rts_off(&self) -> usize {
        (self.rtsoff.load(Ordering::Relaxed) & !0x1F) as usize
    }

    #[inline]
    pub fn max_device_slots(&self) -> usize {
        (self.hcs_params1() & 0xFF) as usize
    }

    #[inline]
    pub fn max_interrups(&self) -> usize {
        ((self.hcs_params1() >> 8) & 0x3FF) as usize
    }

    #[inline]
    pub fn max_ports(&self) -> usize {
        ((self.hcs_params1() >> 24) & 0xFF) as usize
    }

    #[inline]
    pub fn max_scratchpad_size(&self) -> usize {
        let hcs_params2 = self.hcs_params2();
        (((hcs_params2 >> 27) & 0x1F) | (((hcs_params2 >> 21) & 0x1F) << 5)) as usize
    }

    #[inline]
    pub fn opr(&self) -> &'static OperationalRegisters {
        unsafe { transmute((self as *const _ as *const c_void).add(self.length())) }
    }

    #[inline]
    pub fn ports(&self) -> &'static [PortRegisters] {
        unsafe {
            let data: *const PortRegisters =
                transmute((self as *const _ as *const c_void).add(self.length() + 0x400));
            let len = self.max_ports();
            slice::from_raw_parts(data, len)
        }
    }

    #[inline]
    pub fn doorbells(&self) -> &'static [DoorbellRegister] {
        unsafe {
            let data: *const DoorbellRegister =
                transmute((self as *const _ as *const c_void).add(self.db_off()));
            let len = self.max_device_slots();
            slice::from_raw_parts(data, len)
        }
    }

    #[inline]
    pub fn rts(&self) -> &'static RuntimeRegisters {
        unsafe { transmute((self as *const _ as *const c_void).add(self.rts_off())) }
    }

    #[inline]
    pub fn xecp(&self) -> Option<NonZeroUsize> {
        let xecp = self.hcc_params1().xecp();
        if xecp > 0 {
            NonZeroUsize::new((self as *const _ as usize) + (xecp * 4))
        } else {
            None
        }
    }
}

bitflags! {
    /// Host Controller Capability Parameters 1
    #[allow(dead_code)]
    pub struct HccParams1: u32 {
        /// 64bit Addressing Capability
        const AC64  = 0b0000_0000_0000_0001;
        /// BW Negotiation Capability
        const BNC   = 0b0000_0000_0000_0010;
        /// Context Size
        const CSZ   = 0b0000_0000_0000_0100;
        /// Port Power Control
        const PPC   = 0b0000_0000_0000_1000;
        /// Port Indicators
        const PIND  = 0b0000_0000_0001_0000;
        /// Light HC Reset Capability
        const LHRC  = 0b0000_0000_0010_0000;
        /// Latency Tolerance Messaging Capability
        const LTC   = 0b0000_0000_0100_0000;
        /// No Secondary SID Support
        const NSS   = 0b0000_0000_1000_0000;
        /// Parse All Event Data
        const PAE   = 0b0000_0001_0000_0000;
        /// Stopped - Short Packet Capacility
        const SPC   = 0b0000_0010_0000_0000;
        /// Stopped EDTLA Capability
        const SEC   = 0b0000_0100_0000_0000;
        /// Contiguous Frame ID Capability
        const CFC   = 0b0000_1000_0000_0000;
        /// Maximum Primary Stream Array Size
        const MAX_PSA_SIZE  = 0b1111_0000_0000_0000;

        const XECP = 0xFFFF_0000;
    }
}

impl HccParams1 {
    #[inline]
    pub const fn max_psa_size(&self) -> usize {
        ((self.bits() & Self::MAX_PSA_SIZE.bits()) >> 12) as usize
    }

    #[inline]
    pub const fn xecp(&self) -> usize {
        ((self.bits() & Self::XECP.bits()) >> 16) as usize
    }
}

/// xHC Operational Registers
#[repr(C)]
#[allow(dead_code)]
pub struct OperationalRegisters {
    usbcmd: AtomicU32,
    usbsts: AtomicU32,
    pagesize: AtomicU32,
    _rsrv1: [u32; 2],
    dnctrl: AtomicU32,
    crcr: AtomicU64,
    _rsrv2: [u32; 4],
    dcbaap: AtomicU64,
    config: AtomicU32,
}

impl OperationalRegisters {
    pub fn page_size_raw(&self) -> u32 {
        self.pagesize.load(Ordering::Relaxed) & 0xFFFF
    }

    #[inline]
    pub fn page_size(&self) -> usize {
        let bitmap = self.page_size_raw() & 0xFFFF;
        1usize << (12 + bitmap.trailing_zeros())
    }

    #[inline]
    pub fn read_cmd(&self) -> UsbCmd {
        UsbCmd::from_bits_truncate(self.usbcmd.load(Ordering::SeqCst))
    }

    #[inline]
    pub fn write_cmd(&self, val: UsbCmd) {
        self.usbcmd.store(val.bits(), Ordering::SeqCst);
    }

    #[inline]
    pub fn set_cmd(&self, val: UsbCmd) {
        self.write_cmd(self.read_cmd() | val);
    }

    #[inline]
    pub fn status(&self) -> UsbSts {
        UsbSts::from_bits_truncate(self.usbsts.load(Ordering::SeqCst))
    }

    #[inline]
    pub fn reset_status(&self, val: UsbSts) {
        self.usbsts.store(val.bits(), Ordering::SeqCst);
    }

    #[inline]
    pub fn set_crcr(&self, val: NonNullPhysicalAddress) {
        self.crcr.store(val.get().as_u64(), Ordering::SeqCst);
    }

    #[inline]
    pub fn dcbaap(&self) -> PhysicalAddress {
        self.dcbaap.load(Ordering::SeqCst).into()
    }

    #[inline]
    pub fn set_dcbaap(&self, val: NonNullPhysicalAddress) {
        self.dcbaap.store(val.get().as_u64(), Ordering::SeqCst);
    }

    #[inline]
    pub fn set_config(&self, max_dev_slot: usize, u3e: bool, cie: bool) {
        let val = (max_dev_slot & 0xFF) as u32
            | if u3e { 0x100 } else { 0 }
            | if cie { 0x200 } else { 0 };
        self.config.store(val, Ordering::SeqCst);
    }

    #[inline]
    pub fn device_notification_bitmap(&self) -> DeviceNotificationBitmap {
        DeviceNotificationBitmap::from_bits_truncate(self.dnctrl.load(Ordering::SeqCst))
    }

    #[inline]
    pub fn set_device_notification_bitmap(&self, bitmap: DeviceNotificationBitmap) {
        self.dnctrl.store(bitmap.bits(), Ordering::SeqCst);
    }
}

bitflags! {
    /// USBCMD Usb Command Register
    #[allow(dead_code)]
    pub struct UsbCmd: u32 {
        /// Run(1)/Stop(0)
        const RUN   = 0b0000_0000_0000_0001;
        /// Host Controller Reset
        const HCRST = 0b0000_0000_0000_0010;
        /// Interrupt Enable
        const INTE  = 0b0000_0000_0000_0100;

        // TODO: and so on...
    }
}

bitflags! {
    /// USBSTS USB Status Register
    #[allow(dead_code)]
    pub struct UsbSts: u32 {
        /// HC Halted
        const HCH   = 0b0000_0000_0000_0001;

        /// Controller Not Ready
        const CNR   = 0b0000_1000_0000_0000;

        // TODO: and so on...
    }
}

bitflags! {
    /// Device Notification
    pub struct DeviceNotificationBitmap: u32 {
        const FUNCTION_WAKE = 0b0000_0000_0000_0010;
    }
}

/// xHC USB Port Register Set
#[repr(C)]
#[allow(dead_code)]
pub struct PortRegisters {
    portsc: AtomicU32,
    portpmsc: AtomicU32,
    portli: AtomicU32,
    porthlpmc: AtomicU32,
}

impl PortRegisters {
    #[inline]
    pub fn status(&self) -> PortSc {
        PortSc::from_bits_truncate(self.portsc.load(Ordering::SeqCst))
    }

    #[inline]
    pub fn set(&self, val: PortSc) {
        self.write((self.status() & PortSc::PRESERVE_MASK) | val);
    }

    #[inline]
    pub fn write(&self, val: PortSc) {
        self.portsc.store(val.bits(), Ordering::SeqCst);
    }
}

bitflags! {
    /// Port Status and Control Register
    pub struct PortSc: u32 {
        /// A magic word to preserve mask
        const PRESERVE_MASK = 0x0E00C3E0;

        const ALL_CHANGE_BITS = 0x00FE_0000;
        /// ROS Current Connect Status
        const CCS   = 0x0000_0001;
        /// RW1CS Port Enabled
        const PED   = 0x0000_0002;
        /// RO Over current Active
        const OCA   = 0x0000_0008;
        /// RW1S Port Reset
        const PR    = 0x0000_0010;
        /// RWS Port Link State
        const PLS   = 0x0000_01E0;
        /// RWS Port Power
        const PP    = 0x0000_0200;
        /// ROW Port Speed
        const SPEED = 0x0000_3C00;
        /// RWS Port Indicator
        const PIC   = 0x0000_C000;
        /// RW Port Link State Write Strobe
        const LWS   = 0x0001_0000;
        /// RW1CS Connect Status Change
        const CSC   = 0x0002_0000;
        /// RW1CS Port Enabled/Disabled Change
        const PEC   = 0x0004_0000;
        /// RW1CS Warm Port Reset Change
        const WRC   = 0x0008_0000;
        /// RW1CS Over current Change
        const OCC   = 0x0010_0000;
        /// RW1CS Port Reset Change
        const PRC   = 0x0020_0000;
        /// RW1CS Port Link State Change
        const PLC   = 0x0040_0000;
        /// RW1CS Port Config Error Change
        const CEC   = 0x0080_0000;
        /// RO Cold Attach Status
        const CAS   = 0x0100_0000;
        /// RWS Wake on Connect Enable
        const WCE   = 0x0200_0000;
        /// RWS Wake on Disconnect Enable
        const WDE   = 0x0400_0000;
        /// RWS Wake on Over current Enable
        const WOE   = 0x0800_0000;
        /// RO Device Removable
        const DR    = 0x4000_0000;
        /// RW1S Warm Port Reset
        const WPR   = 0x8000_0000;
    }
}

impl PortSc {
    #[inline]
    pub const fn is_connected_status_changed(&self) -> bool {
        self.contains(Self::CSC)
    }

    #[inline]
    pub const fn is_connected(&self) -> bool {
        self.contains(Self::CCS)
    }

    #[inline]
    pub const fn is_enabled(&self) -> bool {
        self.contains(Self::PED)
    }

    #[inline]
    pub const fn is_powered(&self) -> bool {
        self.contains(Self::PP)
    }

    #[inline]
    pub const fn is_changed(&self) -> bool {
        (self.bits() & Self::ALL_CHANGE_BITS.bits()) != 0
    }

    #[inline]
    pub const fn link_state_raw(&self) -> usize {
        ((self.bits() & Self::PLS.bits()) as usize) >> 5
    }

    #[inline]
    pub const fn speed_raw(&self) -> usize {
        ((self.bits() & Self::SPEED.bits()) as usize) >> 10
    }

    #[inline]
    pub const fn port_indicator_raw(&self) -> usize {
        ((self.bits() & Self::PIC.bits()) as usize) >> 14
    }

    #[inline]
    pub fn speed(&self) -> Option<PSIV> {
        FromPrimitive::from_usize(self.speed_raw())
    }

    #[inline]
    pub fn link_state(&self) -> Option<Usb3LinkState> {
        FromPrimitive::from_usize(self.link_state_raw())
    }

    #[inline]
    pub fn is_usb2(&self) -> bool {
        match self.speed() {
            Some(PSIV::LS) | Some(PSIV::FS) | Some(PSIV::HS) => true,
            _ => false,
        }
    }

    #[inline]
    pub fn is_usb3(&self) -> bool {
        !self.is_usb2()
    }
}

/// xHC Runtime Registers
#[repr(C)]
pub struct RuntimeRegisters {
    mfindex: AtomicU32,
    _rsrv1: [u32; 7],
    irs: [InterrupterRegisterSet; 1],
}

impl RuntimeRegisters {
    #[inline]
    pub fn mf_index(&self) -> u32 {
        self.mfindex.load(Ordering::SeqCst) & 0x3FFF
    }

    #[inline]
    pub fn irs0(&self) -> &InterrupterRegisterSet {
        unsafe { self.irs.get_unchecked(0) }
    }

    #[inline]
    pub fn irs(&self, index: usize) -> Option<&InterrupterRegisterSet> {
        self.irs.get(index)
    }
}

/// xHC Interrupter Register Set
#[repr(C)]
#[allow(dead_code)]
pub struct InterrupterRegisterSet {
    iman: AtomicU32,
    imod: AtomicU32,
    erstsz: AtomicU32,
    _rsrv: u32,
    erstba: AtomicU64,
    erdp: AtomicU64,
}

impl InterrupterRegisterSet {
    pub const SIZE_EVENT_RING: usize = MemoryManager::PAGE_SIZE_MIN / size_of::<Trb>();

    pub unsafe fn init(&self, initial_dp: PhysicalAddress, len: usize) {
        let count = 1;
        let (base, erst) = MemoryManager::alloc_dma(count).unwrap();
        *erst = EventRingSegmentTableEntry::new(initial_dp, len as u16);
        self.erstsz.store(count as u32, Ordering::SeqCst);
        self.erdp.store(initial_dp.as_u64(), Ordering::SeqCst);
        self.erstba.store(base.as_u64(), Ordering::SeqCst);
    }

    #[inline]
    pub fn set_iman(&self, val: u32) {
        self.iman.store(val, Ordering::SeqCst);
    }

    pub fn dequeue_event<'a>(&'a self, event_cycle: &'a CycleBit) -> Option<&'a Trb> {
        let erdp = PhysicalAddress::from(self.erdp.load(Ordering::SeqCst));
        let cycle = event_cycle.value();
        let event = unsafe { &*(erdp & !15).direct_map::<Trb>() };
        if event.cycle_bit() == cycle {
            let er_base = erdp & !0xFFF;
            let mut index = 1 + (erdp - er_base) / size_of::<Trb>();
            if index == InterrupterRegisterSet::SIZE_EVENT_RING {
                index = 0;
                event_cycle.toggle();
            }
            let new_erdp = er_base.as_u64() | (index * size_of::<Trb>()) as u64 | 8;
            self.erdp.store(new_erdp, Ordering::SeqCst);

            Some(event)
        } else {
            None
        }
    }
}

/// xHC Doorbell Register
#[repr(transparent)]
pub struct DoorbellRegister(AtomicU32);

impl DoorbellRegister {
    #[inline]
    pub fn raw(&self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }

    #[inline]
    pub fn set_raw(&self, val: u32) {
        self.0.store(val, Ordering::SeqCst);
    }

    #[inline]
    pub fn target(&self) -> Option<DCI> {
        NonZeroU8::new((self.raw() & 0xFF) as u8).map(|v| DCI(v))
    }

    #[inline]
    pub fn set_target(&self, val: Option<DCI>) {
        self.set_raw(val.map(|v| v.0.get() as u32).unwrap_or_default());
    }
}
