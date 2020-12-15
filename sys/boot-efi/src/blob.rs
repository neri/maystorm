// Blob

use byteorder::*;
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
    #[inline]
    #[track_caller]
    pub const fn read_u8(&self, offset: usize) -> u8 {
        self.blob[offset]
    }

    #[inline]
    #[track_caller]
    pub fn read_u16(&self, offset: usize) -> u16 {
        LE::read_u16(&self.blob[offset..offset + 2])
    }

    #[inline]
    #[track_caller]
    pub fn read_u32(&self, offset: usize) -> u32 {
        LE::read_u32(&self.blob[offset..offset + 4])
    }

    #[inline]
    #[track_caller]
    pub fn read_u64(&self, offset: usize) -> u64 {
        LE::read_u64(&self.blob[offset..offset + 8])
    }

    #[inline]
    #[track_caller]
    pub unsafe fn transmute<T>(&self, offset: usize) -> &T {
        transmute((&self.blob[0] as *const u8).add(offset))
    }

    #[inline]
    #[track_caller]
    pub unsafe fn transmute_slice<T>(&self, offset: usize, len: usize) -> &[T] {
        slice::from_raw_parts(transmute((&self.blob[0] as *const u8).add(offset)), len)
    }
}
