// Boot

#[cfg(any(target_os = "uefi"))]
pub mod boot_uefi;
#[cfg(any(target_os = "uefi"))]
pub use self::boot_uefi::*;

#[repr(C, packed)]
pub struct BootInfo {
    pub rsdptr: u64,
    pub total_memory_size: u64,
    pub fb_base: u64,
    pub screen_width: u16,
    pub screen_height: u16,
    pub fb_delta: u16,
    pub _reserved: u16,
}

impl BootInfo {
    pub const fn new() -> Self {
        Self {
            rsdptr: 0,
            total_memory_size: 0,
            fb_base: 0,
            screen_width: 0,
            screen_height: 0,
            fb_delta: 0,
            _reserved: 0,
        }
    }
}
