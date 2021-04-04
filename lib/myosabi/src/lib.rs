// MyOS System Call ABI
#![no_std]

pub mod svc;

pub struct MyOsAbi {}

impl MyOsAbi {
    /// Invalid character representation in Rust
    pub const OPTION_CHAR_NONE: u32 = 0x110000;

    /// Use 32bit bitmap in window
    pub const WINDOW_32BIT_BITMAP: u32 = 0b0000_0000_0000_0001;
}
