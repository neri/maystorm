// MEG-OS Arlequin System Call ABI
#![no_std]

pub mod svc;

/// Invalid character representation in Rust
pub const OPTION_CHAR_NONE: u32 = 0x110000;

pub mod window {
    /// Use 32bit bitmap in window
    pub const USE_BITMAP32: u32 = 1 << 0;
    /// Content is opaque
    pub const OPAQUE_CONTENT: u32 = 1 << 2;
    /// Thin frame
    pub const THIN_FRAME: u32 = 1 << 3;
}
