// PCI: Peripheral Component Interconnect Bus

use crate::arch::cpu::*;
use crate::system::System;
use alloc::{boxed::Box, vec::Vec};
// use num_derive::FromPrimitive;
// use num_traits::FromPrimitive;

#[derive(Debug, Copy, Clone, Default, Ord, PartialOrd, Eq, PartialEq)]
pub struct PciConfigAddressSpace {
    pub bus: u8,
    pub dev: u8,
    pub fun: u8,
    pub register: u8,
}

impl PciConfigAddressSpace {
    #[inline]
    pub const fn bus(bus: u8) -> Self {
        Self {
            bus,
            dev: 0,
            fun: 0,
            register: 0,
        }
    }

    #[inline]
    pub const fn dev(mut self, dev: u8) -> Self {
        self.dev = dev;
        self
    }

    #[inline]
    pub const fn fun(mut self, fun: u8) -> Self {
        self.fun = fun;
        self
    }

    #[inline]
    pub const fn register(mut self, register: u8) -> Self {
        self.register = register;
        self
    }
}

pub(crate) trait PciImpl {
    unsafe fn read_pci(&self, addr: PciConfigAddressSpace) -> u32;

    unsafe fn write_pci(&self, addr: PciConfigAddressSpace, value: u32);
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

    #[inline]
    fn shared() -> &'static mut Pci {
        unsafe { &mut PCI }
    }

    pub(crate) unsafe fn init() {
        let shared = Self::shared();
        let cpu = System::current_processor();
        let bus = 0;
        for dev in 0..32 {
            if let Some(device) = PciDevice::from_address(cpu, bus, dev, 0) {
                shared.devices.push(device);
            }
        }
    }

    pub fn devices() -> &'static [PciDevice] {
        Self::shared().devices.as_slice()
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
    bars: Box<[PciBar]>,
    functions: Box<[PciDevice]>,
    capabilities: Box<[(PciCapabilityId, u8)]>,
}

impl PciDevice {
    unsafe fn from_address(cpu: &Cpu, bus: u8, dev: u8, fun: u8) -> Option<Self> {
        let base = PciConfigAddressSpace::bus(bus).dev(dev).fun(fun);
        let dev_ven = cpu.read_pci(base);
        let vendor_id = PciVendorId(dev_ven as u16);
        if vendor_id == PciVendorId::INVALID {
            return None;
        }
        let device_id = PciDeviceId((dev_ven >> 16) as u16);
        let sta_cmd = cpu.read_pci(base.register(1));
        let subsys = cpu.read_pci(base.register(0x0B));
        let subsys_vendor_id = PciVendorId(subsys as u16);
        let subsys_device_id = PciDeviceId((subsys >> 16) as u16);
        let class_code = cpu.read_pci(base.register(0x02)) >> 8;
        let header_type = ((cpu.read_pci(base.register(3)) >> 16) & 0xFF) as u8;
        let has_multi_func = (header_type & 0x80) != 0;
        let header_type = header_type & 0x7F;

        let bar_limit = match header_type {
            0x00 => 6,
            0x01 => 2,
            _ => 0,
        };
        let mut bars = Vec::with_capacity(bar_limit);
        let mut index = 0;
        while index < bar_limit {
            if let Some(bar) = PciBar::parse(cpu, base, index + 4) {
                bars.push(bar);
                if bar.bar_type() == PciBarType::Mmio64 {
                    index += 1;
                }
            }
            index += 1;
        }

        let mut capabilities = Vec::new();
        if (sta_cmd & 0x0010_0000) != 0 {
            let mut cap_ptr = (cpu.read_pci(base.register(0x0D)) & 0xFF) as u8;

            loop {
                let current_register = cap_ptr / 4;
                let cap_head = cpu.read_pci(base.register(current_register));
                let cap_id = PciCapabilityId((cap_head & 0xFF) as u8);
                let next_ptr = ((cap_head >> 8) & 0xFF) as u8;

                capabilities.push((cap_id, current_register));

                if next_ptr == 0 {
                    break;
                } else {
                    cap_ptr = next_ptr;
                }
            }
        }

        let mut functions = Vec::new();
        if fun == 0 && has_multi_func {
            for fun in 1..8 {
                if let Some(function) = PciDevice::from_address(cpu, bus, dev, fun) {
                    functions.push(function);
                }
            }
        }

        let device = Self {
            addr: base,
            vendor_id,
            device_id,
            subsys_vendor_id,
            subsys_device_id,
            class_code,
            bars: bars.into_boxed_slice(),
            functions: functions.into_boxed_slice(),
            capabilities: capabilities.into_boxed_slice(),
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
    pub const fn subsys_vendor_id(&self) -> PciVendorId {
        self.subsys_vendor_id
    }

    #[inline]
    pub const fn subsys_device_id(&self) -> PciDeviceId {
        self.subsys_device_id
    }

    #[inline]
    pub const fn class_code(&self) -> u32 {
        self.class_code
    }

    #[inline]
    pub fn bars(&self) -> &[PciBar] {
        self.bars.as_ref()
    }

    #[inline]
    pub fn functions(&self) -> &[PciDevice] {
        self.functions.as_ref()
    }

    /// Returns an array of capability ID and register offset pairs.
    #[inline]
    pub fn capabilities(&self) -> &[(PciCapabilityId, u8)] {
        self.capabilities.as_ref()
    }
}

/// PCI Base Address Register
#[derive(Debug, Clone, Copy)]
pub struct PciBar(u64);

impl PciBar {
    const VALID_BASE_MASK: u64 = 0x00FF_FFFF_FFFF_FFFF;

    #[inline]
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    unsafe fn parse(cpu: &Cpu, base: PciConfigAddressSpace, index: usize) -> Option<PciBar> {
        without_interrupts!({
            let reg = base.register(index as u8);
            let raw = cpu.read_pci(reg);
            if raw == 0 {
                return None;
            }
            let org = PciBar::from_raw(raw as u64);
            match org.bar_type() {
                PciBarType::SeparatedIo | PciBarType::Mmio32 => {
                    let bias = org.bar_type().mask_bias() as u32;
                    cpu.write_pci(reg, u32::MAX);
                    let scale = (cpu.read_pci(reg) & bias).trailing_zeros() as usize;
                    cpu.write_pci(reg, org.0 as u32);
                    Some(org.set_scale(scale))
                }
                PciBarType::Mmio64 => {
                    let reg_h = base.register(index as u8 + 1);
                    let org_h = PciBar::from_raw(cpu.read_pci(reg_h) as u64);
                    let bias = org.bar_type().mask_bias();
                    cpu.write_pci(reg, u32::MAX);
                    cpu.write_pci(reg_h, u32::MAX);
                    let data = (cpu.read_pci(reg) as u64) | ((cpu.read_pci(reg_h) as u64) << 32);
                    let scale = (data & bias).trailing_zeros() as usize;
                    cpu.write_pci(reg, org.0 as u32);
                    cpu.write_pci(reg_h, org_h.0 as u32);
                    Some(
                        PciBar::from_raw(((org_h.0 as u64) << 32) | (org.0 as u64))
                            .set_scale(scale),
                    )
                }
                PciBarType::Mmio1MB | PciBarType::Reserved => None,
            }
        })
    }

    #[inline]
    pub const fn base(&self) -> u64 {
        if self.is_separated_io() {
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
        if self.is_separated_io() {
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
    pub const fn is_separated_io(&self) -> bool {
        (self.0 & 0x01) == 0x01
    }

    #[inline]
    pub const fn is_prefetchable(&self) -> bool {
        !self.is_separated_io() && (self.0 & 0x08) == 0x08
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PciBarType {
    /// Separated I/O
    SeparatedIo,
    /// Any 32bit MMIO
    Mmio32,
    /// Under 1MB MMIO (obsoleted)
    Mmio1MB,
    /// Any 64bit MMIO
    Mmio64,
    /// Reserved
    Reserved,
}

impl PciBarType {
    #[inline]
    pub const fn mask_bias(&self) -> u64 {
        match *self {
            PciBarType::SeparatedIo => !0x03,
            PciBarType::Mmio32 | PciBarType::Mmio1MB | PciBarType::Mmio64 => !0x0F,
            PciBarType::Reserved => 0,
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PciCapabilityId(pub u8);

impl PciCapabilityId {
    pub const NULL: Self = Self(0x00);
    pub const PM: Self = Self(0x01);
    pub const AGP: Self = Self(0x02);
    pub const VOD: Self = Self(0x03);
    pub const SLOT_ID: Self = Self(0x04);
    pub const MSI: Self = Self(0x05);
    pub const HOT_SWAP: Self = Self(0x06);
    pub const PCI_X: Self = Self(0x07);
    pub const HYPER_TRANSPORT: Self = Self(0x08);
    pub const VENDOR_SPECIFIC: Self = Self(0x09);
    pub const COMPACT_PCI: Self = Self(0x0B);
    pub const HOT_PLUG: Self = Self(0x0B);
    pub const AGP_8X: Self = Self(0x0E);
    pub const PCI_EXPRESS: Self = Self(0x10);
    pub const MSI_X: Self = Self(0x11);
}

impl From<u8> for PciCapabilityId {
    fn from(raw: u8) -> Self {
        Self(raw)
    }
}
