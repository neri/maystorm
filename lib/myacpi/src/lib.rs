//! Advanced Configuration and Power Interface (ACPI)
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

mod tables;
pub use tables::*;
pub mod bgrt;
pub mod fadt;
pub mod hpet;
pub mod madt;

use core::ffi::c_void;

/// Root System Description Pointer
#[repr(C, packed)]
#[allow(unused)]
pub struct RsdPtr {
    signature: [u8; 8],
    checksum: u8,
    oem_id: [u8; 6],
    rev: u8,
    rsdt_addr: u32,
    len: u32,
    xsdt_addr: u64,
    checksum2: u8,
    _reserved: [u8; 3],
}

impl RsdPtr {
    pub const VALID_SIGNATURE: [u8; 8] = *b"RSD PTR ";
    pub const CURRENT_REV: u8 = 2;

    pub unsafe fn parse(ptr: *const c_void) -> Option<&'static Self> {
        let p = unsafe { &*(ptr as *const Self) };
        p.is_valid().then(|| p)
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        self.signature == Self::VALID_SIGNATURE && self.rev == Self::CURRENT_REV
    }

    #[inline]
    pub fn xsdt(&self) -> &Xsdt {
        unsafe { &*(self.xsdt_addr as usize as *const Xsdt) }
    }
}
