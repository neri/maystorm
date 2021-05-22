//! Haribote-OS Emulator

use super::*;
use alloc::boxed::Box;

mod hoe;
pub use hoe::*;

/// Recognize .HRB file
pub struct HrbRecognizer {
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
