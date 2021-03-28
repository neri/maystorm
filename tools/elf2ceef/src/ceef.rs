// Compact & Efficient Executable Format (unstable)

use core::mem::size_of;
use core::mem::transmute;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CeefHeader {
    pub magic: u16,
    pub version: u8,
    pub n_secs: u8,
    pub entry: u32,
    pub base: u32,
    pub minalloc: u32,
}

impl CeefHeader {
    pub const MAGIC: u16 = 0xCEEF;
    pub const VER_CURRENT: u8 = 0;

    pub const fn is_valid(&self) -> bool {
        self.magic == Self::MAGIC && self.version == Self::VER_CURRENT
    }

    pub fn as_bytes(self) -> [u8; 16] {
        unsafe { transmute(self) }
    }

    pub const fn n_secs(&self) -> usize {
        self.n_secs as usize
    }

    pub fn size_of_headers(&self) -> usize {
        size_of::<Self>() + self.n_secs() * size_of::<CeefSecHeader>()
    }
}

impl Default for CeefHeader {
    fn default() -> Self {
        Self {
            magic: Self::MAGIC,
            version: Self::VER_CURRENT,
            n_secs: 0,
            entry: 0,
            base: 0,
            minalloc: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CeefSecHeader {
    pub attr: u8,
    _reserved: [u8; 3],
    pub filesz: u32,
    pub vaddr: u32,
    pub memsz: u32,
}

impl CeefSecHeader {
    pub const fn new(attr: u8, vaddr: u32, filesz: u32, memsz: u32, align: u8) -> Self {
        Self {
            attr: (attr << 5) | (align & 31),
            _reserved: [0, 0, 0],
            vaddr,
            filesz,
            memsz,
        }
    }

    pub fn as_bytes(self) -> [u8; 16] {
        unsafe { transmute(self) }
    }

    pub const fn attr(&self) -> usize {
        (self.attr >> 5) as usize
    }

    pub const fn align(&self) -> usize {
        (self.attr & 31) as usize
    }
}
