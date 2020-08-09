// Boot

#[cfg(any(target_os = "uefi"))]
pub mod boot_uefi;
#[cfg(any(target_os = "uefi"))]
pub use self::boot_uefi::*;

#[repr(C, packed)]
#[derive(Default)]
pub struct BootInfo {
    pub master_cr3: u64,
    pub rsdptr: u64,
    pub smbiod: u64,
    pub vram_base: u64,
    pub screen_width: u16,
    pub screen_height: u16,
    pub vram_delta: u16,
    pub _reserved: u16,
    pub mm_base: u64,
    pub mm_size: u64,
    pub mm_desc_size: u64,
    pub mm_ver: u64,
    pub kernel_base: u64,
    pub total_memory_size: u64,
    pub free_memory: u32,
    pub static_start: u32,
    pub boot_time: [u32; 4],
    pub real_bitmap: [u32; 8],
    pub cmdline: u64,
}
