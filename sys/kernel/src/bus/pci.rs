// PCI: Peripheral Component Interconnect Bus
use crate::arch::cpu::*;
use alloc::vec::*;

#[derive(Debug, Copy, Clone, Default, Ord, PartialOrd, Eq, PartialEq)]
pub struct PciConfigAddressSpace {
    pub bus: u8,
    pub dev: u8,
    pub fun: u8,
    pub register: u8,
}

impl PciConfigAddressSpace {
    pub const fn bus(bus: u8) -> Self {
        Self {
            bus,
            dev: 0,
            fun: 0,
            register: 0,
        }
    }

    pub const fn dev(mut self, dev: u8) -> Self {
        self.dev = dev;
        self
    }

    pub const fn fun(mut self, fun: u8) -> Self {
        self.fun = fun;
        self
    }

    pub const fn register(mut self, register: u8) -> Self {
        self.register = register;
        self
    }
}

static mut PCI: Pci = Pci::new();

pub struct Pci {
    devices: Vec<PciDevice>,
}

impl Pci {
    const fn new() -> Self {
        Self {
            devices: Vec::new(),
        }
    }

    fn shared() -> &'static mut Pci {
        unsafe { &mut PCI }
    }

    pub(crate) unsafe fn init() {
        let shared = Self::shared();
        let bus = 0;
        for dev in 0..32 {
            if let Some(device) = PciDevice::new(bus, dev, 0) {
                shared.devices.push(device);
            }
        }
    }

    pub fn devices() -> &'static [PciDevice] {
        let shared = Self::shared();
        shared.devices.as_slice()
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PciVendorId(pub u16);

impl PciVendorId {
    pub const INVALID: Self = Self(0xFFFF);
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PciDeviceId(pub u16);

#[repr(C)]
#[derive(Debug)]
pub struct PciDevice {
    addr: PciConfigAddressSpace,
    vendor_id: PciVendorId,
    device_id: PciDeviceId,
    subsys_vendor_id: PciVendorId,
    subsys_device_id: PciDeviceId,
    class_code: u32,
    bars: Vec<PciBar>,
    functions: Vec<PciDevice>,
}

impl PciDevice {
    unsafe fn new(bus: u8, dev: u8, fun: u8) -> Option<Self> {
        let base = PciConfigAddressSpace::bus(bus).dev(dev).fun(fun);
        let dev_ven = Cpu::read_pci(base);
        let vendor_id = PciVendorId(dev_ven as u16);
        if vendor_id == PciVendorId::INVALID {
            return None;
        }
        let device_id = PciDeviceId((dev_ven >> 16) as u16);
        let subsys = Cpu::read_pci(base.register(0x0B));
        let subsys_vendor_id = PciVendorId(subsys as u16);
        let subsys_device_id = PciDeviceId((subsys >> 16) as u16);
        let class_code = Cpu::read_pci(base.register(0x02)) >> 8;
        let header_type = ((Cpu::read_pci(base.register(3)) >> 16) & 0xFF) as u8;

        let mut bars = Vec::new();
        let limit_bar = if header_type == 0x00 {
            6
        } else if header_type == 0x01 {
            2
        } else {
            0
        };

        let mut index = 0;
        while index < limit_bar {
            if let Some(bar) = PciBar::parse(base, index + 4) {
                bars.push(bar);
                if bar.bar_type() == PciBarType::Mmio64 {
                    index += 1;
                }
            }
            index += 1;
        }

        let mut functions = Vec::new();
        if fun == 0 && (header_type & 0x80) != 0 {
            for fun in 1..8 {
                if let Some(function) = PciDevice::new(bus, dev, fun) {
                    functions.push(function);
                }
            }
        }
        functions.shrink_to_fit();

        let device = Self {
            addr: base,
            vendor_id,
            device_id,
            subsys_vendor_id,
            subsys_device_id,
            class_code,
            bars,
            functions,
        };
        Some(device)
    }

    #[inline]
    pub const fn address(&self) -> PciConfigAddressSpace {
        self.addr
    }

    #[inline]
    pub const fn vendor_id(&self) -> PciVendorId {
        self.vendor_id
    }

    #[inline]
    pub const fn device_id(&self) -> PciDeviceId {
        self.device_id
    }

    #[inline]
    pub const fn class_code(&self) -> u32 {
        self.class_code
    }

    #[inline]
    pub fn bars(&self) -> &[PciBar] {
        self.bars.as_slice()
    }

    #[inline]
    pub fn functions(&self) -> &[PciDevice] {
        self.functions.as_slice()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PciBar(u64);

impl PciBar {
    const VALID_BASE_MASK: u64 = 0x00FF_FFFF_FFFF_FFFF;

    #[inline]
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    unsafe fn parse(base: PciConfigAddressSpace, index: usize) -> Option<PciBar> {
        Cpu::without_interrupts(|| {
            let reg = base.register(index as u8);
            let raw = Cpu::read_pci(reg);
            if raw == 0 {
                return None;
            }
            let org = PciBar::from_raw(raw as u64);
            match org.bar_type() {
                PciBarType::SeparatedIo | PciBarType::Mmio32 | PciBarType::Mmio1MB => {
                    let mask = org.bar_type().mask() as u32;
                    let bias = org.bar_type().mask_bias() as u32;
                    Cpu::write_pci(reg, mask);
                    let scale = (!Cpu::read_pci(reg) | bias).count_ones() as usize;
                    Cpu::write_pci(reg, org.0 as u32);
                    Some(org.set_scale(scale))
                }
                PciBarType::Mmio64 => {
                    let reg_h = base.register(index as u8 + 1);
                    let org_h = PciBar::from_raw(Cpu::read_pci(reg_h) as u64);
                    let mask = org.bar_type().mask() as u32;
                    let bias = org.bar_type().mask_bias();
                    Cpu::write_pci(reg, mask);
                    Cpu::write_pci(reg_h, u32::MAX);
                    let data = (Cpu::read_pci(reg) as u64) | ((Cpu::read_pci(reg_h) as u64) << 32);
                    let scale = (!data | bias).count_ones() as usize;
                    Cpu::write_pci(reg, org.0 as u32);
                    Cpu::write_pci(reg_h, org_h.0 as u32);
                    Some(
                        PciBar::from_raw(((org_h.0 as u64) << 32) | (org.0 as u64))
                            .set_scale(scale),
                    )
                }
                PciBarType::Reserved => None,
            }
        })
    }

    #[inline]
    pub const fn base(&self) -> u64 {
        if self.is_io() {
            self.0 & 0xFFFF_FFFC
        } else {
            (self.0 & Self::VALID_BASE_MASK) & !0x0F
        }
    }

    #[inline]
    pub const fn size(&self) -> usize {
        1 << ((self.0 >> 56) & 63)
    }

    #[inline]
    fn set_scale(mut self, scale: usize) -> Self {
        self.0 = (self.0 & Self::VALID_BASE_MASK) | ((scale as u64) << 56);
        self
    }

    #[inline]
    pub const fn bar_type(&self) -> PciBarType {
        use PciBarType::*;
        if self.is_io() {
            SeparatedIo
        } else {
            match self.0 & 0x06 {
                0x00 => Mmio32,
                0x02 => Mmio1MB,
                0x04 => Mmio64,
                _ => Reserved,
            }
        }
    }

    #[inline]
    pub const fn is_io(&self) -> bool {
        (self.0 & 0x01) == 0x01
    }

    #[inline]
    pub const fn is_prefetchable(&self) -> bool {
        !self.is_io() && (self.0 & 0x08) == 0x08
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PciBarType {
    /// Separated I/O
    SeparatedIo,
    /// Any 32bit MMIO
    Mmio32,
    /// Under 1MB MMIO
    Mmio1MB,
    /// Any 64bit MMIO
    Mmio64,
    /// Reserved
    Reserved,
}

impl PciBarType {
    #[inline]
    pub const fn mask(&self) -> u64 {
        match *self {
            PciBarType::SeparatedIo => 0xFFFF_FFFC,
            PciBarType::Mmio32 | PciBarType::Mmio1MB => 0xFFFF_FFF0,
            PciBarType::Mmio64 => 0xFFFF_FFFF_FFFF_FFF0,
            PciBarType::Reserved => 0,
        }
    }

    #[inline]
    pub const fn mask_bias(&self) -> u64 {
        match *self {
            PciBarType::SeparatedIo => 0x03,
            PciBarType::Mmio32 | PciBarType::Mmio1MB | PciBarType::Mmio64 => 0x0F,
            PciBarType::Reserved => 0,
        }
    }
}
