// Haribote-OS Emulator
pub mod hoe;

use super::*;
use alloc::boxed::Box;
use hoe::*;

pub(super) struct HrbRecognizer {
    _phantom: (),
}

impl HrbRecognizer {
    pub fn new() -> Box<Self> {
        Box::new(Self { _phantom: () })
    }
}

impl BinaryRecognizer for HrbRecognizer {
    fn recognize(&self, blob: &[u8]) -> Option<Box<dyn BinaryLoader>> {
        HrbBinaryLoader::identity(blob).map(|v| Box::new(v) as Box<dyn BinaryLoader>)
    }
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct HoeSyscallRegs {
    pub eax: u32,
    pub ecx: u32,
    pub edx: u32,
    pub ebx: u32,
    pub esi: u32,
    pub edi: u32,
    pub ebp: u32,
    _padding7: u32,
}
