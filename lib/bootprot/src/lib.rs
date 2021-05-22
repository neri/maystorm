//! MEG-OS Boot Procotol

#![no_std]

use bitflags::*;
use core::fmt;

#[repr(C)]
#[derive(Default)]
pub struct BootInfo {
    pub platform: Platform,
    pub color_mode: ColorMode,
    pub screen_width: u16,
    pub screen_height: u16,
    pub vram_stride: u16,
    pub vram_base: u64,
    pub master_cr3: u64,
    pub acpi_rsdptr: u64,
    pub smbios: u64,
    pub kernel_base: u64,
    pub total_memory_size: u64,
    pub cmdline: u64,
    pub initrd_base: u32,
    pub initrd_size: u32,
    pub mmap_base: u32,
    pub mmap_len: u32,
    pub real_bitmap: [u32; 8],
    pub flags: BootFlags,
}

#[non_exhaustive]
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum Platform {
    Unknown = 0,
    /// NEC PC-98 Series Computer
    Nec98 = 1,
    /// IBM PC Compatible
    PcCompatible = 2,
    /// Fujitsu FM TOWNS
    FmTowns = 3,
    /// UEFI
    UEFI = 4,
}

impl Default for Platform {
    #[inline]
    fn default() -> Self {
        Self::Unknown
    }
}

impl fmt::Display for Platform {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PcCompatible => write!(f, "PC Compatible"),
            Self::Nec98 => write!(f, "PC-98"),
            Self::FmTowns => write!(f, "FM TOWNS"),
            Self::UEFI => write!(f, "UEFI"),
            _ => write!(f, "Unknown"),
        }
    }
}

#[non_exhaustive]
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ColorMode {
    Unspecified = 0,
    /// 8bit Indexed Color Mode
    Indexed8 = 8,
    /// 32bit ARGB Color Mode
    Argb32 = 32,
}

impl Default for ColorMode {
    #[inline]
    fn default() -> Self {
        Self::Unspecified
    }
}

bitflags! {
    pub struct BootFlags: u16 {
        const FORCE_SINGLE  = 0b0000_0000_0000_0001;
        const HEADLESS      = 0b0000_0000_0000_0010;
        const DEBUG_MODE    = 0b0000_0000_0000_0100;
    }
}

impl Default for BootFlags {
    #[inline]
    fn default() -> Self {
        Self::empty()
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BootMemoryMapDescriptor {
    pub base: u64,
    pub page_count: u32,
    pub mem_type: BootMemoryType,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BootMemoryType {
    Available,
    AcpiReclaim,
    AcpiNonVolatile,
    Mmio,
    MmioPortSpace,
    OsLoaderCode,
    OsLoaderData,
    FirmwareCode,
    FirmwareData,
    Reserved,
    Unavailable,
}
