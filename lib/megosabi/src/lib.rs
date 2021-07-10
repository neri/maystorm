// MEG-OS Arlequin System Call ABI
#![no_std]

pub mod svc;

/// Invalid character representation in Rust
pub const OPTION_CHAR_NONE: u32 = 0x110000;

pub mod window {
    /// Use 32bit bitmap in window
    pub const USE_BITMAP32: u32 = 0b0000_0000_0000_0001;
    /// Transparent Window
    pub const TRANSPARENT_WINDOW: u32 = 0b0000_0000_0000_0010;
    /// Thin border
    pub const THIN_BORDER: u32 = 0b0000_0000_0000_0100;
}
