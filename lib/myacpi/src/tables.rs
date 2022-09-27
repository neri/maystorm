use core::{ffi::c_void, fmt::Display, mem::transmute, str::from_utf8_unchecked};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TableId(pub [u8; 4]);

impl TableId {
    /// Extended System Description Table
    pub const XSDT: Self = Self(*b"XSDT");

    /// Fixed ACPI Description Table
    pub const FADT: Self = Self(*b"FACP");

    /// Multiple APIC Description Table
    pub const MADT: Self = Self(*b"APIC");

    /// High Precision Event Timers
    pub const HPET: Self = Self(*b"HPET");

    /// Boot Graphics Resource Table
    pub const BGRT: Self = Self(*b"BGRT");
}

impl TableId {
    #[inline]
    pub const fn as_str(&self) -> &str {
        unsafe { from_utf8_unchecked(&self.0) }
    }
}

impl Display for TableId {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[repr(C)]
#[allow(unused)]
pub struct AcpiHeader {
    signature: TableId,
    len: u32,
    rev: u8,
    checksum: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
    oem_rev: u32,
    creator_id: u32,
    creator_rev: u32,
}

impl AcpiHeader {
    #[inline]
    pub const fn signature(&self) -> TableId {
        self.signature
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    #[inline]
    pub fn assume<T: AcpiTable>(&self) -> Option<&T> {
        (self.signature() == T::TABLE_ID).then(|| unsafe { transmute(self) })
    }
}

pub unsafe trait AcpiTable: Sized {
    const TABLE_ID: TableId;

    #[inline]
    fn header(&self) -> &AcpiHeader {
        unsafe { transmute(self) }
    }
}

/// Generic Address Structure (GAS)
#[repr(C, packed)]
#[allow(unused)]
#[derive(Debug, Clone, Copy)]
pub struct Gas {
    pub id: GasAddressSpaceId,
    pub bit_width: u8,
    pub bit_offset: u8,
    pub access_size: GasAccessSize,
    pub address: u64,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum GasAddressSpaceId {
    /// System Memory space
    SystemMemory = 0,
    /// System I/O space
    SystemIo,
    /// PCI Configuration space
    PciConfiguration,
    /// Embedded Controller
    EmbeddedController,
    /// SMBus
    SmBus,
    /// SystemCMOS
    SystemCmos,
    /// PciBarTarget
    PciBarTarget,
    /// IPMI
    Ipmi,
    /// General PurposeIO
    Gpio,
    /// GenericSerialBus
    GenericSerialBus,
    /// Platform Communications Channel (PCC)
    Pcc,
    /// Platform Runtime Mechanism (PRM)
    Prm,
    /// Functional Fixed Hardware
    FunctionalFixedHardware = 0x7F,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GasAccessSize {
    Undefined = 0,
    Byte,
    Word,
    Dword,
    Qword,
}

/// Extended System Description Table
#[repr(C, packed)]
pub struct Xsdt {
    _hdr: AcpiHeader,
    _entry: u64,
}

unsafe impl AcpiTable for Xsdt {
    const TABLE_ID: TableId = TableId::XSDT;
}

impl Xsdt {
    #[inline]
    pub fn tables<'a>(&'a self) -> impl Iterator<Item = &'a AcpiHeader> {
        XsdtTables {
            xsdt: self,
            index: 0,
        }
    }

    #[inline]
    pub fn table_count(&self) -> usize {
        (self.header().len() - 36) / 8
    }

    #[inline]
    pub fn find<T: AcpiTable>(&self) -> impl Iterator<Item = &T> {
        self.tables().map(|v| v.assume()).filter_map(|v| v)
    }

    #[inline]
    pub fn find_first<T: AcpiTable>(&self) -> Option<&T> {
        self.find().next()
    }
}

struct XsdtTables<'a> {
    xsdt: &'a Xsdt,
    index: usize,
}

impl<'a> Iterator for XsdtTables<'a> {
    type Item = &'a AcpiHeader;

    fn next(&mut self) -> Option<Self::Item> {
        let offset = self.index * 8 + 36;
        if offset >= self.xsdt.header().len() {
            return None;
        } else {
            self.index += 1;

            Some(unsafe {
                &*(((self.xsdt as *const _ as *const c_void).add(offset) as *const u64)
                    .read_unaligned() as usize as *const AcpiHeader)
            })
        }
    }
}
