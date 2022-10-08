use crate::{arch::cpu::*, mem::PhysicalAddress, sync::RwLock, system::System, *};
use alloc::{
    boxed::Box,
    collections::{btree_map::Values, BTreeMap},
    string::String,
    sync::Arc,
    vec::Vec,
};
use bitflags::*;
use core::{
    cell::UnsafeCell,
    fmt,
    num::NonZeroU8,
    ops::{Add, ControlFlow},
};

use super::install_drivers;

#[repr(transparent)]
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct PciConfigAddress(u32);

impl PciConfigAddress {
    #[inline]
    pub const fn bus(bus: u8) -> Self {
        Self((bus as u32) << 24)
    }

    #[inline]
    pub const fn dev(mut self, dev: u8) -> Self {
        self.0 |= (dev as u32) << 16;
        self
    }

    #[inline]
    pub const fn fun(mut self, fun: u8) -> Self {
        self.0 |= (fun as u32) << 8;
        self
    }

    #[inline]
    pub const fn register(mut self, register: u8) -> Self {
        self.0 |= register as u32;
        self
    }

    #[inline]
    pub const fn get_bus(&self) -> u8 {
        (self.0 >> 24) as u8
    }

    #[inline]
    pub const fn get_dev(&self) -> u8 {
        (self.0 >> 16) as u8
    }

    #[inline]
    pub const fn get_fun(&self) -> u8 {
        (self.0 >> 8) as u8
    }

    #[inline]
    pub const fn get_register(&self) -> u8 {
        self.0 as u8
    }
}

impl Add<u8> for PciConfigAddress {
    type Output = Self;

    fn add(self, rhs: u8) -> Self::Output {
        let register = self.get_register().wrapping_add(rhs);
        self.register(register)
    }
}

impl fmt::Debug for PciConfigAddress {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02x}:{:02x}.{}",
            self.get_bus(),
            self.get_dev(),
            self.get_fun()
        )
    }
}

pub trait PciImpl {
    unsafe fn read_pci(&self, addr: PciConfigAddress) -> u32;

    unsafe fn write_pci(&self, addr: PciConfigAddress, value: u32);
}

pub trait PciDriverRegistrar {
    fn instantiate(&self, device: &PciDevice) -> Option<Arc<dyn PciDriver>>;
}

pub trait PciDriver {
    /// Returns the PCI configuration address of this device instance.
    fn address(&self) -> PciConfigAddress;

    /// Returns the name of the device driver.
    fn name<'a>(&self) -> &'a str;

    /// Returns the current state of the device in a human-readable format.
    fn current_status(&self) -> String;
}

static mut PCI: UnsafeCell<Pci> = UnsafeCell::new(Pci::new());

#[allow(dead_code)]
pub struct Pci {
    devices: BTreeMap<PciConfigAddress, PciDevice>,
    registrars: Vec<Box<dyn PciDriverRegistrar>>,
    drivers: RwLock<BTreeMap<PciConfigAddress, Arc<dyn PciDriver>>>,
}

impl Pci {
    const fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
            registrars: Vec::new(),
            drivers: RwLock::new(BTreeMap::new()),
        }
    }

    #[inline]
    unsafe fn shared_mut() -> &'static mut Pci {
        PCI.get_mut()
    }

    #[inline]
    fn shared() -> &'static Pci {
        unsafe { &*PCI.get() }
    }

    pub unsafe fn init() {
        let shared = Self::shared_mut();
        install_drivers(&mut shared.registrars);

        let cpu = System::current_processor();
        let bus = 0;
        for dev in 0..32 {
            PciDevice::instantiate(cpu, bus, dev, 0);
        }

        for device in Self::devices() {
            for registrar in &shared.registrars {
                match registrar.instantiate(&device) {
                    Some(v) => {
                        // log!("PCI INIT {:?}", device.address());
                        shared.drivers.write().unwrap().insert(device.address(), v);
                    }
                    None => {}
                }
            }
        }
    }

    pub fn devices<'a>() -> Values<'a, PciConfigAddress, PciDevice> {
        Self::shared().devices.values()
    }

    pub fn device_by_addr<'a>(addr: PciConfigAddress) -> Option<&'a PciDevice> {
        Self::shared().devices.get(&addr)
    }

    pub fn drivers() -> Vec<Arc<dyn PciDriver>> {
        Self::shared()
            .drivers
            .read()
            .unwrap()
            .values()
            .map(|v| v.clone())
            .into_iter()
            .collect()
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
    addr: PciConfigAddress,
    vendor_id: PciVendorId,
    device_id: PciDeviceId,
    subsys_vendor_id: PciVendorId,
    subsys_device_id: PciDeviceId,
    class_code: PciClass,
    has_multi_func: bool,
    secondary_bus_number: Option<NonZeroU8>,
    bars: Box<[PciBar]>,
    capabilities: Box<[(PciCapabilityId, u8)]>,
}

impl PciDevice {
    unsafe fn instantiate(cpu: &Cpu, bus: u8, dev: u8, fun: u8) -> bool {
        let base = PciConfigAddress::bus(bus).dev(dev).fun(fun);
        let dev_ven = cpu.read_pci(base);
        let vendor_id = PciVendorId(dev_ven as u16);
        if vendor_id == PciVendorId::INVALID {
            return false;
        }
        let device_id = PciDeviceId((dev_ven >> 16) as u16);
        let sta_cmd = cpu.read_pci(base.register(1));
        let subsys = cpu.read_pci(base.register(0x0B));
        let subsys_vendor_id = PciVendorId(subsys as u16);
        let subsys_device_id = PciDeviceId((subsys >> 16) as u16);
        let class_code = PciClass::from_pci(cpu.read_pci(base.register(0x02)));
        let header_type = ((cpu.read_pci(base.register(3)) >> 16) & 0xFF) as u8;
        let has_multi_func = (header_type & 0x80) != 0;
        let header_type = header_type & 0x7F;

        let secondary_bus_number = if header_type == 0x01 {
            // PCI to PCI bridge
            let val = cpu.read_pci(base.register(6));
            let bus = (val >> 8) as u8;
            NonZeroU8::new(bus)
        } else {
            None
        };

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

        let device = Self {
            addr: base,
            vendor_id,
            device_id,
            subsys_vendor_id,
            subsys_device_id,
            class_code,
            has_multi_func,
            secondary_bus_number,
            bars: bars.into_boxed_slice(),
            capabilities: capabilities.into_boxed_slice(),
        };
        Pci::shared_mut().devices.insert(base, device);

        if fun == 0 && has_multi_func {
            for fun in 1..8 {
                PciDevice::instantiate(cpu, bus, dev, fun);
            }
        }

        secondary_bus_number.map(|bus| {
            let bus = bus.get();
            for dev in 0..32 {
                PciDevice::instantiate(cpu, bus, dev, 0);
            }
        });

        true
    }

    #[inline]
    pub const fn address(&self) -> PciConfigAddress {
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
    pub const fn class_code(&self) -> PciClass {
        self.class_code
    }

    #[inline]
    pub const fn has_multi_func(&self) -> bool {
        self.has_multi_func
    }

    #[inline]
    pub const fn secondary_bus_number(&self) -> Option<NonZeroU8> {
        self.secondary_bus_number
    }

    #[inline]
    pub fn bars(&self) -> &[PciBar] {
        self.bars.as_ref()
    }

    /// Returns an array of capability ID and register offset pairs.
    #[inline]
    pub fn capabilities(&self) -> &[(PciCapabilityId, u8)] {
        self.capabilities.as_ref()
    }

    #[inline]
    pub unsafe fn register_msi(&self, f: fn(usize) -> (), val: usize) -> Result<(), ()> {
        let msi_reg = match self.capabilities.iter().try_for_each(|(id, offset)| {
            if *id == PciCapabilityId::MSI {
                ControlFlow::Break(*offset)
            } else {
                ControlFlow::CONTINUE
            }
        }) {
            ControlFlow::Continue(_) => return Err(()),
            ControlFlow::Break(v) => v,
        };
        let (msi_addr, msi_data) = match Cpu::register_msi(f, val) {
            Ok(v) => v,
            Err(_) => return Err(()),
        };
        let base = self.addr.register(msi_reg);

        let cpu = System::current_processor();
        cpu.write_pci(base + 1, msi_addr as u32);
        cpu.write_pci(base + 2, (msi_addr >> 32) as u32);
        cpu.write_pci(base + 3, msi_data as u32);
        cpu.write_pci(base, (cpu.read_pci(base) & 0xFF8FFFFF) | 0x00010000);

        Ok(())
    }

    pub unsafe fn read_pci_command(&self) -> PciCommand {
        let cpu = System::current_processor();
        PciCommand::from_bits_retain(cpu.read_pci(self.addr.register(1)))
    }

    pub unsafe fn write_pci_command(&self, val: PciCommand) {
        let cpu = System::current_processor();
        cpu.write_pci(self.addr.register(1), val.bits());
    }

    pub unsafe fn set_pci_command(&self, val: PciCommand) {
        let cpu = System::current_processor();
        let base = self.addr.register(1);
        cpu.write_pci(base, cpu.read_pci(base) | val.bits());
    }

    pub unsafe fn clear_pci_command(&self, val: PciCommand) {
        let cpu = System::current_processor();
        let base = self.addr.register(1);
        cpu.write_pci(base, cpu.read_pci(base) & !val.bits());
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct PciCommand: u32 {
        const IO_SPACE      = 0b0000_0000_0000_0001;
        const MEM_SPACE     = 0b0000_0000_0000_0010;
        const BUS_MASTER    = 0b0000_0000_0000_0100;

        const INT_DISABLE   = 0b0000_0100_0000_0000;
    }
}

/// PCI Base Address Register
#[derive(Debug, Clone, Copy)]
pub struct PciBar(u64);

impl PciBar {
    /// Internal data mask
    const VALID_BASE_MASK: u64 = 0x00FF_FFFF_FFFF_FFFF;

    #[inline]
    const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Parse bar
    unsafe fn parse(cpu: &Cpu, base: PciConfigAddress, index: usize) -> Option<PciBar> {
        without_interrupts!({
            let reg = base.register(index as u8);
            let raw = cpu.read_pci(reg);
            if raw == 0 {
                return None;
            }
            let org = PciBar::from_raw(raw as u64);

            let result = match org.bar_type() {
                PciBarType::IsolatedIO | PciBarType::Mmio32 => {
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
            };

            result
        })
    }

    #[inline]
    pub const fn base(&self) -> PhysicalAddress {
        if self.is_isolated_io() {
            PhysicalAddress::new(self.0 & 0xFFFF_FFFC)
        } else {
            PhysicalAddress::new((self.0 & Self::VALID_BASE_MASK) & !0x0F)
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
        if self.is_isolated_io() {
            IsolatedIO
        } else {
            match self.0 & 0x06 {
                0x00 => Mmio32,
                0x02 => Mmio1MB,
                0x04 => Mmio64,
                _ => Reserved,
            }
        }
    }

    /// Returns whether or not this BAR is an x86 isolated IO.
    #[inline]
    pub const fn is_isolated_io(&self) -> bool {
        (self.0 & 0x01) == 0x01
    }

    /// Returns whether or not this BAR is a memory-mapped IO.
    #[inline]
    pub const fn is_mmio(&self) -> bool {
        !self.is_isolated_io()
    }

    #[inline]
    pub const fn is_prefetchable(&self) -> bool {
        self.is_mmio() && (self.0 & 0x08) == 0x08
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PciBarType {
    /// Isolated I/O
    IsolatedIO,
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
            PciBarType::IsolatedIO => !0x03,
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

impl const From<u8> for PciCapabilityId {
    fn from(raw: u8) -> Self {
        Self(raw)
    }
}

/// A type that defines the PCI class code and interface.
///
/// For example, the class code for XHCI (`0x0C_03_30`) is expressed as follows.
/// ```
/// let cc = PciClass::code(0x0C).sub(0x03).interface(0x30);
/// ```
///
/// To see if one class code is included in another class code or interface, compare the following
/// ```
/// let mask = PciClass::code(0x03).sub(0x00);
/// if cc.matches(mask) {
///   // code here
/// }
/// ```
#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct PciClass(u32);

impl PciClass {
    /// Makes an instance from the PCI class code register (0x02)
    #[inline]
    pub const fn from_pci(data: u32) -> Self {
        Self((data & 0xFF_FF_FF_00) | PciClassType::Interface as u32)
    }

    /// Makes an instance from class code.
    #[inline]
    pub const fn code(code: u8) -> Self {
        Self(((code as u32) << 24) | PciClassType::ClassCode as u32)
    }

    /// Chains subclasses to the class code.
    #[inline]
    pub const fn sub(self, sub: u8) -> Self {
        Self(self.0 & 0xFF_00_00_00 | ((sub as u32) << 16) | PciClassType::Subclass as u32)
    }

    /// Chains the programming interface to the class code and subclasses.
    #[inline]
    pub const fn interface(self, interface: u8) -> Self {
        Self(self.0 & 0xFF_FF_00_00 | ((interface as u32) << 8) | PciClassType::Interface as u32)
    }

    #[inline]
    const fn class_type(&self) -> PciClassType {
        PciClassType::from_raw(self.0 & 0xFF)
    }

    #[inline]
    pub const fn raw_data(&self) -> u32 {
        self.0 & self.class_type().mask()
    }

    #[inline]
    pub const fn data(&self) -> u32 {
        self.raw_data() >> 8
    }

    /// Returns whether or not this instance matches the specified class code, subclass, or programming interface.
    #[inline]
    pub const fn matches(&self, other: Self) -> bool {
        match other.class_type() {
            PciClassType::Unspecified => false,
            _ => {
                if self.class_type().mask() < other.class_type().mask() {
                    false
                } else {
                    let mask = other.class_type().mask();
                    (self.0 & mask) == (other.0 & mask)
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum PciClassType {
    Unspecified = 0,
    ClassCode = 1,
    Subclass = 2,
    Interface = 3,
}

impl PciClassType {
    #[inline]
    pub const fn mask(&self) -> u32 {
        match *self {
            PciClassType::Unspecified => 0,
            PciClassType::ClassCode => 0xFF_00_00_00,
            PciClassType::Subclass => 0xFF_FF_00_00,
            PciClassType::Interface => 0xFF_FF_FF_00,
        }
    }

    #[inline]
    pub const fn from_raw(raw: u32) -> Self {
        match raw {
            1 => Self::ClassCode,
            2 => Self::Subclass,
            3 => Self::Interface,
            _ => Self::Unspecified,
        }
    }
}
