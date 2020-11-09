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

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PciVendorId(pub u16);

impl PciVendorId {
    pub const INVALID: Self = Self(0xFFFF);
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PciDeviceId(pub u16);

#[repr(C)]
#[derive(Debug)]
pub struct PciDevice {
    addr: PciConfigAddressSpace,
    vendor_id: PciVendorId,
    device_id: PciDeviceId,
    class_code: u32,
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
        // let subsys = Cpu::read_pci(base.register(0x0B));
        let class_code = Cpu::read_pci(base.register(0x02)) >> 8;
        let header_type = ((Cpu::read_pci(base.register(3)) >> 16) & 0xFF) as u8;
        let mut functions = Vec::new();
        if fun == 0 && (header_type & 0x80) != 0 {
            for fun in 1..8 {
                if let Some(function) = PciDevice::new(bus, dev, fun) {
                    functions.push(function);
                }
            }
        }
        let device = Self {
            addr: base,
            vendor_id,
            device_id,
            class_code,
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
    pub fn functions(&self) -> &[PciDevice] {
        self.functions.as_slice()
    }
}
