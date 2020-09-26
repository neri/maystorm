// Boot Info
#![no_std]
#![feature(type_alias_impl_trait)]

pub mod pe;

use bitflags::*;

#[repr(C)]
#[derive(Default)]
pub struct BootInfo {
    pub cmdline: u64,
    pub master_cr3: u64,
    pub kernel_base: u64,
    pub acpi_rsdptr: u64,
    pub smbios: u64,
    pub vram_base: u64,
    pub screen_width: u16,
    pub screen_height: u16,
    pub vram_stride: u16,
    pub flags: BootFlags,
    pub total_memory_size: u64,
    pub free_memory: u32,
    pub static_start: u32,
    pub boot_time: [u32; 4],
    pub real_bitmap: [u32; 8],
}

bitflags! {
    pub struct BootFlags: u16 {
        const HEADLESS = 0b0000_0000_0000_0001;
    }
}

impl Default for BootFlags {
    fn default() -> Self {
        Self::empty()
    }
}
