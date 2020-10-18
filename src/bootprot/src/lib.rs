// Boot Info
#![no_std]
#![feature(type_alias_impl_trait)]

pub mod pe;

use bitflags::*;

#[repr(C)]
#[derive(Default)]
pub struct BootInfo {
    pub master_cr3: u64,
    pub acpi_rsdptr: u64,
    pub smbios: u64,
    pub vram_base: u64,
    pub screen_width: u16,
    pub screen_height: u16,
    pub vram_stride: u16,
    pub flags: BootFlags,
    _reserved1: [u64; 4],
    pub kernel_base: u64,
    pub total_memory_size: u64,
    pub free_memory: u32,
    pub static_start: u32,
    _reserved2: [u32; 4],
    pub real_bitmap: [u32; 8],
    pub cmdline: u64,
}

bitflags! {
    pub struct BootFlags: u16 {
        const PORTRAIT      = 0b0000_0000_0000_0001;
        const HEADLESS      = 0b0000_0000_0000_0010;
        const DEBUG_MODE    = 0b0000_0000_0000_0100;
    }
}

impl Default for BootFlags {
    fn default() -> Self {
        Self::empty()
    }
}
