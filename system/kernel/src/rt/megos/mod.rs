//! MEG-OS Arlequin subsystem

mod arle;
pub use arle::*;

use super::*;
use alloc::boxed::Box;
use wasm::*;

/// Recognize .wasm file
pub struct WasmRecognizer {
    _phantom: (),
}

impl WasmRecognizer {
    pub fn new() -> Box<Self> {
        Box::new(Self { _phantom: () })
    }
}

impl BinaryRecognizer for WasmRecognizer {
    fn recognize(&self, blob: &[u8]) -> Option<Box<dyn BinaryLoader>> {
        if WasmLoader::identity(blob) {
            Some(Box::new(ArleBinaryLoader::new()) as Box<dyn BinaryLoader>)
        } else {
            None
        }
    }
}
