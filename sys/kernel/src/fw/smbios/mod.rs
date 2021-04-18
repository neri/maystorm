// SMBIOS

use alloc::boxed::Box;
use core::str;
use core::{mem::transmute, slice};

pub struct SMBIOS {
    base: usize,
    n_structures: usize,
}

impl SMBIOS {
    #[inline]
    pub(crate) unsafe fn init(entry: usize) -> Box<Self> {
        let ep: &SmBiosEntryV1 = transmute(entry);
        let base = ep.base as usize;
        let n_structures = ep.n_structures as usize;
        Box::new(Self { base, n_structures })
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &'static SmBiosHeader> {
        SmBiosStructWalker {
            base: self.base,
            offset: 0,
            index: 0,
            limit: self.n_structures,
        }
    }

    #[inline]
    pub fn find(&self, header_type: HeaderType) -> Option<&'static SmBiosHeader> {
        self.iter().find(|v| v.header_type() == header_type)
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct HeaderType(pub u8);

impl HeaderType {
    pub const BIOS_INFO: Self = Self(0x00);
    pub const SYSTEM_INFO: Self = Self(0x01);
}

#[repr(C)]
#[allow(dead_code)]
pub struct SmBiosEntryV1 {
    anchor: [u8; 4], // "_SM_"
    checksum: u8,
    len: u8,
    ver_major: u8,
    ver_minor: u8,
    max_struct: u16,
    revision: u8,
    formatted: [u8; 5],
    anchor2: [u8; 5], // "_DMI_"
    checksum2: u8,
    len2: u8,
    base: u32,
    n_structures: u16,
    rev: u8,
}

// impl SmBiosEntry {
//     fn is_valid(&self) -> bool {
//         (self.anchor == *b"_SM_") && (self.anchor2 == *b"_DMI_")
//     }
// }

/// Common definition of SmBios's structures
#[repr(C)]
pub struct SmBiosHeader {
    header_type: HeaderType,
    size: u8,
    handle: u16,
}

impl SmBiosHeader {
    /// Some Chinese products return a "Default string"
    pub const DEFAULT_STRING: &'static str = "Default string";

    #[inline]
    pub const fn header_type(&self) -> HeaderType {
        self.header_type
    }

    #[inline]
    pub const fn header_size(&self) -> usize {
        self.size as usize
    }

    #[inline]
    pub const fn handle(&self) -> u16 {
        self.handle
    }

    #[inline]
    pub fn as_slice<'a>(&'a self) -> &'a [u8] {
        let data = self as *const _ as *const u8;
        let len = self.header_size();
        unsafe { slice::from_raw_parts(data, len) }
    }

    #[inline]
    fn strings(&self) -> SmBiosStringWalker {
        let base = self as *const _ as usize + self.header_size();
        SmBiosStringWalker { base, offset: 0 }
    }

    #[inline]
    pub fn string<'a>(&'a self, index: usize) -> Option<&'a str> {
        if index > 0 {
            self.strings().nth(index - 1).and_then(|v| match v {
                Self::DEFAULT_STRING => None,
                _ => Some(v),
            })
        } else {
            None
        }
    }

    #[inline]
    pub fn struct_size(&self) -> usize {
        let mut iter = self.strings();
        while iter.next().is_some() {}
        self.header_size() + iter.offset + 1
    }
}

struct SmBiosStringWalker {
    base: usize,
    offset: usize,
}

impl Iterator for SmBiosStringWalker {
    type Item = &'static str;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let ptr = (self.base + self.offset) as *const u8;
            let len = strlen(ptr);
            if len > 0 {
                self.offset += len + 1;
                Some(str::from_utf8(slice::from_raw_parts(ptr, len)).unwrap_or("?"))
            } else {
                None
            }
        }
    }
}

#[inline]
unsafe fn strlen(p: *const u8) -> usize {
    let mut count = 0;
    loop {
        if p.add(count).read_volatile() == 0 {
            break count;
        } else {
            count += 1;
        }
    }
}

struct SmBiosStructWalker {
    base: usize,
    offset: usize,
    index: usize,
    limit: usize,
}

impl Iterator for SmBiosStructWalker {
    type Item = &'static SmBiosHeader;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.limit {
            return None;
        }
        unsafe {
            let p = (self.base + self.offset) as *const SmBiosHeader;
            let r = &*p;
            self.offset += r.struct_size();
            self.index += 1;
            Some(r)
        }
    }
}
