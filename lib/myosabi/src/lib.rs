// MyOS System Call ABI
#![no_std]

pub mod svc;

pub struct MyOsAbi {}

impl MyOsAbi {
    /// Invalid character representation in Rust
    pub const OPTION_CHAR_NONE: u32 = 0x110000;
}
