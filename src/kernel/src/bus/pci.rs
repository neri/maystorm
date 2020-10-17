// PCI: Peripheral Component Interconnect Bus
use crate::arch::cpu::*;
use alloc::vec::*;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PciConfigAddress(u32);

impl PciConfigAddress {
    pub const fn device(bus: u8, dev: u8, fun: u8) -> Self {
        Self(((bus as u32) << 16) | ((dev as u32) << 11) | ((fun as u32) << 8))
    }

    pub const fn register(self: Self, register: u8) -> Self {
        Self(self.0 | (register as u32 * 4))
    }

    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

impl Into<u32> for PciConfigAddress {
    fn into(self) -> u32 {
        self.as_u32()
    }
}

impl From<u32> for PciConfigAddress {
    fn from(val: u32) -> Self {
        Self(val)
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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct PciVendorId(pub u16);

impl PciVendorId {
    pub const INVALID: Self = Self(0xFFFF);
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct PciDeviceId(pub u16);

#[repr(C)]
#[derive(Debug)]
pub struct PciDevice {
    bus: u8,
    dev: u8,
    fun: u8,
    _padding: u8,
    vendor_id: PciVendorId,
    device_id: PciDeviceId,
    class_code: u32,
    class_string: &'static str,
    functions: Vec<PciDevice>,
}

impl PciDevice {
    unsafe fn new(bus: u8, dev: u8, fun: u8) -> Option<Self> {
        let base = PciConfigAddress::device(bus, dev, fun);
        let dev_ven = Cpu::read_pci(base);
        let vendor_id = PciVendorId(dev_ven as u16);
        if vendor_id == PciVendorId::INVALID {
            return None;
        }
        let device_id = PciDeviceId((dev_ven >> 16) as u16);
        // let subsys = Cpu::read_pci(base.register(0x0B));
        let class_code = Cpu::read_pci(base.register(0x02)) >> 8;
        let class_string = Self::find_class_string(class_code);
        let header_type = ((Cpu::read_pci(base.register(3)) >> 16) & 0xFF) as u8;
        let has_multiple_functions = (header_type & 0x80) != 0;
        let mut functions = Vec::new();
        if has_multiple_functions {
            for fun in 1..8 {
                if let Some(function) = PciDevice::new(bus, dev, fun) {
                    functions.push(function);
                }
            }
        }
        let device = Self {
            bus,
            dev,
            fun,
            _padding: 0,
            vendor_id,
            device_id,
            class_code,
            class_string,
            functions,
        };
        Some(device)
    }

    #[inline]
    pub const fn address(&self) -> (u8, u8, u8) {
        (self.bus, self.dev, self.fun)
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
    pub const fn class_string(&self) -> &'static str {
        self.class_string
    }

    #[inline]
    pub fn functions(&self) -> &[PciDevice] {
        self.functions.as_slice()
    }

    fn find_class_string(class_code: u32) -> &'static str {
        const MASK_CC: u32 = 0xFF_00_00;
        const MASK_SUB: u32 = 0xFF_FF_00;
        const MASK_IF: u32 = 0xFF_FF_FF;
        let items = [
            (0x00_00_00, MASK_SUB, "Non-VGA-Compatible devices"),
            (0x00_01_00, MASK_SUB, "VGA-Compatible Device"),
            (0x01_00_00, MASK_SUB, "SCSI Bus Controller"),
            (0x01_01_00, MASK_SUB, "IDE Controller"),
            (0x01_05_00, MASK_SUB, "ATA Controller"),
            (0x01_06_01, MASK_IF, "AHCI 1.0"),
            (0x01_06_00, MASK_SUB, "Serial ATA"),
            (0x01_07_00, MASK_IF, "SAS"),
            (0x01_07_00, MASK_SUB, "Serial Attached SCSI"),
            (0x01_08_01, MASK_IF, "NVMHCI"),
            (0x01_08_02, MASK_IF, "NVM Express"),
            (0x01_08_00, MASK_SUB, "Non-Volatile Memory Controller"),
            (0x01_00_00, MASK_CC, "Mass Storage Controller"),
            (0x02_00_00, MASK_SUB, "Ethernet Controller"),
            (0x02_00_00, MASK_CC, "Network Controller"),
            (0x03_00_00, MASK_CC, "Display Controller"),
            (0x04_00_00, MASK_SUB, "Multimedia Video Controller"),
            (0x04_01_00, MASK_SUB, "Multimedia Audio Controller"),
            (0x04_03_00, MASK_SUB, "Audio Device"),
            (0x04_00_00, MASK_CC, "Multimedia Controller"),
            (0x05_00_00, MASK_CC, "Memory Controller"),
            (0x06_00_00, MASK_SUB, "Host Bridge"),
            (0x06_01_00, MASK_SUB, "ISA Bridge"),
            (0x06_04_00, MASK_SUB, "PCI-to-PCI Bridge"),
            (0x06_09_00, MASK_SUB, "PCI-to-PCI Bridge"),
            (0x06_00_00, MASK_CC, "Bridge Device"),
            (0x07_00_00, MASK_SUB, "Serial Controller"),
            (0x07_01_00, MASK_SUB, "Parallel Controller"),
            (0x07_00_00, MASK_CC, "Simple Communication Controller"),
            (0x08_00_00, MASK_CC, "Base System Peripheral"),
            (0x09_00_00, MASK_CC, "Input Device Controller"),
            (0x0A_00_00, MASK_CC, "Docking Station"),
            (0x0B_00_00, MASK_CC, "Processor"),
            (0x0C_03_30, MASK_IF, "XHCI Controller"),
            (0x0C_03_00, MASK_SUB, "USB Controller"),
            (0x0C_00_00, MASK_CC, "Serial Bus Controller"),
            (0x0D_00_00, MASK_CC, "Wireless Controller"),
            (0x0E_00_00, MASK_CC, "Intelligent Controller"),
            (0x0F_00_00, MASK_CC, "Satellite Communication Controller"),
            (0x10_00_00, MASK_CC, "Encryption Controller"),
            (0x11_00_00, MASK_CC, "Signal Processing Controller"),
            (0x12_00_00, MASK_CC, "Processing Accelerator"),
            (0x13_00_00, MASK_CC, "Non-Essential Instrumentation"),
            (0x40_00_00, MASK_CC, "Co-Processor"),
            (0xFF_00_00, MASK_CC, "(Vendor specific)"),
        ];
        for data in &items {
            if (class_code & data.1) == data.0 {
                return data.2;
            }
        }
        "(Unknown Device)"
    }
}
