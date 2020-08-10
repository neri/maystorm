// _Blobs
use core::mem::*;
use core::slice;

pub struct Blob<'a> {
    blob: &'a [u8],
}

impl<'a> Blob<'a> {
    pub const fn new(blob: &'a [u8]) -> Self {
        Self { blob }
    }
}

impl Blob<'_> {
    pub const fn read_u8(&self, offset: usize) -> u8 {
        self.blob[offset]
    }

    pub const fn read_u16(&self, offset: usize) -> u16 {
        self.blob[offset] as u16 + self.blob[offset + 1] as u16 * 0x100
    }

    pub const fn read_u32(&self, offset: usize) -> u32 {
        self.read_u16(offset) as u32 + self.read_u16(offset + 2) as u32 * 0x10000
    }

    pub const fn read_u64(&self, offset: usize) -> u64 {
        self.read_u32(offset) as u64 + self.read_u32(offset + 4) as u64 * 0x10000_0000
    }

    pub unsafe fn transmute<T>(&self, offset: usize) -> &T {
        transmute((&self.blob[0] as *const u8).add(offset))
    }

    pub unsafe fn transmute_slice<T>(&self, offset: usize, len: usize) -> &[T] {
        slice::from_raw_parts_mut(transmute((&self.blob[0] as *const u8).add(offset)), len)
    }
}
